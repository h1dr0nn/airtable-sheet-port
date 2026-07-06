-- Idempotent first-run seed data. Guarded by meta key 'seeded'; every statement
-- also uses INSERT OR IGNORE so re-running is harmless. Executed by whichever
-- process (desktop app or MCP sidecar) opens the database first.
-- KEEP IN SYNC: packages/storage/src/sql.ts embeds a verbatim copy of this file
-- (SEED_SQL); any edit here must be mirrored there.

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
