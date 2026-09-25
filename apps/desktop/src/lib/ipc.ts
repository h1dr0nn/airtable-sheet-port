import { invoke } from "@tauri-apps/api/core";
import type {
  AuditEvent,
  DataSource,
  TableRecord,
  TableRef,
  TableSchema
} from "@sheet-port/shared";

// Types below are copied verbatim from docs/ipc.md. Do not edit them here
// without updating the contract document first.

export type AppStatus = {
  appVersion: string;
  dbPath: string;
  mcpRunning: boolean;      // any mcp_heartbeat row with last_seen within 30s
  mcpPid: number | null;
  mcpLastSeen: string | null; // ISO timestamp
  sidecars: SidecarHeartbeat[];  // every fresh heartbeat row, newest first
};

export type SidecarHeartbeat = {
  pid: number;
  version: string | null;     // sidecar package version; null for sidecars that predate it
  lastSeen: string;           // ISO timestamp
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
  updatedAt: string;
};

export type SavePermissionRule = {
  id: number | null;      // null -> insert, else update by id
  sourceId: string;
  tableId: string | null;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
};

export type TokenStatus = {
  googleSheets: boolean; // at least one Google account is connected (a keyed
                         // 'google-sheets:{accountKey}' source row exists)
};

/** One connected Google account (one Apps Script bridge). The secret and
 * tokens never cross IPC. */
export type GoogleAccount = {
  sourceId: string;     // "google-sheets:{accountKey}" source row id
  email: string;        // the Google account the bridge executes as
  deploymentId: string; // Apps Script deployment id; "" when the credential is missing
  bridgeUrl: string;    // canonical web app URL; "" when the credential is missing
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

export type FontScale = "small" | "normal" | "large";
export type FontFamily = "classic" | "modern" | "system";
export type Language = "en" | "vi";

export type AppSettings = {
  fontScale: FontScale;       // meta key 'ui_font_scale', 'normal' by default
  fontFamily: FontFamily;     // meta key 'ui_font_family', 'modern' by default
  language: Language;         // meta key 'ui_language', 'en' by default
};

/** Managed-sidecar status returned by mcp_server_start / mcp_server_stop. */
export type SidecarStatus = {
  running: boolean;
  pid: number | null;
};

// ---------------------------------------------------------------------------
// Workbench (Google-Sheets-like curated workspace)
//
// The Workbench is a user-curated tree of spreadsheets grouped into folders,
// distinct from the raw `list_tables` read path. The Tauri path invokes the
// Rust workbench commands; the browser preview falls back to the in-memory demo.
// ---------------------------------------------------------------------------

/** A user-created folder that groups spreadsheets in the Workbench tree. */
export type WorkbenchFolder = {
  id: string;
  name: string;
  position: number; // ascending sort order within the tree
};

/** One spreadsheet the user has added to the Workbench, optionally foldered. */
export type WorkbenchItem = {
  id: string;
  folderId: string | null; // null -> shown under "Ungrouped"
  sourceId: string; // owning data source (e.g. a Google account)
  spreadsheetId: string; // provider spreadsheet id
  name: string; // resolved display name
  position: number; // ascending sort order within its folder
};

/** One tab (sheet) inside a spreadsheet, like a Google Sheets bottom tab. */
export type SheetTab = {
  gid: string; // provider sheet id (gid)
  title: string;
  index: number; // tab order, left to right
};

/** A rectangular block of string cells for one sheet tab (v1: string cells). */
export type GridData = {
  columns: { id: string; title: string }[];
  rows: Record<string, string>[]; // each row keyed by column id
  totalRows: number; // total rows ignoring limit/offset
};

/** Input for adding a spreadsheet: a pasted URL or bare id plus a target folder. */
export type AddSpreadsheetInput = {
  folderId: string | null;
  urlOrId: string;
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
  listAuditEvents(limit: number | null, offset: number | null): Promise<AuditEvent[]>;
  /** Wipes the audit log, then records a single `audit_cleared` trace event.
   * `listAuditEvents` never returns that event, so the feed reads as empty. */
  clearAuditLog(): Promise<void>;
  tokenStatus(): Promise<TokenStatus>;
  /** Every connected Google account (one per bridge), ordered by source id. */
  googleListAccounts(): Promise<GoogleAccount[]>;
  /** Adds (or replaces) an Apps Script bridge; calls it once to learn the email. */
  googleAddBridge(url: string, secret: string): Promise<GoogleAccount>;
  /** Removes one bridge by its source id. Idempotent. */
  googleRemoveBridge(sourceId: string): Promise<void>;
  /** Forces a fresh token fetch from one bridge. */
  googleTestBridge(sourceId: string): Promise<GoogleAccount>;
  getSettings(): Promise<AppSettings>;
  /** Persists the UI font-size scale preference. */
  setFontScale(scale: FontScale): Promise<void>;
  /** Persists the UI font-family preference. */
  setFontFamily(family: FontFamily): Promise<void>;
  /** Persists the UI language preference ("en" | "vi"). */
  setLanguage(language: Language): Promise<void>;
  /** Whether launch-at-login (autostart) is currently enabled. */
  getAutostartEnabled(): Promise<boolean>;
  /** Enables or disables launch-at-login (autostart). */
  setAutostartEnabled(enabled: boolean): Promise<void>;
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

  // --- Workbench ---
  /** The full curated tree: every folder plus every added spreadsheet. */
  workbenchTree(): Promise<{ folders: WorkbenchFolder[]; items: WorkbenchItem[] }>;
  /** Creates a folder at the end of the tree; the name must not be empty. */
  createFolder(name: string): Promise<WorkbenchFolder>;
  /** Renames a folder in place. */
  renameFolder(id: string, name: string): Promise<void>;
  /** Deletes a folder; its spreadsheets fall back to Ungrouped (folderId null). */
  deleteFolder(id: string): Promise<void>;
  /** Resolves a pasted URL/id into a spreadsheet and adds it to the tree. */
  addSpreadsheet(input: AddSpreadsheetInput): Promise<WorkbenchItem>;
  /** Removes one spreadsheet from the Workbench (does not touch the source). */
  removeWorkbenchItem(id: string): Promise<void>;
  /** Moves a spreadsheet to another folder, or to Ungrouped when null. */
  moveWorkbenchItem(id: string, folderId: string | null): Promise<void>;
  /** Lists the sheet tabs of one added spreadsheet, left to right. */
  listSheetTabs(itemId: string): Promise<SheetTab[]>;
  /** Reads a page of one sheet tab as string cells. */
  readSheet(
    itemId: string,
    gid: string,
    limit: number | null,
    offset: number | null
  ): Promise<GridData>;
  /** Writes a single cell; the row must already exist. */
  updateCell(
    itemId: string,
    gid: string,
    rowIndex: number,
    columnId: string,
    value: string
  ): Promise<void>;
  /** Appends a row to a sheet tab, returning its new zero-based row index. */
  appendSheetRow(
    itemId: string,
    gid: string,
    values: Record<string, string>
  ): Promise<{ rowIndex: number }>;
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
  listAuditEvents: (limit, offset) =>
    invoke<AuditEvent[]>("list_audit_events", { limit, offset }),
  clearAuditLog: () => invoke<void>("clear_audit_log"),
  tokenStatus: () => invoke<TokenStatus>("token_status"),
  googleListAccounts: () => invoke<GoogleAccount[]>("google_list_accounts"),
  googleAddBridge: (url, secret) => invoke<GoogleAccount>("google_add_bridge", { url, secret }),
  googleRemoveBridge: (sourceId) => invoke<void>("google_remove_bridge", { sourceId }),
  googleTestBridge: (sourceId) => invoke<GoogleAccount>("google_test_bridge", { sourceId }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setFontScale: (scale) => invoke<void>("set_font_scale", { scale }),
  setFontFamily: (family) => invoke<void>("set_font_family", { family }),
  setLanguage: (language) => invoke<void>("set_language", { language }),
  getAutostartEnabled: () => invoke<boolean>("get_autostart_enabled"),
  setAutostartEnabled: (enabled) => invoke<void>("set_autostart_enabled", { enabled }),
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
  mcpServerStop: () => invoke<SidecarStatus>("mcp_server_stop"),
  // Workbench: wired to the Rust workbench commands (see src-tauri/commands.rs).
  workbenchTree: () =>
    invoke<{ folders: WorkbenchFolder[]; items: WorkbenchItem[] }>("workbench_tree"),
  createFolder: (name) => invoke<WorkbenchFolder>("create_workbench_folder", { name }),
  renameFolder: (id, name) => invoke<void>("rename_workbench_folder", { id, name }),
  deleteFolder: (id) => invoke<void>("delete_workbench_folder", { id }),
  addSpreadsheet: (input) =>
    invoke<WorkbenchItem>("add_workbench_spreadsheet", {
      folderId: input.folderId,
      urlOrId: input.urlOrId
    }),
  removeWorkbenchItem: (id) => invoke<void>("remove_workbench_item", { id }),
  moveWorkbenchItem: (id, folderId) => invoke<void>("move_workbench_item", { id, folderId }),
  listSheetTabs: (itemId) => invoke<SheetTab[]>("list_workbench_sheet_tabs", { itemId }),
  readSheet: (itemId, gid, limit, offset) =>
    invoke<GridData>("read_workbench_sheet", { itemId, gid, limit, offset }),
  updateCell: (itemId, gid, rowIndex, columnId, value) =>
    invoke<void>("update_workbench_cell", { itemId, gid, rowIndex, columnId, value }),
  appendSheetRow: (itemId, gid, values) =>
    invoke<{ rowIndex: number }>("append_workbench_row", { itemId, gid, values })
};

/**
 * In-memory demo backend for the plain-browser `vite dev` preview, loaded on
 * first call. Every IpcApi method returns a promise, so each one can await the
 * lazily imported module before delegating.
 */
function createLazyDemoIpc(): IpcApi {
  let demo: Promise<IpcApi> | null = null;
  const load = () => (demo ??= import("./demoData.js").then((module) => module.demoIpc));
  return new Proxy({} as IpcApi, {
    get: (_target, key) => (...args: unknown[]) =>
      load().then((api) => {
        const method = api[key as keyof IpcApi] as (...params: unknown[]) => Promise<unknown>;
        return method(...args);
      })
  });
}

// The demo exists only in Vite dev mode: `import.meta.env.DEV` is statically
// false in production builds, so the demo modules are dropped from the bundle.
export const ipc: IpcApi = isTauri || !import.meta.env.DEV ? tauriIpc : createLazyDemoIpc();
