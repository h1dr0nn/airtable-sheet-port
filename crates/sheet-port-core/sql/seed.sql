-- Idempotent first-run seed. Guarded by meta key 'seeded'; every statement
-- also uses INSERT OR IGNORE so re-running is harmless. Executed by whichever
-- process (desktop app or MCP sidecar) opens the database first.
--
-- Since schema_version 2 fresh databases start EMPTY: no sources, no
-- permission rules, no mock data. Rows only appear when the user connects a
-- source (e.g. Google Sheets) in the desktop app. The v1 demo workspace is
-- removed by the v1 -> v2 migration in db.rs.

INSERT OR IGNORE INTO meta (key, value) VALUES ('seeded', '1');
INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '2');
