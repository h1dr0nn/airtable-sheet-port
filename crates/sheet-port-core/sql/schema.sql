-- Airtable - Sheet Port shared SQLite schema.
-- Single source of truth for the desktop app and the MCP sidecar, both of
-- which embed it via include_str! in sheet-port-core. Idempotent: every
-- statement uses IF NOT EXISTS.
-- Connections must set: PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON.

CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sources (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('google_sheets', 'provider', 'mock')),
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'placeholder' CHECK (status IN ('connected', 'placeholder', 'error'))
);

CREATE TABLE IF NOT EXISTS permission_rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_id TEXT NOT NULL,
  -- NULL means the rule applies to the whole source; a table-specific rule wins over it.
  table_id TEXT,
  can_read INTEGER NOT NULL DEFAULT 0,
  can_write INTEGER NOT NULL DEFAULT 0,
  can_delete INTEGER NOT NULL DEFAULT 0,
  -- JSON array of ConfirmationAction strings, e.g. '["append","update","bulk_update"]'.
  require_confirmation TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL,
  UNIQUE (source_id, table_id)
);

CREATE TABLE IF NOT EXISTS pending_changes (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  change_type TEXT NOT NULL CHECK (change_type IN ('append', 'update', 'delete', 'format', 'create_spreadsheet', 'create_sheet', 'delete_sheet')),
  created_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'committed', 'rejected')),
  requires_confirmation INTEGER NOT NULL DEFAULT 0,
  -- Agent-visible diff, JSON. For append: {"after": records}. For update:
  -- [{"recordId","before","after"}] per patched record.
  diff TEXT NOT NULL,
  -- Internal payload, JSON, never returned to agents:
  -- append -> {"records": [...]}, update -> {"patches": [...]},
  -- format -> {"plan": {...}}.
  payload TEXT NOT NULL,
  decided_at TEXT,
  -- 'user' when approved/rejected in the desktop app, 'policy' when auto-allowed.
  decided_by TEXT,
  committed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pending_changes_status
  ON pending_changes (status, created_at DESC);

CREATE TABLE IF NOT EXISTS audit_events (
  id TEXT PRIMARY KEY,
  timestamp TEXT NOT NULL,
  actor TEXT NOT NULL CHECK (actor IN ('user', 'agent', 'system')),
  action TEXT NOT NULL,
  source_id TEXT,
  table_id TEXT,
  -- JSON object or NULL.
  metadata TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp
  ON audit_events (timestamp DESC);

-- The MCP sidecar upserts its own row every HEARTBEAT_INTERVAL_MS (10s) and
-- deletes rows older than HEARTBEAT_STALE_MS (30s) on startup. The desktop
-- treats the server as running when any row has last_seen within 30s.
CREATE TABLE IF NOT EXISTS mcp_heartbeat (
  pid INTEGER PRIMARY KEY,
  started_at TEXT NOT NULL,
  last_seen TEXT NOT NULL
);

-- Mock connector data lives in the DB so the desktop UI and the MCP sidecar
-- see the same records and committed changes persist across restarts.
CREATE TABLE IF NOT EXISTS mock_tables (
  source_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  name TEXT NOT NULL,
  -- JSON FieldSchema[].
  fields TEXT NOT NULL,
  PRIMARY KEY (source_id, table_id)
);

CREATE TABLE IF NOT EXISTS mock_records (
  source_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  record_id TEXT NOT NULL,
  -- JSON object of field name -> value.
  fields TEXT NOT NULL,
  -- Stable display ordering; new records take max(position) + 1.
  position INTEGER NOT NULL,
  PRIMARY KEY (source_id, table_id, record_id)
);

-- Workbench: a user-curated tree of spreadsheets grouped into folders, distinct
-- from the raw connector `list_tables` path. Folders and items each carry an
-- ascending `position` for stable display ordering (new rows take max + 1).
CREATE TABLE IF NOT EXISTS workbench_folders (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  position INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workbench_items (
  id TEXT PRIMARY KEY,
  -- NULL means the spreadsheet is Ungrouped. Deleting its folder falls the
  -- item back to Ungrouped rather than removing it (ON DELETE SET NULL).
  folder_id TEXT,
  source_id TEXT NOT NULL,
  spreadsheet_id TEXT NOT NULL,
  name TEXT NOT NULL,
  position INTEGER NOT NULL,
  FOREIGN KEY (folder_id) REFERENCES workbench_folders (id) ON DELETE SET NULL
);
