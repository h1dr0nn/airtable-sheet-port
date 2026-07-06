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

  it("auto-approve is off by default, toggles, and resetSettings clears it", async () => {
    const ipc = createDemoIpc();

    // Mirrors the backend default: meta key absent reads back as off.
    expect((await settle(ipc.getSettings())).autoApproveWrites).toBe(false);

    await settle(ipc.setAutoApprove(true));
    expect((await settle(ipc.getSettings())).autoApproveWrites).toBe(true);

    await settle(ipc.resetSettings());
    expect((await settle(ipc.getSettings())).autoApproveWrites).toBe(false);

    const auditActions = (await settle(ipc.listAuditEvents(null, null))).map(
      (event) => event.action
    );
    expect(auditActions).toContain("settings_updated");
    expect(auditActions).toContain("settings_reset");
  });
});

describe("demo IPC mcp flow", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("defaults to stdio with a null boundPort and a running sidecar", async () => {
    const ipc = createDemoIpc();

    const config = await settle(ipc.getMcpConfig());
    expect(config.transport).toBe("stdio");
    expect(config.port).toBe(4319);
    expect(config.running).toBe(true);
    // boundPort is only meaningful for a running HTTP sidecar.
    expect(config.boundPort).toBeNull();
  });

  it("switching to http exposes boundPort at the configured port", async () => {
    const ipc = createDemoIpc();

    await settle(ipc.setMcpTransport("http"));
    await settle(ipc.setMcpPort(5000));

    const config = await settle(ipc.getMcpConfig());
    expect(config.transport).toBe("http");
    expect(config.port).toBe(5000);
    expect(config.boundPort).toBe(5000);
  });

  it("setMcpPort rejects out-of-range ports", async () => {
    const ipc = createDemoIpc();

    const assertion = expect(ipc.setMcpPort(80)).rejects.toThrow(
      "Port must be an integer between 1024 and 65535"
    );
    await vi.runAllTimersAsync();
    await assertion;
  });

  it("detects a plausible roster with every client state present", async () => {
    const ipc = createDemoIpc();

    const clients = await settle(ipc.mcpDetectClients());
    const byState = new Map(clients.map((client) => [client.state, client]));
    expect(byState.has("unconfigured")).toBe(true);
    expect(byState.has("configured")).toBe(true);
    expect(byState.has("not_found")).toBe(true);
  });

  it("configure and unregister flip an installed client's state", async () => {
    const ipc = createDemoIpc();
    const unconfigured = (await settle(ipc.mcpDetectClients())).find(
      (client) => client.state === "unconfigured"
    );
    if (!unconfigured) {
      throw new Error("expected an unconfigured demo client");
    }

    await settle(ipc.mcpConfigureClient(unconfigured.id));
    let after = (await settle(ipc.mcpDetectClients())).find((c) => c.id === unconfigured.id);
    expect(after?.state).toBe("configured");

    await settle(ipc.mcpUnregisterClient(unconfigured.id));
    after = (await settle(ipc.mcpDetectClients())).find((c) => c.id === unconfigured.id);
    expect(after?.state).toBe("unconfigured");
  });

  it("configureAll configures installed clients but leaves absent ones", async () => {
    const ipc = createDemoIpc();

    await settle(ipc.mcpConfigureAll());
    const clients = await settle(ipc.mcpDetectClients());
    expect(clients.some((client) => client.state === "unconfigured")).toBe(false);
    // A not_found client is never installed, so it stays absent.
    expect(clients.some((client) => client.state === "not_found")).toBe(true);
  });

  it("rejects configuring a client that is not installed", async () => {
    const ipc = createDemoIpc();
    const absent = (await settle(ipc.mcpDetectClients())).find(
      (client) => client.state === "not_found"
    );
    if (!absent) {
      throw new Error("expected a not_found demo client");
    }

    const assertion = expect(ipc.mcpConfigureClient(absent.id)).rejects.toThrow("not installed");
    await vi.runAllTimersAsync();
    await assertion;
  });
});
