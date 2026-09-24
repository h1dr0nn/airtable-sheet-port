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
  pendingCount: number;     // pending_changes WHERE status = 'pending' (staged dry runs)
};
```

### `list_sources() -> DataSource[]`

Rows from `sources`, mapped to `DataSource` (id, kind, name, status).

### `list_tables(sourceId: string) -> TableRef[]`

Routed through the `ConnectorRegistry` by the source's kind (google_sheets ->
Drive spreadsheet listing; mock -> `mock_tables`, only in builds with the cargo
feature `mock`). Unknown sourceId -> `Ok([])`.

### `describe_table(sourceId: string, tableId: string) -> TableSchema`

Routed through the `ConnectorRegistry` (google_sheets -> header row + inferred
types; mock -> `mock_tables.fields` JSON under the `mock` feature). Unknown table ->
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
  updatedAt: string;
};
```

Rules carry only the three toggles; the per-action confirmation list was removed
in 2.0.0. A rule created for a new bridge source allows read, write and delete.

### `save_permission_rule(rule: SavePermissionRule) -> PermissionRuleRow`

```ts
type SavePermissionRule = {
  id: number | null;      // null -> insert, else update by id
  sourceId: string;
  tableId: string | null;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
};
```

Upsert honoring `UNIQUE(source_id, table_id)`. Writes an audit event
(`actor='user'`, `action='permission_rule_saved'`, metadata = rule snapshot).

### `delete_permission_rule(id: number) -> void`

Writes audit event `permission_rule_deleted`.

### `list_changes(status: string | null) -> PendingChange[]`

`PendingChange` from `@sheet-port/shared` (diff = parsed JSON; `payload` is NEVER
returned). `status = null` -> all, newest first, limit 200.

The Changes screen is a history view of these rows. Most agent writes are staged
and committed in one call, so they appear already `committed`; only changes an
agent staged with `dryRun: true` stay `pending`.

### `reject_change(changeId: string) -> PendingChange`

Discards a staged change: transition `pending -> rejected` only (else `Err`).
Sets `decided_at` (now, ISO), `decided_by='user'`. Audit event `change_rejected`
(actor user). `commit_change` refuses a rejected change afterwards.

There is no approve command: the desktop cannot commit and does not gate
commits. Committing stays agent-side (`commit_change` MCP tool).

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
};
```

`googleSheets` reflects whether any Google account is connected. The OS keychain
cannot be enumerated, so account presence is derived from the `sources` table,
which `google_add_bridge` / `google_remove_bridge` keep in lockstep with the
keychain entries. The `provider` field was removed in 2.0.0.
No tokens are ever returned to the frontend or agents.

## Settings (app-managed preferences)

App-managed preferences live in the shared `meta` table so both processes see
them. Frontend-only prefs (e.g. theme, kept in `localStorage`) are NOT part of
this contract and are not reset by `reset_settings`.

### `get_settings() -> AppSettings`

```ts
type AppSettings = {
  fontScale: "small" | "normal" | "large";    // meta key 'ui_font_scale', default 'normal'
  fontFamily: "classic" | "modern" | "system"; // meta key 'ui_font_family', default 'modern'
  language: "en" | "vi";                       // meta key 'ui_language', default 'en'
};
```

`fontScale` / `fontFamily` / `language` are appearance preferences the frontend
applies to the UI. Absent (or out-of-contract) meta values read back as their
defaults.

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
`ui_font_scale`, `ui_font_family`, and `ui_language` meta keys. Prefs-only -
does NOT touch Google bridges or tokens, permission rules, sources, changes, or
the audit log. Audit event
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

## Google Sheets accounts (bridge pool)

Google access goes through Apps Script bridges (`bridge/`, setup in
`bridge/README.md`); there is no OAuth flow, client id, or client secret. The
app keeps a pool of bridges, one per Google account:

- Each bridge is `{bridgeUrl, secret, deploymentId}` (deploymentId parsed from
  the `/exec` URL) plus a cached access token, stored in the OS keychain under
  service `sheet-port`, user `google_sheets:{accountKey}`. accountKey is the
  sanitized email the bridge reports.
- Each account is one source row: id `google-sheets:{accountKey}`, kind
  `google_sheets`, name `Google Sheets ({email})`. Adding a bridge for an email
  that is already connected replaces it.
- The token is re-fetched from the bridge when it is within 60s of expiry. The
  MCP sidecar reads the keychain directly, so agents work while the desktop app
  is closed.
- The secret and tokens never cross IPC.

Removed in 2.0.0: `get_google_config`, `set_google_client_id`,
`set_google_client_secret`, `google_connect`, `google_disconnect`.

```ts
type GoogleAccount = {
  sourceId: string;     // 'google-sheets:{accountKey}'
  email: string;        // as reported by the bridge
  deploymentId: string; // parsed from the bridge URL
  bridgeUrl: string;    // the /exec URL (not secret on its own)
};
```

### `google_list_accounts() -> GoogleAccount[]`

Every connected account, ordered by source id.

### `google_add_bridge(url: string, secret: string) -> GoogleAccount`

Parses the deploymentId from the `/exec` URL, then calls the bridge once with
the secret. On `ok: true` it derives accountKey from the reported email, stores
the bridge and the fresh token in the keychain, upserts the
`google-sheets:{accountKey}` source row, and gives the new source read, write
and delete permission. An existing account with the same email is replaced. A
wrong secret or an unreachable bridge -> `Err` with a user-displayable message.
The secret is never written to the audit log.

### `google_remove_bridge(sourceId: string) -> void`

Removes ONE account: its keychain entry and its source row. Rejects a
`sourceId` that is not a keyed Google account.

### `google_test_bridge(sourceId: string) -> GoogleAccount`

Fetches a token from the account's bridge and returns the account as the bridge
reports it. `Err` when the bridge rejects the secret or cannot be reached.

## Workbench

A user-curated tree of spreadsheets grouped into folders, distinct from the raw
`list_tables` path. Folders and items live in `workbench_folders` /
`workbench_items` (see `schema.sql`); deleting a folder falls its items back to
Ungrouped (`folder_id` NULL) via `ON DELETE SET NULL`. Every folder/item
mutation records an audit event (actor user). Grid reads and writes are DIRECT
(not staged as pending changes): the desktop user edits the sheet in place.

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

## Change pipeline (cross-process)

1. An agent write tool (`update_records`, `append_records`, `update_cells`,
   `format_table`, `create_spreadsheet`, `create_sheet`, `delete_sheet`) makes the
   sidecar check the permission rule and insert a `pending_changes` row with the
   diff.
2. Without `dryRun`, the same call commits it: permission re-checked, connector
   write, status -> `committed`. With `dryRun: true` the row stays `pending` until
   `commit_change`.
3. `commit_change` refuses `rejected` and `committed` rows; there is no approval
   state to wait for.
4. Desktop `reject_change` discards a `pending` row; the sidecar reads fresh state
   from SQLite on every call, so no IPC between the processes is needed.

## Window / capabilities

`decorations: false`; the frontend renders a custom titlebar with
`data-tauri-drag-region` and uses `@tauri-apps/api/window` for minimize /
toggle-maximize / close. `src-tauri/capabilities/default.json` must grant:
`core:window:allow-minimize`, `core:window:allow-toggle-maximize`,
`core:window:allow-close`, `core:window:allow-start-dragging`, `core:default`.
