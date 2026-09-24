import { afterEach, describe, expect, it, vi } from "vitest";
import { MutationObserver, QueryClient } from "@tanstack/react-query";
import { createTranslator } from "../../i18n/useTranslation.js";
import type { GoogleAccount } from "../../lib/ipc.js";
import { addBridgeMutationOptions, type AddBridgeInput } from "../useGoogleBridges.js";

const { googleAddBridge, toastSuccess, toastError } = vi.hoisted(() => ({
  googleAddBridge: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn()
}));

vi.mock("../../lib/ipc.js", () => ({ ipc: { googleAddBridge } }));
vi.mock("@sheet-port/ui", () => ({ toast: { success: toastSuccess, error: toastError } }));

const BRIDGE_URL = "https://script.google.com/macros/s/AKfycbTestDeployment/exec";
const BRIDGE_SECRET = "top-secret-value";
const ACCOUNT = {
  sourceId: "google-sheets:user_example_com",
  email: "user@example.com",
  deploymentId: "AKfycbTestDeployment",
  bridgeUrl: BRIDGE_URL
};

/** Runs the add-bridge mutation and returns the serialized mutation cache. */
async function runAdd(): Promise<{ queryClient: QueryClient; cache: string }> {
  const queryClient = new QueryClient();
  const observer = new MutationObserver<GoogleAccount, unknown, AddBridgeInput>(
    queryClient,
    addBridgeMutationOptions(queryClient, createTranslator("en"))
  );
  await observer.mutate({ url: BRIDGE_URL, secret: BRIDGE_SECRET }).catch(() => undefined);
  const cache = JSON.stringify(
    queryClient
      .getMutationCache()
      .getAll()
      .map((mutation) => mutation.state)
  );
  return { queryClient, cache: `${cache}${JSON.stringify(observer.getCurrentResult())}` };
}

describe("addBridgeMutationOptions", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("sends the secret over IPC but keeps it out of the mutation cache on success", async () => {
    googleAddBridge.mockResolvedValue(ACCOUNT);

    const { cache } = await runAdd();

    expect(googleAddBridge).toHaveBeenCalledWith(BRIDGE_URL, BRIDGE_SECRET);
    expect(cache).toContain(BRIDGE_URL);
    expect(cache).not.toContain(BRIDGE_SECRET);
    expect(toastSuccess).toHaveBeenCalledTimes(1);
    expect(JSON.stringify(toastSuccess.mock.calls)).not.toContain(BRIDGE_SECRET);
  });

  it("keeps the secret out of the mutation cache and the toast on failure", async () => {
    googleAddBridge.mockRejectedValue(new Error("The bridge rejected the secret"));

    const { cache } = await runAdd();

    expect(cache).not.toContain(BRIDGE_SECRET);
    expect(toastError).toHaveBeenCalledTimes(1);
    expect(JSON.stringify(toastError.mock.calls)).not.toContain(BRIDGE_SECRET);
  });

  it("invalidates the Google account list after an add", async () => {
    googleAddBridge.mockResolvedValue(ACCOUNT);
    const queryClient = new QueryClient();
    const invalidate = vi.spyOn(queryClient, "invalidateQueries");
    const observer = new MutationObserver<GoogleAccount, unknown, AddBridgeInput>(
      queryClient,
      addBridgeMutationOptions(queryClient, createTranslator("en"))
    );

    await observer.mutate({ url: BRIDGE_URL, secret: BRIDGE_SECRET });

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["google-accounts"] });
  });
});
