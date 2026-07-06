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

### `token_status() -> TokenStatus`

```ts
type TokenStatus = {
  googleSheets: boolean; // OS keychain entry exists (service "sheet-port", user "google_sheets")
  provider: boolean;     // ... user "provider"
};
```

Keyring stub only; no tokens are ever returned to the frontend or agents.

## Settings (app-managed preferences)

App-managed preferences live in the shared `meta` table so both processes see
them. Frontend-only prefs (e.g. theme, kept in `localStorage`) are NOT part of
this contract and are not reset by `reset_settings`.

### `get_settings() -> AppSettings`

```ts
type AppSettings = {
  autoApproveWrites: boolean; // meta key 'auto_approve_writes' === '1'
};
```

### `set_auto_approve(enabled: boolean) -> void`

Enabling writes meta `auto_approve_writes = '1'`; disabling deletes the key so
it reads back as the absent default. When on, the commit path treats a
`requires_confirmation` change as policy-approved and bypasses the human
confirmation gate (see `docs/security.md`). Audit event (`actor='user'`,
`action='settings_updated'`, metadata `{key:'auto_approve_writes', enabled}`).

### `reset_settings() -> void`

Resets app-managed preferences to their defaults: deletes the
`auto_approve_writes` meta key. Prefs-only - does NOT touch Google tokens, the
client id/secret, permission rules, sources, pending changes, or the audit log.
Audit event (`actor='user'`, `action='settings_reset'`).

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

## Google Sheets account

### `get_google_config() -> GoogleConfig`

```ts
type GoogleConfig = {
  clientId: string | null;       // meta key 'google_client_id'
  connectedEmail: string | null; // parsed from the 'google-sheets' sources row name,
                                 // null when the row is absent
};
```

### `set_google_client_id(clientId: string) -> void`

Trims and stores the OAuth desktop client id in `meta` (`google_client_id`).
Empty -> `Err("Google client ID must not be empty")`. Audit event
(`actor='user'`, `action='settings_updated'`, metadata `{key}` only - the id
value is never audited).

### `google_connect() -> { email: string }`

Runs the full interactive OAuth flow (system browser consent + loopback
redirect + PKCE token exchange) using the stored client id; missing id ->
`Err("Google client ID is not configured. Set it in the desktop app settings")`.
Blocks until the user finishes or the flow times out, so it is an async
command executed on a blocking task with its OWN SQLite connection (the
shared one stays free for status polling). On success the token lands in the
OS keychain, the `google-sheets` sources row is upserted, and an audit event
`google_connected` (actor user, metadata `{email}`) is written.

### `google_disconnect() -> void`

Deletes the keychain credential and the `google-sheets` sources row
(idempotent). Audit event `google_disconnected` (actor user).

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
