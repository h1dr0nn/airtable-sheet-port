import { afterEach, describe, expect, it, vi } from "vitest";
import { MutationObserver, QueryClient } from "@tanstack/react-query";
import { createTranslator } from "../../i18n/useTranslation.js";
import { clearAuditMutationOptions } from "../useAuditEvents.js";

const { clearAuditLog, toastSuccess, toastError } = vi.hoisted(() => ({
  clearAuditLog: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn()
}));

vi.mock("../../lib/ipc.js", () => ({ ipc: { clearAuditLog } }));
vi.mock("@sheet-port/ui", () => ({ toast: { success: toastSuccess, error: toastError } }));

async function runClear(queryClient = new QueryClient()): Promise<void> {
  const observer = new MutationObserver<void, unknown, void>(
    queryClient,
    clearAuditMutationOptions(queryClient, createTranslator("en"))
  );
  await observer.mutate().catch(() => undefined);
}

describe("clearAuditMutationOptions", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("does not announce a successful clear with a new notification", async () => {
    clearAuditLog.mockResolvedValue(undefined);

    await runClear();

    expect(clearAuditLog).toHaveBeenCalledTimes(1);
    expect(toastSuccess).not.toHaveBeenCalled();
    expect(toastError).not.toHaveBeenCalled();
  });

  it("refreshes every activity query after the clear", async () => {
    clearAuditLog.mockResolvedValue(undefined);
    const queryClient = new QueryClient();
    const invalidate = vi.spyOn(queryClient, "invalidateQueries");

    await runClear(queryClient);

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["audit-events"] });
  });

  it("reports a failed clear", async () => {
    clearAuditLog.mockRejectedValue(new Error("database is locked"));

    await runClear();

    expect(toastError).toHaveBeenCalledTimes(1);
    expect(toastSuccess).not.toHaveBeenCalled();
  });
});
