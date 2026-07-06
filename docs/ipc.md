# Desktop IPC Contract (Tauri Commands)

Contract between the Rust backend (`apps/desktop/src-tauri`) and the React frontend.
Both sides MUST match this document exactly. All commands return `Result<T, String>`;
the error string is user-displayable. All Rust structs serialize with
`#[serde(rename_all = "camelCase")]` so JSON field names match the TypeScript types
in `@sheet-port/shared`.

## Shared state model

Rust and the Node MCP sidecar share one SQLite database (WAL mode):

- Path: `%APPDATA%/sheet-port/sheet-port.db` (Windows), `~/Library/Application Support/sheet-port/sheet-port.db` (macOS), `$XDG_DATA_HOME/sheet-port/sheet-port.db` or `~/.local/share/sheet-port/sheet-port.db` (Linux).
- Env override: `SHEET_PORT_DB` (absolute file path) - used by tests and smoke scripts.
- Schema: `packages/storage/schema.sql`, seed: `packages/storage/seed.sql`. Rust embeds both via `include_str!`; the Node side loads the same files. Whichever process opens the DB first applies schema + seed (idempotent).
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

Mock source only for now: rows from `mock_tables`. Unknown sourceId -> `Ok([])`.

### `describe_table(sourceId: string, tableId: string) -> TableSchema`

From `mock_tables.fields` JSON. Unknown table -> `Err("Unknown table ...")`.

### `read_table(sourceId: string, tableId: string, limit: number | null, offset: number | null) -> TablePage`

```ts
type TablePage = {
  records: TableRecord[]; // ordered by position
  total: number;          // total record count ignoring limit/offset
};
```

Default limit 100, max 500. Reads `mock_records`.

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
