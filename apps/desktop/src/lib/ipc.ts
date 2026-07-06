import { invoke } from "@tauri-apps/api/core";
import type {
  AuditEvent,
  DataSource,
  PendingChange,
  TableRecord,
  TableRef,
  TableSchema
} from "@sheet-port/shared";
import { demoIpc } from "./demoData.js";

// Types below are copied verbatim from docs/ipc.md. Do not edit them here
// without updating the contract document first.

export type AppStatus = {
  appVersion: string;
  dbPath: string;
  mcpRunning: boolean;      // any mcp_heartbeat row with last_seen within 30s
  mcpPid: number | null;
  mcpLastSeen: string | null; // ISO timestamp
  pendingCount: number;     // pending_changes WHERE status = 'pending'
};

export type TablePage = {
  records: TableRecord[]; // ordered by position
  total: number;          // total record count ignoring limit/offset
};

export type PermissionRuleRow = {
  id: number;
  sourceId: string;
  tableId: string | null;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: string[]; // ConfirmationAction[]
  updatedAt: string;
};

export type SavePermissionRule = {
  id: number | null;      // null -> insert, else update by id
  sourceId: string;
  tableId: string | null;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: string[];
};

export type TokenStatus = {
  googleSheets: boolean; // OS keychain entry exists (service "sheet-port", user "google_sheets")
  provider: boolean;     // ... user "provider"
};

export type GoogleConfig = {
  clientId: string | null;       // OAuth desktop client id from Settings, null until saved
  hasClientSecret: boolean;      // secret presence only; the value never crosses IPC
};

/** One connected Google account, as returned by google_list_accounts. */
export type GoogleAccount = {
  sourceId: string; // "google-sheets:{accountKey}" source row id
  email: string;
};

export type McpTransport = "stdio" | "http";

export type McpConfigView = {
  transport: McpTransport; // meta key 'mcp_transport', default 'stdio'
  port: number;            // meta key 'mcp_port', default 4319, range 1024-65535
  running: boolean;        // fresh heartbeat exists right now
  boundPort: number | null; // configured port when running AND http, else null
};

/** Whether a known MCP client is installed and, if so, its config state. */
export type McpClientState =
  | "configured"    // client config already points at this sidecar
  | "unconfigured"  // client is installed but not registered yet
  | "not_found";    // client is not installed on this machine

/** One MCP client the desktop can auto-configure (Claude Desktop, Cursor, ...). */
export type McpClient = {
  id: string;                 // stable client id, e.g. "claude-desktop"
  name: string;               // display name, e.g. "Claude Desktop"
  state: McpClientState;
  /** Resolved config file the desktop would edit, null when not found. */
  configPath: string | null;
};

/** Raw shape returned by the Rust mcp_detect_clients command. */
type RawDetectedClient = {
  id: string;
  displayName: string;
  installed: boolean;
  configured: boolean;
  detectable: boolean;
  configPath?: string | null;
};

/** Collapses the backend booleans into the single UI state. */
function toMcpClient(raw: RawDetectedClient): McpClient {
  const state: McpClientState = raw.configured
    ? "configured"
    : raw.installed
      ? "unconfigured"
      : "not_found";
  return { id: raw.id, name: raw.displayName, state, configPath: raw.configPath ?? null };
}

export type GoogleConnectResult = {
  email: string;
};

export type FontScale = "small" | "normal" | "large";
export type FontFamily = "classic" | "modern" | "system";

export type AppSettings = {
  autoApproveWrites: boolean; // meta key 'auto_approve_writes' === '1', off by default
  fontScale: FontScale;       // meta key 'ui_font_scale', 'normal' by default
  fontFamily: FontFamily;     // meta key 'ui_font_family', 'modern' by default
};

/** Managed-sidecar status returned by mcp_server_start / mcp_server_stop. */
export type SidecarStatus = {
  running: boolean;
  pid: number | null;
};

/** Every Tauri command from docs/ipc.md, typed. */
export interface IpcApi {
  getAppStatus(): Promise<AppStatus>;
  listSources(): Promise<DataSource[]>;
  listTables(sourceId: string): Promise<TableRef[]>;
  describeTable(sourceId: string, tableId: string): Promise<TableSchema>;
  readTable(
    sourceId: string,
    tableId: string,
    limit: number | null,
    offset: number | null
  ): Promise<TablePage>;
  listPermissionRules(): Promise<PermissionRuleRow[]>;
  savePermissionRule(rule: SavePermissionRule): Promise<PermissionRuleRow>;
  deletePermissionRule(id: number): Promise<void>;
  listChanges(status: string | null): Promise<PendingChange[]>;
  approveChange(changeId: string): Promise<PendingChange>;
  rejectChange(changeId: string): Promise<PendingChange>;
  listAuditEvents(limit: number | null, offset: number | null): Promise<AuditEvent[]>;
  /** Wipes the audit log, then records a single `audit_cleared` trace event. */
  clearAuditLog(): Promise<void>;
  tokenStatus(): Promise<TokenStatus>;
  getGoogleConfig(): Promise<GoogleConfig>;
  /** Every connected Google account (sourceId + email), ordered by source id. */
  googleListAccounts(): Promise<GoogleAccount[]>;
  setGoogleClientId(clientId: string): Promise<void>;
  /** Stores the OAuth client secret in the OS keychain; empty string clears it. */
  setGoogleClientSecret(clientSecret: string): Promise<void>;
  /** Long-running: resolves after the user finishes the browser consent flow. */
  googleConnect(): Promise<GoogleConnectResult>;
  /** Removes one connected account by its source id. Idempotent. */
  googleDisconnect(sourceId: string): Promise<void>;
  getSettings(): Promise<AppSettings>;
  /** Enabling bypasses the human confirmation gate; disabling restores it. */
  setAutoApprove(enabled: boolean): Promise<void>;
  /** Persists the UI font-size scale preference. */
  setFontScale(scale: FontScale): Promise<void>;
  /** Persists the UI font-family preference. */
  setFontFamily(family: FontFamily): Promise<void>;
  /** Prefs-only reset: does not touch credentials, permission rules, or data. */
  resetSettings(): Promise<void>;
  /** Persisted transport/port plus the live sidecar heartbeat state. */
  getMcpConfig(): Promise<McpConfigView>;
  /** Persists the transport choice; takes effect after a sidecar restart. */
  setMcpTransport(transport: McpTransport): Promise<void>;
  /** Persists the HTTP port (1024-65535); takes effect after a sidecar restart. */
  setMcpPort(port: number): Promise<void>;
  /** Scans the machine for known MCP clients and their config state. */
  mcpDetectClients(): Promise<McpClient[]>;
  /** Writes this sidecar into the given client's config file. */
  mcpConfigureClient(id: string): Promise<void>;
  /** Removes this sidecar from the given client's config file. */
  mcpUnregisterClient(id: string): Promise<void>;
  /** Configures every currently detected (installed) client at once. */
  mcpConfigureAll(): Promise<void>;
  /** Starts the desktop-managed HTTP sidecar; errors if one already runs. */
  mcpServerStart(): Promise<SidecarStatus>;
  /** Stops the desktop-managed sidecar if one is running. Idempotent. */
  mcpServerStop(): Promise<SidecarStatus>;
}

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const tauriIpc: IpcApi = {
  getAppStatus: () => invoke<AppStatus>("get_app_status"),
  listSources: () => invoke<DataSource[]>("list_sources"),
  listTables: (sourceId) => invoke<TableRef[]>("list_tables", { sourceId }),
  describeTable: (sourceId, tableId) =>
    invoke<TableSchema>("describe_table", { sourceId, tableId }),
  readTable: (sourceId, tableId, limit, offset) =>
    invoke<TablePage>("read_table", { sourceId, tableId, limit, offset }),
  listPermissionRules: () => invoke<PermissionRuleRow[]>("list_permission_rules"),
  savePermissionRule: (rule) => invoke<PermissionRuleRow>("save_permission_rule", { rule }),
  deletePermissionRule: (id) => invoke<void>("delete_permission_rule", { id }),
  listChanges: (status) => invoke<PendingChange[]>("list_changes", { status }),
  approveChange: (changeId) => invoke<PendingChange>("approve_change", { changeId }),
  rejectChange: (changeId) => invoke<PendingChange>("reject_change", { changeId }),
  listAuditEvents: (limit, offset) =>
    invoke<AuditEvent[]>("list_audit_events", { limit, offset }),
  clearAuditLog: () => invoke<void>("clear_audit_log"),
  tokenStatus: () => invoke<TokenStatus>("token_status"),
  getGoogleConfig: () => invoke<GoogleConfig>("get_google_config"),
  googleListAccounts: () => invoke<GoogleAccount[]>("google_list_accounts"),
  setGoogleClientId: (clientId) => invoke<void>("set_google_client_id", { clientId }),
  setGoogleClientSecret: (clientSecret) =>
    invoke<void>("set_google_client_secret", { clientSecret }),
  googleConnect: () => invoke<GoogleConnectResult>("google_connect"),
  googleDisconnect: (sourceId) => invoke<void>("google_disconnect", { sourceId }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setAutoApprove: (enabled) => invoke<void>("set_auto_approve", { enabled }),
  setFontScale: (scale) => invoke<void>("set_font_scale", { scale }),
  setFontFamily: (family) => invoke<void>("set_font_family", { family }),
  resetSettings: () => invoke<void>("reset_settings"),
  getMcpConfig: () => invoke<McpConfigView>("get_mcp_config"),
  setMcpTransport: (transport) => invoke<void>("set_mcp_transport", { transport }),
  setMcpPort: (port) => invoke<void>("set_mcp_port", { port }),
  mcpDetectClients: async () => {
    // The Rust command reports raw booleans; the UI works with a single state.
    const raw = await invoke<RawDetectedClient[]>("mcp_detect_clients");
    return raw.map(toMcpClient);
  },
  mcpConfigureClient: (id) => invoke<void>("mcp_configure_client", { id }),
  mcpUnregisterClient: (id) => invoke<void>("mcp_unregister_client", { id }),
  mcpConfigureAll: () => invoke<void>("mcp_configure_all"),
  mcpServerStart: () => invoke<SidecarStatus>("mcp_server_start"),
  mcpServerStop: () => invoke<SidecarStatus>("mcp_server_stop")
};

// Plain-browser dev preview falls back to clickable in-memory fixtures.
export const ipc: IpcApi = isTauri ? tauriIpc : demoIpc;
