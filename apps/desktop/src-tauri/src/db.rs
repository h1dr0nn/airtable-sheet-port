//! SQLite access shared with the Node MCP sidecar (see docs/ipc.md).
//! Whichever process opens the database first applies schema + seed; both
//! files are idempotent so re-applying is harmless.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

/// Absolute-file-path override used by tests and smoke scripts.
pub const DB_ENV_VAR: &str = "SHEET_PORT_DB";
const DB_DIR_NAME: &str = "sheet-port";
const DB_FILE_NAME: &str = "sheet-port.db";
const BUSY_TIMEOUT_MS: u64 = 5000;

// include_str! paths are relative to THIS source file. The schema and seed
// live in packages/storage and are shared verbatim with the Node MCP sidecar.
const SCHEMA_SQL: &str = include_str!("../../../../packages/storage/schema.sql");
const SEED_SQL: &str = include_str!("../../../../packages/storage/seed.sql");

/// Connection plus the resolved path, managed as Tauri state.
pub struct DbState {
    pub conn: Mutex<Connection>,
    pub path: PathBuf,
}

impl DbState {
    pub fn init() -> Result<Self, String> {
        let path = resolve_db_path()?;
        let conn = open_at(&path)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path,
        })
    }
}

/// `SHEET_PORT_DB` override, else the per-user app-data directory documented
/// in docs/ipc.md (APPDATA / Application Support / XDG data home).
pub fn resolve_db_path() -> Result<PathBuf, String> {
    if let Ok(overridden) = std::env::var(DB_ENV_VAR) {
        if !overridden.trim().is_empty() {
            return Ok(PathBuf::from(overridden));
        }
    }
    Ok(platform_data_dir()?.join(DB_DIR_NAME).join(DB_FILE_NAME))
}

fn platform_data_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .map_err(|_| "APPDATA environment variable is not set".to_string())
    } else if cfg!(target_os = "macos") {
        home_dir().map(|home| home.join("Library").join("Application Support"))
    } else {
        match std::env::var("XDG_DATA_HOME") {
            Ok(dir) if !dir.trim().is_empty() => Ok(PathBuf::from(dir)),
            _ => home_dir().map(|home| home.join(".local").join("share")),
        }
    }
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME environment variable is not set".to_string())
}

/// Opens (creating parent dirs), sets the shared pragmas, and applies
/// schema + seed. Tests call this directly with temp paths so they never
/// touch the `SHEET_PORT_DB` env var.
pub fn open_at(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create database directory: {error}"))?;
    }
    let conn = Connection::open(path)
        .map_err(|error| format!("Could not open database at {}: {error}", path.display()))?;
    apply_pragmas(&conn)?;
    apply_schema_and_seed(&conn)?;
    Ok(conn)
}

fn apply_pragmas(conn: &Connection) -> Result<(), String> {
    // journal_mode returns the resulting mode, so query it instead of execute.
    conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))
        .map_err(|error| format!("Could not enable WAL mode: {error}"))?;
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))
        .map_err(|error| format!("Could not set busy timeout: {error}"))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|error| format!("Could not enable foreign keys: {error}"))?;
    Ok(())
}

fn apply_schema_and_seed(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|error| format!("Could not apply database schema: {error}"))?;
    conn.execute_batch(SEED_SQL)
        .map_err(|error| format!("Could not apply seed data: {error}"))?;
    Ok(())
}

/// ISO-8601 UTC with milliseconds, matching SQLite's
/// `strftime('%Y-%m-%dT%H:%M:%fZ', 'now')` used by the seed and the sidecar.
pub fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    /// Isolated temp-file DB per test; avoids env-var races between
    /// parallel tests by never touching `SHEET_PORT_DB`.
    pub fn temp_db_path() -> PathBuf {
        std::env::temp_dir()
            .join("sheet-port-tests")
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

    #[test]
    fn schema_and_seed_apply_idempotently_twice() {
        let path = temp_db_path();
        let first = open_at(&path).expect("first open should succeed");
        drop(first);
        let second = open_at(&path).expect("second open should reapply schema and seed");

        let sources: i64 = second
            .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))
            .expect("sources count");
        assert_eq!(sources, 3, "seed sources must not duplicate on re-open");

        let records: i64 = second
            .query_row("SELECT COUNT(*) FROM mock_records", [], |row| row.get(0))
            .expect("mock records count");
        assert_eq!(records, 3, "seed records must not duplicate on re-open");

        let seeded: String = second
            .query_row("SELECT value FROM meta WHERE key = 'seeded'", [], |row| {
                row.get(0)
            })
            .expect("seeded meta row");
        assert_eq!(seeded, "1");
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
    fn resolve_db_path_prefers_env_override() {
        // Serialize env access within this test only; other tests use open_at directly.
        let _ = std::env::var(DB_ENV_VAR);
        let custom = temp_db_path();
        std::env::set_var(DB_ENV_VAR, &custom);
        let resolved = resolve_db_path().expect("resolve with override");
        std::env::remove_var(DB_ENV_VAR);
        assert_eq!(resolved, custom);
    }
}
