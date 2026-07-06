// Embedded copies of packages/storage/schema.sql and packages/storage/seed.sql.
// The .sql files are the human-readable source of truth (the Rust desktop
// backend embeds them via include_str!). tsc does not copy .sql assets into
// dist, so the Node side ships them as string constants instead of reading
// from disk. KEEP IN SYNC: any edit to schema.sql or seed.sql must be copied
// here verbatim, and vice versa.

export const SCHEMA_SQL = `
-- Airtable - Sheet Port shared SQLite schema.
-- Single source of truth for BOTH the Rust desktop backend (rusqlite, include_str!)
-- and the Node MCP sidecar (node:sqlite). Idempotent: every statement uses IF NOT EXISTS.
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
  change_type TEXT NOT NULL CHECK (change_type IN ('append', 'update', 'delete')),
  created_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'committed', 'rejected')),
  requires_confirmation INTEGER NOT NULL DEFAULT 0,
  -- Agent-visible diff, JSON. For append: {"after": records}. For update:
  -- [{"recordId","before","after"}] per patched record.
  diff TEXT NOT NULL,
  -- Internal payload, JSON, never returned to agents:
  -- append -> {"records": [...]}, update -> {"patches": [...]}.
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
`;

export const SEED_SQL = `
-- Idempotent first-run seed data. Guarded by meta key 'seeded'; every statement
-- also uses INSERT OR IGNORE so re-running is harmless. Executed by whichever
-- process (desktop app or MCP sidecar) opens the database first.

INSERT OR IGNORE INTO sources (id, kind, name, status) VALUES
  ('mock-source', 'mock', 'Demo Workspace', 'connected'),
  ('google-placeholder', 'google_sheets', 'Google Sheets (connect soon)', 'placeholder'),
  ('provider-placeholder', 'provider', 'Additional provider (connect soon)', 'placeholder');

INSERT OR IGNORE INTO permission_rules
  (source_id, table_id, can_read, can_write, can_delete, require_confirmation, updated_at)
VALUES
  ('mock-source', 'customers', 1, 1, 0,
   '["append","update","delete","bulk_update"]',
   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

INSERT OR IGNORE INTO mock_tables (source_id, table_id, name, fields) VALUES
  ('mock-source', 'customers', 'Customers',
   '[{"name":"Name","type":"string","required":true},{"name":"Email","type":"email"},{"name":"Plan","type":"enum","enumValues":["free","pro","enterprise"]},{"name":"Seats","type":"number"},{"name":"Active","type":"boolean"}]');

INSERT OR IGNORE INTO mock_records (source_id, table_id, record_id, fields, position) VALUES
  ('mock-source', 'customers', 'rec_seed_1',
   '{"Name":"Aurora Labs","Email":"ops@auroralabs.dev","Plan":"pro","Seats":24,"Active":true}', 1),
  ('mock-source', 'customers', 'rec_seed_2',
   '{"Name":"Basalt Co","Email":"it@basalt.co","Plan":"free","Seats":3,"Active":true}', 2),
  ('mock-source', 'customers', 'rec_seed_3',
   '{"Name":"Cirrus Retail","Email":"admin@cirrus.shop","Plan":"enterprise","Seats":180,"Active":false}', 3);

INSERT OR IGNORE INTO meta (key, value) VALUES ('seeded', '1');
INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '1');
`;
