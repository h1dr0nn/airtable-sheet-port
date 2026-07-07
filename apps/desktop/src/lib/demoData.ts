import type {
  AuditEvent,
  DataSource,
  PendingChange,
  TableRecord,
  TableRef,
  TableSchema
} from "@sheet-port/shared";
import type {
  AppSettings,
  AppStatus,
  CloseBehavior,
  FontFamily,
  FontScale,
  GoogleAccount,
  GoogleConfig,
  GoogleConnectResult,
  IpcApi,
  Language,
  McpClient,
  McpClientState,
  McpConfigView,
  McpTransport,
  PermissionRuleRow,
  SavePermissionRule,
  SidecarStatus,
  TablePage,
  TokenStatus
} from "./ipc.js";
import { createWorkbenchDemo } from "./workbenchDemo.js";

// In-memory stand-in for the Rust backend so `vite dev` in a plain browser is
// fully clickable. Mirrors the schema v2 empty state: a fresh database has no
// sources, rules, changes, or audit rows. Clicking Connect on the Google
// Sheets card links a fake account after a short delay so every screen stays
// explorable without OAuth.

const DEMO_LATENCY_MS = 150;
const GOOGLE_CONNECT_DELAY_MS = 1_500;
const DEFAULT_READ_LIMIT = 100;
const MAX_READ_LIMIT = 500;
const DEFAULT_AUDIT_LIMIT = 100;
const MAX_AUDIT_LIMIT = 500;
const CHANGES_LIST_LIMIT = 200;
const DEMO_MCP_PID = 48213;
const HEARTBEAT_AGE_MS = 4_000;

// Mirrors core::db defaults for the MCP sidecar config.
const DEFAULT_MCP_TRANSPORT: McpTransport = "stdio";
const DEFAULT_MCP_PORT = 4319;

// Plausible client roster for the browser preview: one installed-but-unconfigured,
// one installed-and-configured, and one absent so every dot state is visible.
type DemoClientSeed = { id: string; name: string; state: McpClientState; configPath: string | null };
const DEMO_MCP_CLIENTS: readonly DemoClientSeed[] = [
  {
    id: "claude-desktop",
    name: "Claude Desktop",
    state: "unconfigured",
    configPath: "C:\\Users\\demo\\AppData\\Roaming\\Claude\\claude_desktop_config.json"
  },
  {
    id: "cursor",
    name: "Cursor",
    state: "configured",
    configPath: "C:\\Users\\demo\\.cursor\\mcp.json"
  },
  {
    id: "vscode",
    name: "VS Code",
    state: "not_found",
    configPath: null
  }
];

// Mirrors sheet-port-core: google::GOOGLE_SOURCE_ID and the source name format.
const GOOGLE_SOURCE_ID = "google-sheets";
const DEMO_GOOGLE_EMAIL = "demo.user@gmail.com";
// Pre-filled so the Connect button is immediately clickable in the preview.
const DEMO_GOOGLE_CLIENT_ID = "000000000000-demo.apps.googleusercontent.com";

// Default UI font preferences; mirror core::db defaults (normal + modern).
const DEFAULT_FONT_SCALE: FontScale = "normal";
const DEFAULT_FONT_FAMILY: FontFamily = "modern";

// Default UI language; mirrors core::db default ("en").
const DEFAULT_LANGUAGE: Language = "en";

// Default window close behavior; mirrors core::db default ("ask").
const DEFAULT_CLOSE_BEHAVIOR: CloseBehavior = "ask";

/** Builds the "google-sheets:{accountKey}" source id from an email address. */
function sourceIdForEmail(email: string): string {
  return `${GOOGLE_SOURCE_ID}:${email.toLowerCase()}`;
}

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const delay = () => wait(DEMO_LATENCY_MS);
const nowIso = () => new Date().toISOString();

const SHEET_TABLE: TableRef = { sourceId: GOOGLE_SOURCE_ID, tableId: "leads", name: "Leads" };

const SHEET_SCHEMA: TableSchema = {
  sourceId: GOOGLE_SOURCE_ID,
  tableId: SHEET_TABLE.tableId,
  name: SHEET_TABLE.name,
  fields: [
    { name: "Name", type: "string", required: true },
    { name: "Email", type: "email" },
    { name: "Stage", type: "enum", enumValues: ["new", "qualified", "won"] },
    { name: "Value", type: "number" }
  ]
};

const SHEET_RECORDS: readonly TableRecord[] = [
  { id: "row_2", fields: { Name: "Aurora Labs", Email: "ops@auroralabs.dev", Stage: "qualified", Value: 12_000 } },
  { id: "row_3", fields: { Name: "Basalt Co", Email: "it@basalt.co", Stage: "new", Value: 3_500 } },
  { id: "row_4", fields: { Name: "Cirrus Retail", Email: "admin@cirrus.shop", Stage: "won", Value: 48_000 } },
  { id: "row_5", fields: { Name: "Drift Systems", Email: "hello@drift.example", Stage: "new", Value: 7_250 } }
];

// One pending preview appears alongside the connected source so the Changes
// and Dashboard screens stay clickable; it references only demo-real rows.
function buildSeededChange(): PendingChange {
  const before = SHEET_RECORDS[1];
  return {
    id: "chg_demo_google_update",
    sourceId: GOOGLE_SOURCE_ID,
    tableId: SHEET_TABLE.tableId,
    type: "update",
    createdAt: nowIso(),
    status: "pending",
    requiresConfirmation: true,
    diff: [
      {
        recordId: before?.id ?? "row_3",
        before: { ...before?.fields },
        after: { ...before?.fields, Stage: "qualified", Value: 9_000 }
      }
    ]
  };
}

type DemoOptions = {
  /** Override the pre-seeded client id; pass null to model a fresh install. */
  googleClientId?: string | null;
};

/** Builds an isolated demo backend; exported for tests. */
export function createDemoIpc(options: DemoOptions = {}): IpcApi {
  let googleClientId =
    options.googleClientId === undefined ? DEMO_GOOGLE_CLIENT_ID : options.googleClientId;
  // Mirrors the OS keychain: only presence is observable, never the value.
  let hasGoogleClientSecret = false;
  // Connected accounts, keyed by source id. Starts empty like a fresh backend;
  // each googleConnect adds one, googleDisconnect removes one by source id.
  let googleAccounts: GoogleAccount[] = [];
  // Rotates the fake email so a second connect models a distinct account.
  let demoConnectCount = 0;

  // App-managed preference mirror; off by default, cleared on reset.
  let autoApproveWrites = false;
  // UI font preferences; mirror the backend defaults, cleared on reset.
  let fontScale: FontScale = DEFAULT_FONT_SCALE;
  let fontFamily: FontFamily = DEFAULT_FONT_FAMILY;
  // UI language; mirrors the backend default, cleared on reset.
  let language: Language = DEFAULT_LANGUAGE;
  // Window close behavior; mirrors the backend default, cleared on reset.
  let closeBehavior: CloseBehavior = DEFAULT_CLOSE_BEHAVIOR;
  // Launch-at-login mirror; off by default in the preview.
  let autostartEnabled = false;

  // Desktop-managed HTTP sidecar: not running until mcp_server_start.
  let managedSidecarPid: number | null = null;

  // MCP sidecar config mirror; the demo sidecar is treated as always running.
  let mcpTransport: McpTransport = DEFAULT_MCP_TRANSPORT;
  let mcpPort = DEFAULT_MCP_PORT;
  // Clone the seed so configure/unregister mutate an isolated demo roster.
  let mcpClients: McpClient[] = DEMO_MCP_CLIENTS.map((client) => ({ ...client }));

  const setClientState = (id: string, state: McpClientState): McpClient => {
    const existing = mcpClients.find((client) => client.id === id);
    if (!existing) {
      throw new Error(`Unknown MCP client ${id}`);
    }
    if (existing.state === "not_found") {
      throw new Error(`${existing.name} is not installed`);
    }
    const updated: McpClient = { ...existing, state };
    mcpClients = mcpClients.map((client) => (client.id === id ? updated : client));
    return updated;
  };

  let sources: DataSource[] = [];
  let permissionRules: PermissionRuleRow[] = [];
  let nextRuleId = 1;
  let changes: PendingChange[] = [];
  let auditEvents: AuditEvent[] = [];
  let auditCounter = 0;

  const makeAuditId = () => {
    auditCounter += 1;
    return `evt_demo_${auditCounter}`;
  };

  const pushAudit = (event: Omit<AuditEvent, "id" | "timestamp">): void => {
    auditEvents = [...auditEvents, { ...event, id: makeAuditId(), timestamp: nowIso() }];
  };

  const isGoogleConnected = () => googleAccounts.length > 0;
  // The first connected account owns the demo tables/records so the Tables and
  // Changes screens stay explorable regardless of which account was added.
  const primarySourceId = () => googleAccounts[0]?.sourceId ?? null;

  const decideChange = (changeId: string, status: "approved" | "rejected"): PendingChange => {
    const existing = changes.find((change) => change.id === changeId);
    if (!existing) {
      throw new Error(`Unknown change ${changeId}`);
    }
    if (existing.status !== "pending") {
      throw new Error(`Change ${changeId} is already ${existing.status}`);
    }
    const decided: PendingChange = { ...existing, status, decidedAt: nowIso(), decidedBy: "user" };
    changes = changes.map((change) => (change.id === changeId ? decided : change));
    pushAudit({
      actor: "user",
      action: status === "approved" ? "change_approved" : "change_rejected",
      sourceId: decided.sourceId,
      tableId: decided.tableId,
      metadata: { changeId }
    });
    return decided;
  };

  const newestChangeFirst = (a: PendingChange, b: PendingChange) =>
    b.createdAt.localeCompare(a.createdAt);
  const newestEventFirst = (a: AuditEvent, b: AuditEvent) => b.timestamp.localeCompare(a.timestamp);

  // Curated Workbench tree lives in its own module to keep this file focused.
  const workbench = createWorkbenchDemo({ delay });

  return {
    ...workbench,
    async getAppStatus(): Promise<AppStatus> {
      return {
        appVersion: "0.0.1 (browser demo)",
        dbPath: "C:\\Users\\demo\\AppData\\Roaming\\sheet-port\\sheet-port.db",
        mcpRunning: true,
        mcpPid: DEMO_MCP_PID,
        mcpLastSeen: new Date(Date.now() - HEARTBEAT_AGE_MS).toISOString(),
        pendingCount: changes.filter((change) => change.status === "pending").length
      };
    },
    async listSources(): Promise<DataSource[]> {
      await delay();
      return [...sources];
    },
    async listTables(sourceId: string): Promise<TableRef[]> {
      await delay();
      return isGoogleConnected() && sourceId === GOOGLE_SOURCE_ID ? [{ ...SHEET_TABLE }] : [];
    },
    async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
      await delay();
      if (!isGoogleConnected() || sourceId !== SHEET_SCHEMA.sourceId || tableId !== SHEET_SCHEMA.tableId) {
        throw new Error(`Unknown table ${sourceId}/${tableId}`);
      }
      return SHEET_SCHEMA;
    },
    async readTable(sourceId, tableId, limit, offset): Promise<TablePage> {
      await delay();
      const isSheet =
        isGoogleConnected() && sourceId === SHEET_TABLE.sourceId && tableId === SHEET_TABLE.tableId;
      const records = isSheet ? [...SHEET_RECORDS] : [];
      const effectiveLimit = Math.min(limit ?? DEFAULT_READ_LIMIT, MAX_READ_LIMIT);
      const effectiveOffset = offset ?? 0;
      return {
        records: records.slice(effectiveOffset, effectiveOffset + effectiveLimit),
        total: records.length
      };
    },
    async listPermissionRules(): Promise<PermissionRuleRow[]> {
      await delay();
      return [...permissionRules];
    },
    async savePermissionRule(rule: SavePermissionRule): Promise<PermissionRuleRow> {
      await delay();
      if (rule.sourceId.trim() === "") {
        throw new Error("sourceId is required");
      }
      const matchesPair = (row: PermissionRuleRow) =>
        row.sourceId === rule.sourceId && row.tableId === rule.tableId;
      const existing = rule.id !== null
        ? permissionRules.find((row) => row.id === rule.id)
        : permissionRules.find(matchesPair);
      if (rule.id !== null && !existing) {
        throw new Error(`Unknown permission rule ${rule.id}`);
      }
      const saved: PermissionRuleRow = {
        id: existing ? existing.id : nextRuleId++,
        sourceId: rule.sourceId,
        tableId: rule.tableId,
        read: rule.read,
        write: rule.write,
        deleteRecords: rule.deleteRecords,
        requireConfirmationFor: [...rule.requireConfirmationFor],
        updatedAt: nowIso()
      };
      permissionRules = existing
        ? permissionRules.map((row) => (row.id === saved.id ? saved : row))
        : [...permissionRules, saved];
      pushAudit({
        actor: "user",
        action: "permission_rule_saved",
        sourceId: saved.sourceId,
        tableId: saved.tableId ?? undefined,
        metadata: { ...saved }
      });
      return saved;
    },
    async deletePermissionRule(id: number): Promise<void> {
      await delay();
      const existing = permissionRules.find((row) => row.id === id);
      if (!existing) {
        throw new Error(`Unknown permission rule ${id}`);
      }
      permissionRules = permissionRules.filter((row) => row.id !== id);
      pushAudit({
        actor: "user",
        action: "permission_rule_deleted",
        sourceId: existing.sourceId,
        tableId: existing.tableId ?? undefined,
        metadata: { id }
      });
    },
    async listChanges(status: string | null): Promise<PendingChange[]> {
      await delay();
      const filtered = status === null ? changes : changes.filter((change) => change.status === status);
      return [...filtered].sort(newestChangeFirst).slice(0, CHANGES_LIST_LIMIT);
    },
    async approveChange(changeId: string): Promise<PendingChange> {
      await delay();
      return decideChange(changeId, "approved");
    },
    async rejectChange(changeId: string): Promise<PendingChange> {
      await delay();
      return decideChange(changeId, "rejected");
    },
    async listAuditEvents(limit, offset): Promise<AuditEvent[]> {
      await delay();
      const effectiveLimit = Math.min(limit ?? DEFAULT_AUDIT_LIMIT, MAX_AUDIT_LIMIT);
      const effectiveOffset = offset ?? 0;
      return [...auditEvents]
        .sort(newestEventFirst)
        .slice(effectiveOffset, effectiveOffset + effectiveLimit);
    },
    async clearAuditLog(): Promise<void> {
      await delay();
      // Mirrors clear_audit_log: wipe first, then record a single trace event
      // AFTER, so a freshly cleared log holds exactly this one entry.
      auditEvents = [];
      pushAudit({ actor: "user", action: "audit_cleared" });
    },
    async tokenStatus(): Promise<TokenStatus> {
      await delay();
      return { googleSheets: isGoogleConnected(), provider: false };
    },
    async getGoogleConfig(): Promise<GoogleConfig> {
      await delay();
      return {
        clientId: googleClientId,
        hasClientSecret: hasGoogleClientSecret
      };
    },
    async googleListAccounts(): Promise<GoogleAccount[]> {
      await delay();
      return googleAccounts.map((account) => ({ ...account }));
    },
    async setGoogleClientId(clientId: string): Promise<void> {
      await delay();
      const trimmed = clientId.trim();
      if (trimmed === "") {
        throw new Error("Google client ID must not be empty");
      }
      googleClientId = trimmed;
    },
    async setGoogleClientSecret(clientSecret: string): Promise<void> {
      await delay();
      // Mirrors core::google::set_client_secret: empty string clears the entry.
      hasGoogleClientSecret = clientSecret !== "";
    },
    async googleConnect(): Promise<GoogleConnectResult> {
      if (googleClientId === null) {
        throw new Error("Google client ID is not configured. Set it in the desktop app settings");
      }
      // Stands in for the real browser consent round-trip.
      await wait(GOOGLE_CONNECT_DELAY_MS);
      const wasConnected = isGoogleConnected();
      // First account owns the demo tables under the bare source id (mirrors the
      // legacy "default" account); later accounts get a distinct email + id.
      const email = wasConnected
        ? `demo.user+${demoConnectCount + 1}@gmail.com`
        : DEMO_GOOGLE_EMAIL;
      demoConnectCount += 1;
      const sourceId = wasConnected ? sourceIdForEmail(email) : GOOGLE_SOURCE_ID;
      // Re-linking the same account is idempotent, like the real backend.
      if (!googleAccounts.some((account) => account.sourceId === sourceId)) {
        googleAccounts = [...googleAccounts, { sourceId, email }];
        sources = [
          ...sources.filter((source) => source.id !== sourceId),
          {
            id: sourceId,
            kind: "google_sheets",
            name: `Google Sheets (${email})`,
            status: "connected"
          }
        ];
      }
      pushAudit({
        actor: "user",
        action: "google_connected",
        sourceId,
        metadata: { email }
      });
      if (!wasConnected && !changes.some((change) => change.sourceId === GOOGLE_SOURCE_ID)) {
        const seeded = buildSeededChange();
        changes = [...changes, seeded];
        pushAudit({
          actor: "agent",
          action: "preview_update_records",
          sourceId: seeded.sourceId,
          tableId: seeded.tableId,
          metadata: { changeId: seeded.id, records: 1 }
        });
      }
      return { email };
    },
    async googleDisconnect(sourceId: string): Promise<void> {
      await delay();
      // Idempotent, like core::google::disconnect: removing an absent account is
      // a no-op rather than an error.
      googleAccounts = googleAccounts.filter((account) => account.sourceId !== sourceId);
      sources = sources.filter((source) => source.id !== sourceId);
      pushAudit({ actor: "user", action: "google_disconnected", sourceId });
    },
    async getSettings(): Promise<AppSettings> {
      await delay();
      return { autoApproveWrites, fontScale, fontFamily, language, closeBehavior };
    },
    async setAutoApprove(enabled: boolean): Promise<void> {
      await delay();
      autoApproveWrites = enabled;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "auto_approve_writes", enabled }
      });
    },
    async setFontScale(scale: FontScale): Promise<void> {
      await delay();
      fontScale = scale;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "ui_font_scale", scale }
      });
    },
    async setFontFamily(family: FontFamily): Promise<void> {
      await delay();
      fontFamily = family;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "ui_font_family", family }
      });
    },
    async setLanguage(next: Language): Promise<void> {
      await delay();
      language = next;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "ui_language", value: next }
      });
    },
    async setCloseBehavior(behavior: CloseBehavior): Promise<void> {
      await delay();
      closeBehavior = behavior;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "close_behavior", behavior }
      });
    },
    async windowHideToTray(): Promise<void> {
      await delay();
      // No real window in the browser preview; record the intent for parity.
      pushAudit({ actor: "user", action: "window_hidden_to_tray" });
    },
    async windowQuit(): Promise<void> {
      await delay();
      // No-op in the browser preview; the real backend exits the process.
      pushAudit({ actor: "user", action: "window_quit" });
    },
    async getAutostartEnabled(): Promise<boolean> {
      await delay();
      return autostartEnabled;
    },
    async setAutostartEnabled(enabled: boolean): Promise<void> {
      await delay();
      autostartEnabled = enabled;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "autostart", enabled }
      });
    },
    async resetSettings(): Promise<void> {
      await delay();
      // Prefs-only: mirrors reset_settings clearing the app-managed meta keys.
      autoApproveWrites = false;
      fontScale = DEFAULT_FONT_SCALE;
      fontFamily = DEFAULT_FONT_FAMILY;
      language = DEFAULT_LANGUAGE;
      closeBehavior = DEFAULT_CLOSE_BEHAVIOR;
      pushAudit({ actor: "user", action: "settings_reset" });
    },
    async getMcpConfig(): Promise<McpConfigView> {
      await delay();
      // stdio: clients spawn the sidecar themselves, so it reads as running.
      // http: only the desktop-managed child counts, toggled by start/stop.
      const running = mcpTransport === "http" ? managedSidecarPid !== null : true;
      return {
        transport: mcpTransport,
        port: mcpPort,
        running,
        boundPort: mcpTransport === "http" && running ? mcpPort : null
      };
    },
    async setMcpTransport(transport: McpTransport): Promise<void> {
      await delay();
      mcpTransport = transport;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "mcp_transport", transport }
      });
    },
    async setMcpPort(port: number): Promise<void> {
      await delay();
      // Mirrors set_mcp_port validation so the preview rejects the same values.
      if (!Number.isInteger(port) || port < 1024 || port > 65_535) {
        throw new Error("Port must be an integer between 1024 and 65535");
      }
      mcpPort = port;
      pushAudit({
        actor: "user",
        action: "settings_updated",
        metadata: { key: "mcp_port", port }
      });
    },
    async mcpDetectClients(): Promise<McpClient[]> {
      await delay();
      return mcpClients.map((client) => ({ ...client }));
    },
    async mcpConfigureClient(id: string): Promise<void> {
      await delay();
      const updated = setClientState(id, "configured");
      pushAudit({
        actor: "user",
        action: "mcp_client_configured",
        metadata: { client: updated.id }
      });
    },
    async mcpUnregisterClient(id: string): Promise<void> {
      await delay();
      const updated = setClientState(id, "unconfigured");
      pushAudit({
        actor: "user",
        action: "mcp_client_unregistered",
        metadata: { client: updated.id }
      });
    },
    async mcpConfigureAll(): Promise<void> {
      await delay();
      // Only installed clients are touched; absent ones stay not_found.
      mcpClients = mcpClients.map((client) =>
        client.state === "unconfigured" ? { ...client, state: "configured" } : client
      );
      pushAudit({ actor: "user", action: "mcp_clients_configured_all" });
    },
    async mcpServerStart(): Promise<SidecarStatus> {
      await delay();
      // Mirrors the backend guard: only one managed child may run at a time.
      if (managedSidecarPid !== null) {
        throw new Error("The MCP server is already running");
      }
      managedSidecarPid = DEMO_MCP_PID;
      pushAudit({
        actor: "user",
        action: "mcp_server_started",
        metadata: { pid: managedSidecarPid, port: mcpPort, transport: "http" }
      });
      return { running: true, pid: managedSidecarPid };
    },
    async mcpServerStop(): Promise<SidecarStatus> {
      await delay();
      // Idempotent, like the backend: stopping when nothing runs is fine.
      const stoppedPid = managedSidecarPid;
      managedSidecarPid = null;
      if (stoppedPid !== null) {
        pushAudit({
          actor: "user",
          action: "mcp_server_stopped",
          metadata: { pid: stoppedPid }
        });
      }
      return { running: false, pid: null };
    }
  };
}

// Plain-browser dev preview instance shared by lib/ipc.ts.
export const demoIpc: IpcApi = createDemoIpc();
