# Desktop IPC Contract (Tauri Commands)

Contract between the Rust backend (`apps/desktop/src-tauri`) and the React frontend.
Both sides MUST match this document exactly. All commands return `Result<T, String>`;
the error string is user-displayable. All Rust structs serialize with
`#[serde(rename_all = "camelCase")]` so JSON field names match the TypeScript types
in `@sheet-port/shared`.

## Shared state model

The desktop app and the Rust MCP sidecar (`crates/sheet-port-mcp`) share one SQLite database (WAL mode):

- Path: `%APPDATA%/sheet-port/sheet-port.db` (Windows), `~/Library/Application Support/sheet-port/sheet-port.db` (macOS), `$XDG_DATA_HOME/sheet-port/sheet-port.db` or `~/.local/share/sheet-port/sheet-port.db` (Linux).
- Env override: `SHEET_PORT_DB` (absolute file path) - used by tests and smoke scripts.
- Schema: `crates/sheet-port-core/sql/schema.sql`, seed: `crates/sheet-port-core/sql/seed.sql`. Both are embedded once via `include_str!` in the shared core crate, so the desktop app and the sidecar apply identical SQL. Whichever process opens the DB first applies schema + seed (the schema is idempotent; the seed is guarded by the `meta` key `seeded`).
- Connection pragmas: `journal_mode=WAL`, `busy_timeout=5000`, `foreign_keys=ON`.

## Commands

### `get_app_status() -> AppStatus`

```ts
type AppStatus = {
  appVersion: string;
  dbPath: string;
  mcpRunning: boolean;      // any mcp_heartbeat row with last_seen within 30s
  mcpPid: number | null;
  mcpLastSeen: string | null; // ISO timestamp
  pendingCount: number;     // pending_changes WHERE status = 'pending'
};
```

### `list_sources() -> DataSource[]`

Rows from `sources`, mapped to `DataSource` (id, kind, name, status).

### `list_tables(sourceId: string) -> TableRef[]`

Routed through the `ConnectorRegistry` by the source's kind (mock ->
`mock_tables`, google_sheets -> Drive spreadsheet listing). Unknown
sourceId -> `Ok([])`.

### `describe_table(sourceId: string, tableId: string) -> TableSchema`

Routed through the `ConnectorRegistry` (mock -> `mock_tables.fields` JSON,
google_sheets -> header row + inferred types). Unknown table ->
`Err("Unknown ... table ...")`.

### `read_table(sourceId: string, tableId: string, limit: number | null, offset: number | null) -> TablePage`

```ts
type TablePage = {
  records: TableRecord[]; // ordered by position (mock) / sheet row (google)
  total: number;          // total record count ignoring limit/offset
};
```

Routed through the `ConnectorRegistry`. Default limit 100, clamp 1..=500,
offset floors at 0.

### `list_permission_rules() -> PermissionRuleRow[]`

```ts
type PermissionRuleRow = {
  id: number;
  sourceId: string;
  tableId: string | null;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: string[]; // ConfirmationAction[]
  updatedAt: string;
};
```

### `save_permission_rule(rule: SavePermissionRule) -> PermissionRuleRow`

```ts
type SavePermissionRule = {
  id: number | null;      // null -> insert, else update by id
  sourceId: string;
  tableId: string | null;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: string[];
};
```

Upsert honoring `UNIQUE(source_id, table_id)`. Writes an audit event
(`actor='user'`, `action='permission_rule_saved'`, metadata = rule snapshot).

### `delete_permission_rule(id: number) -> void`

Writes audit event `permission_rule_deleted`.

### `list_changes(status: string | null) -> PendingChange[]`

`PendingChange` from `@sheet-port/shared` (diff = parsed JSON; `payload` is NEVER
returned). `status = null` -> all, newest first, limit 200.

### `approve_change(changeId: string) -> PendingChange`

Transition `pending -> approved` only (else `Err`). Sets `decided_at` (now, ISO),
`decided_by='user'`. Audit event `change_approved` (actor user).

### `reject_change(changeId: string) -> PendingChange`

Transition `pending -> rejected` only. Same bookkeeping, audit `change_rejected`.

Note: commit stays agent-side (`commit_change` MCP tool). The desktop only decides.

### `list_audit_events(limit: number | null, offset: number | null) -> AuditEvent[]`

Newest first. Default limit 100, max 500.

### `clear_audit_log() -> void`

Deletes every row in `audit_events`, then records a single `audit_cleared`
event (`actor='user'`, no source/table/metadata) AFTER the wipe so the clear
itself leaves a trace. A freshly cleared log therefore holds exactly this one
event.

### `token_status() -> TokenStatus`

```ts
type TokenStatus = {
  googleSheets: boolean; // at least one Google account is connected (a keyed
                         // 'google-sheets:{accountKey}' source row exists)
  provider: boolean;     // OS keychain entry exists (service "sheet-port", user "provider")
};
```

`googleSheets` reflects whether any Google account is connected. The OS keychain
cannot be enumerated, so account presence is derived from the `sources` table,
which the connect/disconnect flow keeps in lockstep with the keychain entries.
No tokens are ever returned to the frontend or agents.

## Settings (app-managed preferences)

App-managed preferences live in the shared `meta` table so both processes see
them. Frontend-only prefs (e.g. theme, kept in `localStorage`) are NOT part of
this contract and are not reset by `reset_settings`.

### `get_settings() -> AppSettings`

```ts
type AppSettings = {
  autoApproveWrites: boolean;                 // meta key 'auto_approve_writes' === '1'
  fontScale: "small" | "normal" | "large";    // meta key 'ui_font_scale', default 'normal'
  fontFamily: "classic" | "modern" | "system"; // meta key 'ui_font_family', default 'modern'
  language: "en" | "vi";                       // meta key 'ui_language', default 'en'
};
```

`fontScale` / `fontFamily` / `language` are appearance preferences the frontend
applies to the UI. Absent (or out-of-contract) meta values read back as their
defaults.

### `set_auto_approve(enabled: boolean) -> void`

Enabling writes meta `auto_approve_writes = '1'`; disabling deletes the key so
it reads back as the absent default. When on, the commit path treats a
`requires_confirmation` change as policy-approved and bypasses the human
confirmation gate (see `docs/security.md`). Audit event (`actor='user'`,
`action='settings_updated'`, metadata `{key:'auto_approve_writes', enabled}`).

### `set_font_scale(scale: "small" | "normal" | "large") -> void`

Persists `ui_font_scale`. Rejects any other value with a clear error. Audit
event (`actor='user'`, `action='settings_updated'`, metadata
`{key:'ui_font_scale', value}`).

### `set_font_family(family: "classic" | "modern" | "system") -> void`

Persists `ui_font_family`. Rejects any other value. Audit event
(`actor='user'`, `action='settings_updated'`, metadata
`{key:'ui_font_family', value}`).

### `set_language(language: "en" | "vi") -> void`

Persists `ui_language`. Rejects any other value with a clear error; an
out-of-contract stored value reads back as the default (`'en'`). Audit event
(`actor='user'`, `action='settings_updated'`, metadata
`{key:'ui_language', value}`).

### `reset_settings() -> void`

Resets app-managed preferences to their defaults: deletes the
`auto_approve_writes`, `ui_font_scale`, `ui_font_family`, and `ui_language` meta
keys. Prefs-only - does NOT touch Google tokens, the client id/secret,
permission rules, sources, pending changes, or the audit log. Audit event
(`actor='user'`, `action='settings_reset'`).

## MCP transport

The MCP sidecar transport and port live in the shared `meta` table so the
sidecar and the desktop app never drift. The sidecar reads them ONCE at startup,
so changing either only takes effect after the sidecar restarts - these commands
just persist config. See `docs/architecture.md` and `docs/security.md`.

### `get_mcp_config() -> McpConfigView`

```ts
type McpConfigView = {
  transport: "stdio" | "http"; // meta key 'mcp_transport', default 'stdio'
  port: number;                // meta key 'mcp_port', default 4319, range 1024-65535
  running: boolean;            // fresh heartbeat exists right now
  boundPort: number | null;    // configured port when running AND http, else null
};
```

`boundPort` is the configured port reported back only while an HTTP sidecar is
running; the desktop cannot observe the sidecar's actual socket across the DB, so
it equals the bound port unless the config changed without a restart. Null for
stdio or when not running.

### `set_mcp_transport(transport: "stdio" | "http") -> void`

Persists `mcp_transport`. Rejects any value other than `stdio` / `http`. Audit
event (`actor='user'`, `action='settings_updated'`, metadata
`{key:'mcp_transport', transport}`).

### `set_mcp_port(port: number) -> void`

Persists `mcp_port` after validating `1024 <= port <= 65535`; out-of-range values
are rejected with a clear error. Audit event (`actor='user'`,
`action='settings_updated'`, metadata `{key:'mcp_port', port}`).

## MCP server process control (HTTP transport)

For the HTTP transport the desktop app can manage the sidecar as a child
process. For the stdio transport there is nothing to start: the agent's MCP
client spawns the sidecar itself, so these commands are HTTP-only.

The child is spawned from the resolved `sheet-port-mcp` binary with the
environment overrides `SHEET_PORT_MCP_TRANSPORT=http` and
`SHEET_PORT_MCP_PORT={configured port}`, forcing it onto HTTP + the configured
port regardless of the stored `mcp_transport`. Exactly one managed child is
tracked at a time. On app exit the child is killed so no orphan sidecar lingers.
The heartbeat that `get_mcp_config` reports (`running`/`boundPort`) reflects any
fresh sidecar, including one started this way.

### `mcp_server_start() -> SidecarStatus`

```ts
type SidecarStatus = {
  running: boolean;
  pid: number | null;
};
```

Spawns the managed sidecar child on the HTTP transport bound to the configured
port. Starting when a managed child is already running is a clear error (only
one is allowed); an already-exited previous child is reaped first. Errors when
the resolved binary does not exist yet (build the release sidecar first). Audit
event (`actor='user'`, `action='mcp_server_started'`, metadata
`{pid, port, transport:'http'}`).

### `mcp_server_stop() -> SidecarStatus`

Kills the managed sidecar child if one is running. Idempotent: no managed child
is not an error. Audit event (`actor='user'`, `action='mcp_server_stopped'`,
metadata `{pid}`) is written only when a child was actually stopped.

## Google Sheets account (multi-account)

Multiple Google accounts can be connected at once. Each connected account is
its own source row with id `google-sheets:{accountKey}` (accountKey = the
sanitized email), kind `google_sheets`, name `Google Sheets ({email})`, plus
its own OS keychain entry `google_sheets:{accountKey}` holding that account's
tokens. The OAuth client id (`meta.google_client_id`) and client secret
(keychain `google_client_secret`) are SHARED across all accounts - there is a
single OAuth app.

Backward compatibility: on startup, a pre-multi-account single connection (bare
`google-sheets` source row + legacy `google_sheets` keychain entry) is migrated
into the keyed scheme (accountKey derived from the stored email, or `default`).
The migration is idempotent and best-effort.

### `get_google_config() -> GoogleConfig`

```ts
type GoogleConfig = {
  clientId: string | null;   // meta key 'google_client_id' (shared)
  hasClientSecret: boolean;  // keychain 'google_client_secret' present (shared)
};
```

Shared config only. The connected accounts are read separately from
`google_list_accounts` so the UI can render the full list.

### `google_list_accounts() -> GoogleAccount[]`

```ts
type GoogleAccount = {
  sourceId: string; // 'google-sheets:{accountKey}'
  email: string;
};
```

Every connected Google account, ordered by source id. Derived from the keyed
`google_sheets` source rows.

### `set_google_client_id(clientId: string) -> void`

Trims and stores the OAuth desktop client id in `meta` (`google_client_id`).
Empty -> `Err("Google client ID must not be empty")`. Audit event
(`actor='user'`, `action='settings_updated'`, metadata `{key}` only - the id
value is never audited).

### `set_google_client_secret(clientSecret: string) -> void`

Stores the shared OAuth client secret in the OS keychain (empty clears it).
Audit event (`actor='user'`, `action='settings_updated'`, metadata
`{key:'google_client_secret'}`).

### `google_connect() -> { email: string }`

Connects a NEW account (or updates an existing one when the same email is used
again). Runs the full interactive OAuth flow (system browser consent + loopback
redirect + PKCE token exchange) using the stored client id; missing id ->
`Err("Google client ID is not configured. Set it in the desktop app settings")`.
Blocks until the user finishes or the flow times out, so it is an async command
executed on a blocking task with its OWN SQLite connection (the shared one stays
free for status polling). On success the account key is derived from the
signed-in email, that account's tokens land in the OS keychain, its
`google-sheets:{accountKey}` sources row is upserted, and an audit event
`google_connected` (actor user, source = the account's source id, metadata
`{email}`) is written.

### `google_disconnect(sourceId: string) -> void`

Removes ONE account: its keychain credential and its
`google-sheets:{accountKey}` sources row (idempotent). Rejects a `sourceId`
that is not a keyed Google account. Audit event `google_disconnected` (actor
user, source = `sourceId`).

## Workbench

A user-curated tree of spreadsheets grouped into folders, distinct from the raw
`list_tables` path. Folders and items live in `workbench_folders` /
`workbench_items` (see `schema.sql`); deleting a folder falls its items back to
Ungrouped (`folder_id` NULL) via `ON DELETE SET NULL`. Every folder/item
mutation records an audit event (actor user). Grid reads and writes are DIRECT
(no pending-change/approval flow): the desktop user is the approver.

```ts
type WorkbenchFolder = { id: string; name: string; position: number };
type WorkbenchItem = {
  id: string;
  folderId: string | null;   // null -> Ungrouped
  sourceId: string;          // owning Google account source id
  spreadsheetId: string;
  name: string;              // resolved spreadsheet title
  position: number;
};
type SheetTab = { gid: string; title: string; index: number };
type GridData = {
  columns: { id: string; title: string }[]; // id AND title = A1 column letter
  rows: Record<string, string>[];            // each row keyed by column id
  totalRows: number;                         // all sheet rows ignoring limit/offset
};
```

### `workbench_tree() -> { folders: WorkbenchFolder[]; items: WorkbenchItem[] }`

Folders ordered by `position` then `name`; items by `position`.

### `create_workbench_folder(name: string) -> WorkbenchFolder`

`position = max + 1`. Name is trimmed and must not be empty. Audit
`workbench_folder_created`.

### `rename_workbench_folder(id: string, name: string) -> void`

Trimmed non-empty name. Unknown id -> `Err`. Audit `workbench_folder_renamed`.

### `delete_workbench_folder(id: string) -> void`

Its items fall back to Ungrouped. Unknown id -> `Err`. Audit
`workbench_folder_deleted`.

### `add_workbench_spreadsheet(folderId: string | null, urlOrId: string) -> WorkbenchItem`

Source = the first connected `google_sheets` source (clear error when none is
connected). `urlOrId` is parsed to a spreadsheet id (Google URL / bare id /
`id:selector`); the name is the spreadsheet's own title. If the same spreadsheet
already exists in that folder the existing item is returned. `position = max + 1`
within the folder. Audit `workbench_item_added`.

### `remove_workbench_item(id: string) -> void`

Removes the item (does not touch the source). Unknown id -> `Err`. Audit
`workbench_item_removed`.

### `move_workbench_item(id: string, folderId: string | null) -> void`

Moves the item to the end of the destination folder (or Ungrouped when null).
Unknown item id or non-null target folder -> `Err`. Audit `workbench_item_moved`.

### `list_workbench_sheet_tabs(itemId: string) -> SheetTab[]`

Resolves the item to its source + spreadsheet, then lists the tabs left to right.

### `read_workbench_sheet(itemId: string, gid: string, limit: number | null, offset: number | null) -> GridData`

Reads one tab (`tableId = {spreadsheetId}:{gid}`) as a RAW mirror of string
cells - exactly like Google Sheets. Columns are the A1 column letters (id AND
title = `A`, `B`, `C`, ...); the column count is the widest sheet row. Rows are
EVERY sheet row starting at row 1 (the first row is real data, never consumed as
a header); empty cells are empty strings. Default limit 100, clamp 1..=500,
offset floors at 0; `totalRows` counts all sheet rows (row 1 included).

### `update_workbench_cell(itemId: string, gid: string, rowIndex: number, columnId: string, value: string) -> void`

Writes one cell directly. `rowIndex` is 0-based over ALL sheet rows (sheet row =
rowIndex + 1, so row 1 = index 0); `columnId` is the A1 column letter. Audit
`workbench_cell_updated` (actor user).

### `append_workbench_row(itemId: string, gid: string, values: Record<string, string>) -> { rowIndex: number }`

Appends a row at the bottom, ordered by column letter (values keyed by column
id; absent columns write empty cells), and returns its new 0-based row index
(= the previous `totalRows`). Audit `workbench_row_appended` (actor user).

## Confirmation enforcement (cross-process)

1. Agent calls `preview_update_records` / `append_records` -> sidecar inserts a
   `pending_changes` row with `requires_confirmation` from the permission rule.
2. Agent calls `commit_change`:
   - status `rejected`/`committed` -> error.
   - `requires_confirmation = 1` and status is not `approved` -> error telling the
     agent to ask the user to approve in the desktop app.
   - `requires_confirmation = 0` and status `pending` -> allowed (`decided_by='policy'`).
   - Permission re-checked at commit time; connector write; status -> `committed`.
3. Desktop `approve_change` / `reject_change` flips the row; the sidecar reads
   fresh state from SQLite on every call, so no IPC between the processes is needed.

## Window / capabilities

`decorations: false`; the frontend renders a custom titlebar with
`data-tauri-drag-region` and uses `@tauri-apps/api/window` for minimize /
toggle-maximize / close. `src-tauri/capabilities/default.json` must grant:
`core:window:allow-minimize`, `core:window:allow-toggle-maximize`,
`core:window:allow-close`, `core:window:allow-start-dragging`, `core:default`.
