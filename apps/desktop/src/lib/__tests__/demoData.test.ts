import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createDemoIpc } from "../demoData.js";

/** Kicks the demo backend's internal setTimeout delays under fake timers. */
async function settle<T>(promise: Promise<T>): Promise<T> {
  await vi.runAllTimersAsync();
  return promise;
}

describe("demo IPC google flow", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("models the real v2 empty state on a fresh instance", async () => {
    const ipc = createDemoIpc();

    expect(await settle(ipc.listSources())).toEqual([]);
    expect(await settle(ipc.listPermissionRules())).toEqual([]);
    expect(await settle(ipc.listChanges(null))).toEqual([]);
    expect(await settle(ipc.listAuditEvents(null, null))).toEqual([]);
    expect((await settle(ipc.tokenStatus())).googleSheets).toBe(false);

    const config = await settle(ipc.getGoogleConfig());
    expect(config.connectedEmail).toBeNull();
    // Pre-seeded so the browser preview's Connect button is clickable.
    expect(config.clientId).not.toBeNull();
    // Mirrors the real backend: no secret in the keychain until saved.
    expect(config.hasClientSecret).toBe(false);
  });

  it("setGoogleClientSecret stores presence and empty string clears it", async () => {
    const ipc = createDemoIpc();

    await settle(ipc.setGoogleClientSecret("GOCSPX-demo-secret"));
    expect((await settle(ipc.getGoogleConfig())).hasClientSecret).toBe(true);

    await settle(ipc.setGoogleClientSecret(""));
    expect((await settle(ipc.getGoogleConfig())).hasClientSecret).toBe(false);
  });

  it("connect adds a connected google-sheets source with tables and a seeded change", async () => {
    const ipc = createDemoIpc();

    const { email } = await settle(ipc.googleConnect());
    expect(email).toContain("@");

    const sources = await settle(ipc.listSources());
    expect(sources).toHaveLength(1);
    expect(sources[0]).toMatchObject({
      id: "google-sheets",
      kind: "google_sheets",
      status: "connected"
    });
    expect(sources[0]?.name).toContain(email);

    const config = await settle(ipc.getGoogleConfig());
    expect(config.connectedEmail).toBe(email);
    expect((await settle(ipc.tokenStatus())).googleSheets).toBe(true);

    const tables = await settle(ipc.listTables("google-sheets"));
    expect(tables.length).toBeGreaterThan(0);
    const firstTable = tables[0];
    if (!firstTable) {
      throw new Error("expected a demo table");
    }
    const page = await settle(ipc.readTable(firstTable.sourceId, firstTable.tableId, null, null));
    expect(page.total).toBeGreaterThan(0);

    const changes = await settle(ipc.listChanges("pending"));
    expect(changes).toHaveLength(1);
    expect(changes[0]?.sourceId).toBe("google-sheets");

    const auditActions = (await settle(ipc.listAuditEvents(null, null))).map(
      (event) => event.action
    );
    expect(auditActions).toContain("google_connected");
  });

  it("disconnect removes the source and is idempotent", async () => {
    const ipc = createDemoIpc();
    await settle(ipc.googleConnect());

    await settle(ipc.googleDisconnect());
    expect(await settle(ipc.listSources())).toEqual([]);
    expect((await settle(ipc.getGoogleConfig())).connectedEmail).toBeNull();
    expect((await settle(ipc.tokenStatus())).googleSheets).toBe(false);
    expect(await settle(ipc.listTables("google-sheets"))).toEqual([]);

    // Second disconnect mirrors core::google::disconnect (no error).
    await expect(settle(ipc.googleDisconnect())).resolves.toBeUndefined();
  });

  it("setGoogleClientId trims the value and rejects blank input", async () => {
    const ipc = createDemoIpc();

    await settle(ipc.setGoogleClientId("  my-client-id.apps.googleusercontent.com  "));
    expect((await settle(ipc.getGoogleConfig())).clientId).toBe(
      "my-client-id.apps.googleusercontent.com"
    );

    // Attach the rejection handler before advancing timers so the rejected
    // promise is never momentarily unhandled.
    const assertion = expect(ipc.setGoogleClientId("   ")).rejects.toThrow(
      "Google client ID must not be empty"
    );
    await vi.runAllTimersAsync();
    await assertion;
  });

  it("connect fails when no client id is configured", async () => {
    const ipc = createDemoIpc({ googleClientId: null });

    // Rejects before any timer is scheduled, so no timer kick is needed.
    await expect(ipc.googleConnect()).rejects.toThrow("Google client ID is not configured");
    expect(await settle(ipc.listSources())).toEqual([]);
  });
});
