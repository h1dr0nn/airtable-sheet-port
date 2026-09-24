import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createDemoIpc } from "../demoData.js";

/** Kicks the demo backend's internal setTimeout delays under fake timers. */
async function settle<T>(promise: Promise<T>): Promise<T> {
  await vi.runAllTimersAsync();
  return promise;
}

const BRIDGE_URL = "https://script.google.com/macros/s/AKfycbDemoDeployment1/exec";
const SECOND_BRIDGE_URL = "https://script.google.com/a/macros/example.com/s/AKfycbDemoDeployment2/exec";
const BRIDGE_SECRET = "demo-secret";

describe("demo IPC google bridge flow", () => {
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
    expect(await settle(ipc.tokenStatus())).toEqual({ googleSheets: false });
    expect(await settle(ipc.googleListAccounts())).toEqual([]);
  });

  it("adding a bridge links an account with tables and a seeded staged change", async () => {
    const ipc = createDemoIpc();

    const account = await settle(ipc.googleAddBridge(BRIDGE_URL, BRIDGE_SECRET));
    expect(account.email).toContain("@");
    expect(account.deploymentId).toBe("AKfycbDemoDeployment1");
    expect(account.bridgeUrl).toBe(BRIDGE_URL);

    const sources = await settle(ipc.listSources());
    expect(sources).toHaveLength(1);
    expect(sources[0]).toMatchObject({
      id: account.sourceId,
      kind: "google_sheets",
      status: "connected"
    });
    expect(sources[0]?.name).toContain(account.email);

    expect(await settle(ipc.googleListAccounts())).toEqual([account]);
    expect((await settle(ipc.tokenStatus())).googleSheets).toBe(true);

    const tables = await settle(ipc.listTables(account.sourceId));
    const firstTable = tables[0];
    if (!firstTable) {
      throw new Error("expected a demo table");
    }
    const page = await settle(ipc.readTable(firstTable.sourceId, firstTable.tableId, null, null));
    expect(page.total).toBeGreaterThan(0);

    const changes = await settle(ipc.listChanges("pending"));
    expect(changes).toHaveLength(1);
    expect(changes[0]?.sourceId).toBe(account.sourceId);

    const auditActions = (await settle(ipc.listAuditEvents(null, null))).map(
      (event) => event.action
    );
    expect(auditActions).toContain("google_bridge_added");
  });

  it("rejects a malformed URL or an empty secret before calling the bridge", async () => {
    const ipc = createDemoIpc();

    await expect(ipc.googleAddBridge("https://example.com/exec", BRIDGE_SECRET)).rejects.toThrow(
      "bridge URL"
    );
    await expect(ipc.googleAddBridge(BRIDGE_URL, "   ")).rejects.toThrow("secret");
    expect(await settle(ipc.googleListAccounts())).toEqual([]);
  });

  it("re-adding the same deployment replaces it instead of adding an account", async () => {
    const ipc = createDemoIpc();

    const first = await settle(ipc.googleAddBridge(BRIDGE_URL, BRIDGE_SECRET));
    const again = await settle(ipc.googleAddBridge(BRIDGE_URL, "rotated-secret"));
    expect(again.sourceId).toBe(first.sourceId);
    expect(await settle(ipc.googleListAccounts())).toHaveLength(1);
  });

  it("a second deployment adds a distinct account and remove drops just one", async () => {
    const ipc = createDemoIpc();

    const first = await settle(ipc.googleAddBridge(BRIDGE_URL, BRIDGE_SECRET));
    const second = await settle(ipc.googleAddBridge(SECOND_BRIDGE_URL, BRIDGE_SECRET));
    expect(second.email).not.toBe(first.email);
    expect(second.deploymentId).toBe("AKfycbDemoDeployment2");

    await settle(ipc.googleRemoveBridge(first.sourceId));
    expect(await settle(ipc.googleListAccounts())).toEqual([second]);
  });

  it("remove clears the source and is idempotent", async () => {
    const ipc = createDemoIpc();
    const account = await settle(ipc.googleAddBridge(BRIDGE_URL, BRIDGE_SECRET));

    await settle(ipc.googleRemoveBridge(account.sourceId));
    expect(await settle(ipc.listSources())).toEqual([]);
    expect(await settle(ipc.googleListAccounts())).toEqual([]);
    expect((await settle(ipc.tokenStatus())).googleSheets).toBe(false);
    expect(await settle(ipc.listTables(account.sourceId))).toEqual([]);

    // Second remove mirrors core::google::remove_bridge (no error).
    await expect(settle(ipc.googleRemoveBridge(account.sourceId))).resolves.toBeUndefined();
  });

  it("test returns the account and records an audit event", async () => {
    const ipc = createDemoIpc();
    const account = await settle(ipc.googleAddBridge(BRIDGE_URL, BRIDGE_SECRET));

    expect(await settle(ipc.googleTestBridge(account.sourceId))).toEqual(account);
    const auditActions = (await settle(ipc.listAuditEvents(null, null))).map(
      (event) => event.action
    );
    expect(auditActions).toContain("google_bridge_tested");

    await expect(ipc.googleTestBridge("google-sheets:missing")).rejects.toThrow("No bridge");
  });

  it("discarding a pending change marks it rejected by the user", async () => {
    const ipc = createDemoIpc();
    await settle(ipc.googleAddBridge(BRIDGE_URL, BRIDGE_SECRET));
    const [pending] = await settle(ipc.listChanges("pending"));
    if (!pending) {
      throw new Error("expected a seeded pending change");
    }

    const discarded = await settle(ipc.rejectChange(pending.id));
    expect(discarded.status).toBe("rejected");
    expect(discarded.decidedBy).toBe("user");
    expect(await settle(ipc.listChanges("pending"))).toEqual([]);
  });

  it("font preferences default, persist, and reset with settings", async () => {
    const ipc = createDemoIpc();

    let settings = await settle(ipc.getSettings());
    expect(settings.fontScale).toBe("normal");
    expect(settings.fontFamily).toBe("modern");

    await settle(ipc.setFontScale("large"));
    await settle(ipc.setFontFamily("classic"));
    settings = await settle(ipc.getSettings());
    expect(settings.fontScale).toBe("large");
    expect(settings.fontFamily).toBe("classic");

    await settle(ipc.resetSettings());
    settings = await settle(ipc.getSettings());
    expect(settings.fontScale).toBe("normal");
    expect(settings.fontFamily).toBe("modern");

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

  it("http sidecar is offline until started, then start/stop toggle it", async () => {
    const ipc = createDemoIpc();

    await settle(ipc.setMcpTransport("http"));
    await settle(ipc.setMcpPort(5000));

    // Managed HTTP child is not running until explicitly started.
    let config = await settle(ipc.getMcpConfig());
    expect(config.transport).toBe("http");
    expect(config.port).toBe(5000);
    expect(config.running).toBe(false);
    expect(config.boundPort).toBeNull();

    const started = await settle(ipc.mcpServerStart());
    expect(started.running).toBe(true);
    expect(started.pid).not.toBeNull();

    config = await settle(ipc.getMcpConfig());
    expect(config.running).toBe(true);
    expect(config.boundPort).toBe(5000);

    // Starting again while running mirrors the backend guard.
    const secondStart = expect(ipc.mcpServerStart()).rejects.toThrow("already running");
    await vi.runAllTimersAsync();
    await secondStart;

    const stopped = await settle(ipc.mcpServerStop());
    expect(stopped.running).toBe(false);
    config = await settle(ipc.getMcpConfig());
    expect(config.running).toBe(false);
    expect(config.boundPort).toBeNull();
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
