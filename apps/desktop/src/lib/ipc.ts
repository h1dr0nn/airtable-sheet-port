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
  connectedEmail: string | null; // linked Google account, null when disconnected
  hasClientSecret: boolean;      // secret presence only; the value never crosses IPC
};

export type GoogleConnectResult = {
  email: string;
};

export type AppSettings = {
  autoApproveWrites: boolean; // meta key 'auto_approve_writes' === '1', off by default
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
  tokenStatus(): Promise<TokenStatus>;
  getGoogleConfig(): Promise<GoogleConfig>;
  setGoogleClientId(clientId: string): Promise<void>;
  /** Stores the OAuth client secret in the OS keychain; empty string clears it. */
  setGoogleClientSecret(clientSecret: string): Promise<void>;
  /** Long-running: resolves after the user finishes the browser consent flow. */
  googleConnect(): Promise<GoogleConnectResult>;
  googleDisconnect(): Promise<void>;
  getSettings(): Promise<AppSettings>;
  /** Enabling bypasses the human confirmation gate; disabling restores it. */
  setAutoApprove(enabled: boolean): Promise<void>;
  /** Prefs-only reset: does not touch credentials, permission rules, or data. */
  resetSettings(): Promise<void>;
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
  tokenStatus: () => invoke<TokenStatus>("token_status"),
  getGoogleConfig: () => invoke<GoogleConfig>("get_google_config"),
  setGoogleClientId: (clientId) => invoke<void>("set_google_client_id", { clientId }),
  setGoogleClientSecret: (clientSecret) =>
    invoke<void>("set_google_client_secret", { clientSecret }),
  googleConnect: () => invoke<GoogleConnectResult>("google_connect"),
  googleDisconnect: () => invoke<void>("google_disconnect"),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setAutoApprove: (enabled) => invoke<void>("set_auto_approve", { enabled }),
  resetSettings: () => invoke<void>("reset_settings")
};

// Plain-browser dev preview falls back to clickable in-memory fixtures.
export const ipc: IpcApi = isTauri ? tauriIpc : demoIpc;
