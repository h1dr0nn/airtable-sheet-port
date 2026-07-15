//! Shared SQLite access (see docs/ipc.md). Whichever process opens the
//! database first applies schema + seed + pending migrations; the schema is
//! idempotent and the seed only runs while the meta 'seeded' marker is
//! absent. Since schema_version 2 fresh databases start empty (no demo
//! workspace); the v1 -> v2 migration removes the demo rows early builds
//! seeded on first run, and the v2 -> v3 / v3 -> v4 migrations widen the
//! pending-change CHECK constraint to allow the 'format' and then the
//! structural ('create_spreadsheet' / 'create_sheet' / 'delete_sheet') types.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::constants::{
    CLOSE_BEHAVIOR_DEFAULT, CLOSE_BEHAVIOR_VALUES, MCP_PORT_DEFAULT, MCP_PORT_MAX, MCP_PORT_MIN,
    MCP_TRANSPORT_HTTP, MCP_TRANSPORT_STDIO, META_CLOSE_BEHAVIOR, META_MCP_PORT,
    META_MCP_TRANSPORT, META_UI_FONT_FAMILY, META_UI_FONT_SCALE, META_UI_LANGUAGE,
    UI_FONT_FAMILY_DEFAULT, UI_FONT_FAMILY_VALUES, UI_FONT_SCALE_DEFAULT, UI_FONT_SCALE_VALUES,
    UI_LANGUAGE_DEFAULT, UI_LANGUAGE_VALUES,
};
use crate::error::{db_error, CoreError};

/// Absolute-file-path override used by tests and smoke scripts.
pub const DB_ENV_VAR: &str = "SHEET_PORT_DB";
const DB_DIR_NAME: &str = "sheet-port";
const DB_FILE_NAME: &str = "sheet-port.db";
const BUSY_TIMEOUT_MS: u64 = 5000;

const META_SEEDED_KEY: &str = "seeded";
const META_SCHEMA_VERSION_KEY: &str = "schema_version";
const SCHEMA_VERSION_CURRENT: &str = "4";

// include_str! paths are relative to THIS source file. The .sql files are the
// single source of truth for the shared database contract.
const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");
const SEED_SQL: &str = include_str!("../sql/seed.sql");

/// v1 -> v2: drop the demo workspace ('mock-source' + placeholder sources,
/// all mock data, and the demo permission rule). User-created rows survive.
/// Runs atomically together with the version bump.
const MIGRATE_V1_TO_V2_SQL: &str = "\
BEGIN IMMEDIATE;
DELETE FROM sources WHERE id IN ('mock-source', 'google-placeholder', 'provider-placeholder');
DELETE FROM mock_tables;
DELETE FROM mock_records;
DELETE FROM permission_rules WHERE source_id = 'mock-source';
INSERT INTO meta (key, value) VALUES ('schema_version', '2')
  ON CONFLICT(key) DO UPDATE SET value = excluded.value;
COMMIT;";

/// v2 -> v3: widen the pending_changes.change_type CHECK to allow 'format'.
/// SQLite cannot ALTER a CHECK constraint, so the table is rebuilt in place;
/// existing rows are copied verbatim. Runs atomically with the version bump.
const MIGRATE_V2_TO_V3_SQL: &str = "\
BEGIN IMMEDIATE;
DROP INDEX IF EXISTS idx_pending_changes_status;
ALTER TABLE pending_changes RENAME TO pending_changes_v2;
CREATE TABLE pending_changes (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  change_type TEXT NOT NULL CHECK (change_type IN ('append', 'update', 'delete', 'format')),
  created_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'committed', 'rejected')),
  requires_confirmation INTEGER NOT NULL DEFAULT 0,
  diff TEXT NOT NULL,
  payload TEXT NOT NULL,
  decided_at TEXT,
  decided_by TEXT,
  committed_at TEXT
);
INSERT INTO pending_changes
  (id, source_id, table_id, change_type, created_at, status,
   requires_confirmation, diff, payload, decided_at, decided_by, committed_at)
  SELECT id, source_id, table_id, change_type, created_at, status,
         requires_confirmation, diff, payload, decided_at, decided_by, committed_at
  FROM pending_changes_v2;
DROP TABLE pending_changes_v2;
CREATE INDEX idx_pending_changes_status ON pending_changes (status, created_at DESC);
INSERT INTO meta (key, value) VALUES ('schema_version', '3')
  ON CONFLICT(key) DO UPDATE SET value = excluded.value;
COMMIT;";

/// v3 -> v4: widen the pending_changes.change_type CHECK to allow the
/// coordinate-level 'update_cells' type and the structural change types
/// ('create_spreadsheet', 'create_sheet', 'delete_sheet'). Same
/// rebuild-in-place approach as v2 -> v3; existing rows are copied verbatim.
const MIGRATE_V3_TO_V4_SQL: &str = "\
BEGIN IMMEDIATE;
DROP INDEX IF EXISTS idx_pending_changes_status;
ALTER TABLE pending_changes RENAME TO pending_changes_v3;
CREATE TABLE pending_changes (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  change_type TEXT NOT NULL CHECK (change_type IN ('append', 'update', 'delete', 'format', 'update_cells', 'create_spreadsheet', 'create_sheet', 'delete_sheet')),
  created_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'committed', 'rejected')),
  requires_confirmation INTEGER NOT NULL DEFAULT 0,
  diff TEXT NOT NULL,
  payload TEXT NOT NULL,
  decided_at TEXT,
  decided_by TEXT,
  committed_at TEXT
);
INSERT INTO pending_changes
  (id, source_id, table_id, change_type, created_at, status,
   requires_confirmation, diff, payload, decided_at, decided_by, committed_at)
  SELECT id, source_id, table_id, change_type, created_at, status,
         requires_confirmation, diff, payload, decided_at, decided_by, committed_at
  FROM pending_changes_v3;
DROP TABLE pending_changes_v3;
CREATE INDEX idx_pending_changes_status ON pending_changes (status, created_at DESC);
INSERT INTO meta (key, value) VALUES ('schema_version', '4')
  ON CONFLICT(key) DO UPDATE SET value = excluded.value;
COMMIT;";

/// `SHEET_PORT_DB` override, else the per-user app-data directory documented
/// in docs/ipc.md (APPDATA / Application Support / XDG data home).
pub fn resolve_db_path() -> Result<PathBuf, CoreError> {
    if let Ok(overridden) = std::env::var(DB_ENV_VAR) {
        if !overridden.trim().is_empty() {
            return Ok(PathBuf::from(overridden));
        }
    }
    Ok(platform_data_dir()?.join(DB_DIR_NAME).join(DB_FILE_NAME))
}

fn platform_data_dir() -> Result<PathBuf, CoreError> {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .map_err(|_| CoreError::Storage("APPDATA environment variable is not set".to_string()))
    } else if cfg!(target_os = "macos") {
        home_dir().map(|home| home.join("Library").join("Application Support"))
    } else {
        match std::env::var("XDG_DATA_HOME") {
            Ok(dir) if !dir.trim().is_empty() => Ok(PathBuf::from(dir)),
            _ => home_dir().map(|home| home.join(".local").join("share")),
        }
    }
}

fn home_dir() -> Result<PathBuf, CoreError> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| CoreError::Storage("HOME environment variable is not set".to_string()))
}

/// Opens the database at the resolved default path.
pub fn open_default() -> Result<(Connection, PathBuf), CoreError> {
    let path = resolve_db_path()?;
    let conn = open_at(&path)?;
    Ok((conn, path))
}

/// Opens (creating parent dirs), sets the shared pragmas, and applies
/// schema + first-run seed + migrations. Tests call this directly with temp
/// paths so they never touch the `SHEET_PORT_DB` env var.
pub fn open_at(path: &Path) -> Result<Connection, CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            CoreError::Storage(format!("Could not create database directory: {error}"))
        })?;
    }
    let conn = Connection::open(path).map_err(|error| {
        CoreError::Storage(format!(
            "Could not open database at {}: {error}",
            path.display()
        ))
    })?;
    apply_pragmas(&conn)?;
    apply_schema_and_seed(&conn)?;
    Ok(conn)
}

fn apply_pragmas(conn: &Connection) -> Result<(), CoreError> {
    // journal_mode returns the resulting mode, so query it instead of execute.
    conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))
        .map_err(|error| db_error("Could not enable WAL mode", error))?;
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))
        .map_err(|error| db_error("Could not set busy timeout", error))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|error| db_error("Could not enable foreign keys", error))?;
    Ok(())
}

fn apply_schema_and_seed(conn: &Connection) -> Result<(), CoreError> {
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|error| db_error("Could not apply database schema", error))?;
    // The meta guard keeps deleted seed rows deleted: INSERT OR IGNORE alone
    // would resurrect them on every reopen once their PK is gone.
    if !is_seeded(conn)? {
        conn.execute_batch(SEED_SQL)
            .map_err(|error| db_error("Could not apply seed data", error))?;
    }
    migrate_to_current_version(conn)?;
    Ok(())
}

/// Brings older databases up to `SCHEMA_VERSION_CURRENT` by applying each
/// migration in sequence (v1 -> v2 -> v3 -> v4). A missing schema_version means a v1
/// database (the v1 seed always wrote it, but be defensive); fresh databases
/// get the current version from the seed and skip every step. Each step
/// advances the version marker inside its own transaction, so the loop
/// terminates; an unknown or newer marker is left untouched rather than
/// migrated blindly.
fn migrate_to_current_version(conn: &Connection) -> Result<(), CoreError> {
    loop {
        let version = get_meta(conn, META_SCHEMA_VERSION_KEY)?;
        if version.as_deref() == Some(SCHEMA_VERSION_CURRENT) {
            return Ok(());
        }
        let (sql, target) = match version.as_deref() {
            None | Some("1") => (MIGRATE_V1_TO_V2_SQL, "2"),
            Some("2") => (MIGRATE_V2_TO_V3_SQL, "3"),
            Some("3") => (MIGRATE_V3_TO_V4_SQL, "4"),
            Some(_) => return Ok(()),
        };
        conn.execute_batch(sql).map_err(|error| {
            db_error(
                &format!("Could not migrate database to schema version {target}"),
                error,
            )
        })?;
    }
}

fn is_seeded(conn: &Connection) -> Result<bool, CoreError> {
    Ok(get_meta(conn, META_SEEDED_KEY)?.is_some())
}

/// Reads a settings value from the shared `meta` table (e.g.
/// [`crate::constants::META_GOOGLE_CLIENT_ID`]).
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, CoreError> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
    .map_err(|error| db_error("Could not read meta value", error))
}

/// Upserts a settings value in the shared `meta` table.
pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .map_err(|error| db_error("Could not write meta value", error))?;
    Ok(())
}

/// Removes a settings value from the shared `meta` table. Idempotent: a
/// missing key is not an error (used to reset a setting back to its absent
/// default).
pub fn delete_meta(conn: &Connection, key: &str) -> Result<(), CoreError> {
    conn.execute("DELETE FROM meta WHERE key = ?1", [key])
        .map_err(|error| db_error("Could not delete meta value", error))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Appearance preferences (docs/ipc.md "Settings"). Stored in `meta` so the
// value survives restarts; the frontend applies the actual fonts. Both keys
// are validated against a fixed allow-list so an unknown value can never be
// persisted, and a missing key reads back as the documented default.
// ---------------------------------------------------------------------------

/// The stored UI font scale, or the default when the key is absent. An
/// unexpected stored value also falls back to the default so the UI never gets
/// an out-of-contract token.
pub fn get_ui_font_scale(conn: &Connection) -> Result<String, CoreError> {
    read_enum_meta(
        conn,
        META_UI_FONT_SCALE,
        &UI_FONT_SCALE_VALUES,
        UI_FONT_SCALE_DEFAULT,
    )
}

/// Persists the UI font scale after validating it against the allow-list.
pub fn set_ui_font_scale(conn: &Connection, value: &str) -> Result<(), CoreError> {
    let validated = validate_enum(value, &UI_FONT_SCALE_VALUES, "ui_font_scale")?;
    set_meta(conn, META_UI_FONT_SCALE, validated)
}

/// The stored UI font family, or the default when the key is absent.
pub fn get_ui_font_family(conn: &Connection) -> Result<String, CoreError> {
    read_enum_meta(
        conn,
        META_UI_FONT_FAMILY,
        &UI_FONT_FAMILY_VALUES,
        UI_FONT_FAMILY_DEFAULT,
    )
}

/// Persists the UI font family after validating it against the allow-list.
pub fn set_ui_font_family(conn: &Connection, value: &str) -> Result<(), CoreError> {
    let validated = validate_enum(value, &UI_FONT_FAMILY_VALUES, "ui_font_family")?;
    set_meta(conn, META_UI_FONT_FAMILY, validated)
}

/// The stored UI language ("en" | "vi"), or the default ("en") when the key is
/// absent or holds an out-of-contract value.
pub fn get_language(conn: &Connection) -> Result<String, CoreError> {
    read_enum_meta(
        conn,
        META_UI_LANGUAGE,
        &UI_LANGUAGE_VALUES,
        UI_LANGUAGE_DEFAULT,
    )
}

/// Persists the UI language after validating it against the allow-list.
pub fn set_language(conn: &Connection, value: &str) -> Result<(), CoreError> {
    let validated = validate_enum(value, &UI_LANGUAGE_VALUES, "ui_language")?;
    set_meta(conn, META_UI_LANGUAGE, validated)
}

/// The stored window close behavior ("ask" | "tray" | "quit"), or the default
/// ("ask") when the key is absent or holds an out-of-contract value.
pub fn get_close_behavior(conn: &Connection) -> Result<String, CoreError> {
    read_enum_meta(
        conn,
        META_CLOSE_BEHAVIOR,
        &CLOSE_BEHAVIOR_VALUES,
        CLOSE_BEHAVIOR_DEFAULT,
    )
}

/// Persists the window close behavior after validating it against the
/// allow-list.
pub fn set_close_behavior(conn: &Connection, value: &str) -> Result<(), CoreError> {
    let validated = validate_enum(value, &CLOSE_BEHAVIOR_VALUES, "close_behavior")?;
    set_meta(conn, META_CLOSE_BEHAVIOR, validated)
}

/// Reads a meta value that must be one of `allowed`, returning `default` when
/// the key is absent or holds an out-of-contract value.
fn read_enum_meta(
    conn: &Connection,
    key: &str,
    allowed: &[&str],
    default: &str,
) -> Result<String, CoreError> {
    let stored = get_meta(conn, key)?;
    Ok(match stored {
        Some(value) if allowed.contains(&value.as_str()) => value,
        _ => default.to_string(),
    })
}

/// Validates a settings value against a fixed allow-list, echoing back the
/// matching canonical `&'static str` (never the caller's owned string).
fn validate_enum<'a>(
    value: &str,
    allowed: &'a [&'a str],
    field: &str,
) -> Result<&'a str, CoreError> {
    allowed
        .iter()
        .copied()
        .find(|candidate| *candidate == value)
        .ok_or_else(|| {
            CoreError::InvalidInput(format!(
                "{field} must be one of {}, got \"{value}\"",
                allowed.join(", ")
            ))
        })
}

// ---------------------------------------------------------------------------
// MCP sidecar transport config (shared by the sidecar and the desktop app).
// See docs/architecture.md and docs/security.md. The values live in `meta` so
// the running sidecar and the desktop settings never drift; the sidecar reads
// them once at startup, so changes take effect on the next sidecar restart.
// ---------------------------------------------------------------------------

/// The MCP sidecar transport selected in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTransport {
    /// stdio JSON-RPC (default). No port; the agent's MCP client spawns it.
    Stdio,
    /// Loopback HTTP (streamable-http), bound strictly to 127.0.0.1.
    Http,
}

impl McpTransport {
    /// The canonical meta string ("stdio" | "http").
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => MCP_TRANSPORT_STDIO,
            Self::Http => MCP_TRANSPORT_HTTP,
        }
    }

    /// Parses a meta value. Only "http" selects HTTP; anything else (including
    /// an absent/unknown value) falls back to the safe stdio default.
    pub fn from_meta_value(value: Option<&str>) -> Self {
        match value {
            Some(MCP_TRANSPORT_HTTP) => Self::Http,
            _ => Self::Stdio,
        }
    }
}

/// Resolved MCP transport settings after defaults and clamping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpConfig {
    pub transport: McpTransport,
    /// The configured HTTP port, always within [MCP_PORT_MIN, MCP_PORT_MAX].
    /// Meaningful only when `transport == Http`.
    pub port: u16,
}

/// Clamps an out-of-range or unparseable port to the valid window, defaulting
/// to `MCP_PORT_DEFAULT` when the string is absent or not a number. Never
/// fails: the sidecar must always end up with a usable port.
pub fn clamp_mcp_port(raw: Option<&str>) -> u16 {
    match raw.and_then(|value| value.trim().parse::<u16>().ok()) {
        Some(port) => port.clamp(MCP_PORT_MIN, MCP_PORT_MAX),
        None => MCP_PORT_DEFAULT,
    }
}

/// Reads the effective MCP transport config from `meta`, applying the stdio
/// default and clamping the port. Used by the sidecar at startup and by the
/// desktop `get_mcp_config` command.
pub fn get_mcp_config(conn: &Connection) -> Result<McpConfig, CoreError> {
    let transport = McpTransport::from_meta_value(get_meta(conn, META_MCP_TRANSPORT)?.as_deref());
    let port = clamp_mcp_port(get_meta(conn, META_MCP_PORT)?.as_deref());
    Ok(McpConfig { transport, port })
}

/// Persists the selected transport. Writing "stdio" stores the explicit
/// default (rather than deleting the key) so the desktop can show the choice.
pub fn set_mcp_transport(conn: &Connection, transport: McpTransport) -> Result<(), CoreError> {
    set_meta(conn, META_MCP_TRANSPORT, transport.as_str())
}

/// Persists the HTTP port after validating the range. Rejects out-of-range
/// values with a clear message instead of silently clamping, so the desktop
/// surfaces the error to the user.
pub fn set_mcp_port(conn: &Connection, port: u16) -> Result<(), CoreError> {
    if !(MCP_PORT_MIN..=MCP_PORT_MAX).contains(&port) {
        return Err(CoreError::InvalidInput(format!(
            "port must be an integer between {MCP_PORT_MIN} and {MCP_PORT_MAX}"
        )));
    }
    set_meta(conn, META_MCP_PORT, &port.to_string())
}

/// ISO-8601 UTC with milliseconds, matching SQLite's
/// `strftime('%Y-%m-%dT%H:%M:%fZ', 'now')` used by the seed and JS
/// `new Date().toISOString()`.
pub fn now_iso() -> String {
    format_iso(chrono::Utc::now())
}

/// The instant `ms_ago` milliseconds in the past, in the same ISO shape.
/// ISO strings compare lexicographically, so callers use this for freshness
/// checks in plain SQL.
pub fn iso_before(ms_ago: i64) -> String {
    format_iso(chrono::Utc::now() - chrono::Duration::milliseconds(ms_ago))
}

/// The instant `ms_ahead` milliseconds in the future, in the same ISO shape.
/// Used for token-expiry bookkeeping (lexicographic comparison).
pub(crate) fn iso_after(ms_ahead: i64) -> String {
    format_iso(chrono::Utc::now() + chrono::Duration::milliseconds(ms_ahead))
}

fn format_iso(instant: chrono::DateTime<chrono::Utc>) -> String {
    instant.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    /// Isolated temp-file DB per test; avoids env-var races between
    /// parallel tests by never touching `SHEET_PORT_DB`.
    pub fn temp_db_path() -> PathBuf {
        std::env::temp_dir()
            .join("sheet-port-core-tests")
            .join(format!("{}.db", uuid::Uuid::new_v4()))
    }

    pub fn open_temp_db() -> Connection {
        open_at(&temp_db_path()).expect("temp db should open")
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::temp_db_path;
    use super::*;
    use crate::constants::{
        MCP_PORT_DEFAULT, MCP_PORT_MIN, META_CLOSE_BEHAVIOR, META_GOOGLE_CLIENT_ID,
        META_MCP_TRANSPORT, META_UI_LANGUAGE,
    };

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
    }

    /// The exact rows the v1 seed shipped, plus user-created rows that must
    /// survive the v1 -> v2 migration.
    const V1_DATABASE_FIXTURE: &str = r#"
INSERT INTO sources (id, kind, name, status) VALUES
  ('mock-source', 'mock', 'Demo Workspace', 'connected'),
  ('google-placeholder', 'google_sheets', 'Google Sheets (connect soon)', 'placeholder'),
  ('provider-placeholder', 'provider', 'Additional provider (connect soon)', 'placeholder'),
  ('google-sheets', 'google_sheets', 'Google Sheets (user@example.com)', 'connected');
INSERT INTO permission_rules
  (source_id, table_id, can_read, can_write, can_delete, require_confirmation, updated_at)
VALUES
  ('mock-source', 'customers', 1, 1, 0, '["append","update","delete","bulk_update"]',
   '2026-01-01T00:00:00.000Z'),
  ('google-sheets', NULL, 1, 0, 0, '[]', '2026-01-01T00:00:00.000Z');
INSERT INTO mock_tables (source_id, table_id, name, fields) VALUES
  ('mock-source', 'customers', 'Customers', '[]');
INSERT INTO mock_records (source_id, table_id, record_id, fields, position) VALUES
  ('mock-source', 'customers', 'rec_seed_1', '{}', 1);
"#;

    fn build_v1_database(path: &std::path::Path, meta_sql: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        let setup = Connection::open(path).expect("open raw");
        setup.execute_batch(SCHEMA_SQL).expect("apply schema");
        setup
            .execute_batch(V1_DATABASE_FIXTURE)
            .expect("apply v1 fixture");
        setup.execute_batch(meta_sql).expect("apply v1 meta");
    }

    #[test]
    fn fresh_databases_start_empty_at_schema_version_2() {
        let path = temp_db_path();
        let conn = open_at(&path).expect("open");

        assert_eq!(count(&conn, "sources"), 0, "no seeded sources");
        assert_eq!(count(&conn, "permission_rules"), 0, "no seeded rules");
        assert_eq!(count(&conn, "mock_tables"), 0, "no seeded mock tables");
        assert_eq!(count(&conn, "mock_records"), 0, "no seeded mock records");
        assert_eq!(get_meta(&conn, "seeded").expect("meta"), Some("1".into()));
        assert_eq!(
            get_meta(&conn, "schema_version").expect("meta"),
            Some(SCHEMA_VERSION_CURRENT.to_string())
        );
    }

    #[test]
    fn schema_and_seed_apply_idempotently_twice() {
        let path = temp_db_path();
        let first = open_at(&path).expect("first open should succeed");
        drop(first);
        let second = open_at(&path).expect("second open should reapply schema");

        assert_eq!(count(&second, "sources"), 0, "reopen must not add rows");
        let meta_rows: i64 = count(&second, "meta");
        assert_eq!(meta_rows, 2, "meta must hold exactly seeded + version");
    }

    #[test]
    fn user_rows_survive_reopen_without_reseeding() {
        // Rows created after the first open (e.g. by connecting a source or
        // by the test fixture) must not be wiped by later opens.
        let path = temp_db_path();
        let conn = open_at(&path).expect("first open");
        crate::test_fixtures::install_demo_workspace(&conn);
        drop(conn);

        let reopened = open_at(&path).expect("second open");
        assert_eq!(count(&reopened, "sources"), 1);
        assert_eq!(count(&reopened, "mock_records"), 3);
        assert_eq!(count(&reopened, "permission_rules"), 1);
    }

    #[test]
    fn migrates_v1_seeded_database_to_empty_v2() {
        let path = temp_db_path();
        build_v1_database(
            &path,
            "INSERT INTO meta (key, value) VALUES ('seeded', '1'), ('schema_version', '1');",
        );

        let conn = open_at(&path).expect("open migrates v1 database");

        let source_ids: Vec<String> = conn
            .prepare("SELECT id FROM sources ORDER BY id")
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        assert_eq!(
            source_ids,
            ["google-sheets"],
            "demo sources removed, user source kept"
        );
        assert_eq!(count(&conn, "mock_tables"), 0);
        assert_eq!(count(&conn, "mock_records"), 0);
        let rule_sources: Vec<String> = conn
            .prepare("SELECT source_id FROM permission_rules")
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        assert_eq!(
            rule_sources,
            ["google-sheets"],
            "demo rule removed, user rule kept"
        );
        assert_eq!(
            get_meta(&conn, "schema_version").expect("meta"),
            Some(SCHEMA_VERSION_CURRENT.to_string())
        );
    }

    /// A schema_version 2 `pending_changes` table (CHECK without 'format') plus
    /// one committed row, to prove the v2 -> v3 rebuild preserves data and then
    /// accepts the new change type.
    const V2_PENDING_CHANGES_FIXTURE: &str = r#"
DROP TABLE pending_changes;
CREATE TABLE pending_changes (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  change_type TEXT NOT NULL CHECK (change_type IN ('append', 'update', 'delete')),
  created_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'committed', 'rejected')),
  requires_confirmation INTEGER NOT NULL DEFAULT 0,
  diff TEXT NOT NULL,
  payload TEXT NOT NULL,
  decided_at TEXT,
  decided_by TEXT,
  committed_at TEXT
);
CREATE INDEX idx_pending_changes_status ON pending_changes (status, created_at DESC);
INSERT INTO pending_changes
  (id, source_id, table_id, change_type, created_at, status, requires_confirmation, diff, payload)
VALUES
  ('chg_keep', 'google-sheets', 'sheet-1', 'update', '2026-01-01T00:00:00.000Z',
   'committed', 1, '[]', '{"type":"update","patches":[]}');
"#;

    #[test]
    fn migrates_v2_pending_changes_to_v3_and_accepts_format() {
        let path = temp_db_path();
        // Build a v2-shaped database: current schema, then swap in the old
        // pending_changes table and mark the version as 2.
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        let setup = Connection::open(&path).expect("open raw");
        setup.execute_batch(SCHEMA_SQL).expect("apply schema");
        setup
            .execute_batch(V2_PENDING_CHANGES_FIXTURE)
            .expect("install v2 pending_changes");
        setup
            .execute_batch(
                "INSERT INTO meta (key, value) VALUES ('seeded', '1'), ('schema_version', '2');",
            )
            .expect("apply v2 meta");
        drop(setup);

        let conn = open_at(&path).expect("open migrates v2 database");

        assert_eq!(
            get_meta(&conn, "schema_version").expect("meta"),
            Some(SCHEMA_VERSION_CURRENT.to_string())
        );
        // The existing row survived the table rebuild.
        assert_eq!(count(&conn, "pending_changes"), 1);
        let kept: String = conn
            .query_row(
                "SELECT change_type FROM pending_changes WHERE id = 'chg_keep'",
                [],
                |row| row.get(0),
            )
            .expect("kept row");
        assert_eq!(kept, "update");
        // The widened CHECK now accepts a 'format' change.
        conn.execute(
            "INSERT INTO pending_changes
                 (id, source_id, table_id, change_type, created_at, status,
                  requires_confirmation, diff, payload)
             VALUES ('chg_fmt', 'google-sheets', 'sheet-1', 'format',
                     '2026-01-02T00:00:00.000Z', 'pending', 1, '{}', '{}')",
            [],
        )
        .expect("format change type is accepted after migration");
    }

    #[test]
    fn migrates_seeded_database_missing_schema_version() {
        let path = temp_db_path();
        build_v1_database(
            &path,
            "INSERT INTO meta (key, value) VALUES ('seeded', '1');",
        );

        let conn = open_at(&path).expect("open migrates unversioned database");
        assert_eq!(count(&conn, "mock_records"), 0);
        assert_eq!(
            get_meta(&conn, "schema_version").expect("meta"),
            Some(SCHEMA_VERSION_CURRENT.to_string())
        );
    }

    #[test]
    fn meta_helpers_read_and_upsert_values() {
        let conn = test_support::open_temp_db();
        assert_eq!(get_meta(&conn, META_GOOGLE_CLIENT_ID).expect("get"), None);

        set_meta(&conn, META_GOOGLE_CLIENT_ID, "client-1").expect("insert");
        assert_eq!(
            get_meta(&conn, META_GOOGLE_CLIENT_ID).expect("get"),
            Some("client-1".to_string())
        );

        set_meta(&conn, META_GOOGLE_CLIENT_ID, "client-2").expect("upsert");
        assert_eq!(
            get_meta(&conn, META_GOOGLE_CLIENT_ID).expect("get"),
            Some("client-2".to_string())
        );
    }

    #[test]
    fn ui_font_scale_defaults_and_round_trips_valid_values() {
        let conn = test_support::open_temp_db();
        assert_eq!(get_ui_font_scale(&conn).expect("default"), "normal");

        set_ui_font_scale(&conn, "large").expect("set large");
        assert_eq!(get_ui_font_scale(&conn).expect("get"), "large");
        set_ui_font_scale(&conn, "small").expect("set small");
        assert_eq!(get_ui_font_scale(&conn).expect("get"), "small");
    }

    #[test]
    fn ui_font_scale_rejects_unknown_values_and_ignores_stored_junk() {
        let conn = test_support::open_temp_db();
        assert!(set_ui_font_scale(&conn, "huge").is_err());
        // A value written outside the validated setter still reads back as the
        // default so the UI never gets an out-of-contract token.
        set_meta(&conn, META_UI_FONT_SCALE, "huge").expect("force junk");
        assert_eq!(get_ui_font_scale(&conn).expect("get"), "normal");
    }

    #[test]
    fn ui_font_family_defaults_and_round_trips_valid_values() {
        let conn = test_support::open_temp_db();
        assert_eq!(get_ui_font_family(&conn).expect("default"), "modern");

        set_ui_font_family(&conn, "classic").expect("set classic");
        assert_eq!(get_ui_font_family(&conn).expect("get"), "classic");
        set_ui_font_family(&conn, "system").expect("set system");
        assert_eq!(get_ui_font_family(&conn).expect("get"), "system");
        assert!(set_ui_font_family(&conn, "comic-sans").is_err());
    }

    #[test]
    fn close_behavior_defaults_and_round_trips_valid_values() {
        let conn = test_support::open_temp_db();
        assert_eq!(get_close_behavior(&conn).expect("default"), "ask");

        set_close_behavior(&conn, "tray").expect("set tray");
        assert_eq!(get_close_behavior(&conn).expect("get"), "tray");
        set_close_behavior(&conn, "quit").expect("set quit");
        assert_eq!(get_close_behavior(&conn).expect("get"), "quit");
    }

    #[test]
    fn close_behavior_rejects_unknown_values_and_ignores_stored_junk() {
        let conn = test_support::open_temp_db();
        assert!(set_close_behavior(&conn, "explode").is_err());
        // A value written outside the validated setter still reads back as the
        // default so the UI never gets an out-of-contract token.
        set_meta(&conn, META_CLOSE_BEHAVIOR, "explode").expect("force junk");
        assert_eq!(get_close_behavior(&conn).expect("get"), "ask");
    }

    #[test]
    fn language_defaults_and_round_trips_valid_values() {
        let conn = test_support::open_temp_db();
        assert_eq!(get_language(&conn).expect("default"), "en");

        set_language(&conn, "vi").expect("set vi");
        assert_eq!(get_language(&conn).expect("get"), "vi");
        set_language(&conn, "en").expect("set en");
        assert_eq!(get_language(&conn).expect("get"), "en");
    }

    #[test]
    fn language_rejects_unknown_values_and_ignores_stored_junk() {
        let conn = test_support::open_temp_db();
        assert!(set_language(&conn, "fr").is_err());
        // A value written outside the validated setter still reads back as the
        // default so the UI never gets an out-of-contract token.
        set_meta(&conn, META_UI_LANGUAGE, "fr").expect("force junk");
        assert_eq!(get_language(&conn).expect("get"), "en");
    }

    #[test]
    fn mcp_config_defaults_to_stdio_and_default_port() {
        let conn = test_support::open_temp_db();
        let config = get_mcp_config(&conn).expect("config");
        assert_eq!(config.transport, McpTransport::Stdio);
        assert_eq!(config.port, MCP_PORT_DEFAULT);
    }

    #[test]
    fn set_mcp_transport_round_trips_http_and_stdio() {
        let conn = test_support::open_temp_db();
        set_mcp_transport(&conn, McpTransport::Http).expect("set http");
        assert_eq!(
            get_mcp_config(&conn).expect("config").transport,
            McpTransport::Http
        );
        set_mcp_transport(&conn, McpTransport::Stdio).expect("set stdio");
        assert_eq!(
            get_mcp_config(&conn).expect("config").transport,
            McpTransport::Stdio
        );
    }

    #[test]
    fn set_mcp_port_persists_valid_and_rejects_out_of_range() {
        let conn = test_support::open_temp_db();
        set_mcp_port(&conn, 5000).expect("valid port");
        assert_eq!(get_mcp_config(&conn).expect("config").port, 5000);
        assert!(set_mcp_port(&conn, MCP_PORT_MIN - 1).is_err(), "below min");
        // MCP_PORT_MAX is u16::MAX so an above-max literal cannot be typed; the
        // clamp path covers stored junk instead (see clamp_mcp_port test).
        assert_eq!(get_mcp_config(&conn).expect("config").port, 5000);
    }

    #[test]
    fn clamp_mcp_port_handles_absent_and_invalid_values() {
        assert_eq!(clamp_mcp_port(None), MCP_PORT_DEFAULT);
        assert_eq!(clamp_mcp_port(Some("not-a-number")), MCP_PORT_DEFAULT);
        assert_eq!(
            clamp_mcp_port(Some("80")),
            MCP_PORT_MIN,
            "below min clamps up"
        );
        assert_eq!(clamp_mcp_port(Some("4319")), 4319);
    }

    #[test]
    fn unknown_transport_meta_value_falls_back_to_stdio() {
        let conn = test_support::open_temp_db();
        set_meta(&conn, META_MCP_TRANSPORT, "carrier-pigeon").expect("set");
        assert_eq!(
            get_mcp_config(&conn).expect("config").transport,
            McpTransport::Stdio
        );
    }

    #[test]
    fn now_iso_matches_sqlite_strftime_shape() {
        let value = now_iso();
        // e.g. 2026-07-06T12:34:56.789Z
        assert_eq!(value.len(), 24, "unexpected timestamp shape: {value}");
        assert!(value.ends_with('Z'));
        assert_eq!(&value[10..11], "T");
        assert_eq!(&value[19..20], ".");
    }

    #[test]
    fn iso_before_and_after_are_ordered_around_now() {
        let earlier = iso_before(30_000);
        let now = now_iso();
        let later = iso_after(30_000);
        assert!(earlier < now, "{earlier} must sort before {now}");
        assert!(now < later, "{now} must sort before {later}");
        assert_eq!(earlier.len(), 24);
        assert_eq!(later.len(), 24);
    }

    #[test]
    fn resolve_db_path_prefers_env_override() {
        // Serialize env access within this test only; other tests use open_at
        // directly with temp paths.
        let _ = std::env::var(DB_ENV_VAR);
        let custom = temp_db_path();
        std::env::set_var(DB_ENV_VAR, &custom);
        let resolved = resolve_db_path().expect("resolve with override");
        std::env::remove_var(DB_ENV_VAR);
        assert_eq!(resolved, custom);
    }
}
