//! Shared SQLite access (see docs/ipc.md). Whichever process opens the
//! database first applies schema + seed; the schema is idempotent and the
//! seed only runs while the meta 'seeded' marker is absent, so user-visible
//! deletions of seed rows survive restarts.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::error::{db_error, CoreError};

/// Absolute-file-path override used by tests and smoke scripts.
pub const DB_ENV_VAR: &str = "SHEET_PORT_DB";
const DB_DIR_NAME: &str = "sheet-port";
const DB_FILE_NAME: &str = "sheet-port.db";
const BUSY_TIMEOUT_MS: u64 = 5000;

// include_str! paths are relative to THIS source file. The .sql files are the
// single source of truth for the shared database contract.
const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");
const SEED_SQL: &str = include_str!("../sql/seed.sql");

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
/// schema + first-run seed. Tests call this directly with temp paths so they
/// never touch the `SHEET_PORT_DB` env var.
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
    Ok(())
}

fn is_seeded(conn: &Connection) -> Result<bool, CoreError> {
    let marker: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'seeded'", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| db_error("Could not read seed marker", error))?;
    Ok(marker.is_some())
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

    #[test]
    fn schema_and_seed_apply_idempotently_twice() {
        let path = temp_db_path();
        let first = open_at(&path).expect("first open should succeed");
        drop(first);
        let second = open_at(&path).expect("second open should reapply schema");

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
    fn does_not_reseed_deleted_rows_once_seeded() {
        // User-visible deletions must survive process restarts.
        let path = temp_db_path();
        let conn = open_at(&path).expect("first open");
        conn.execute(
            "DELETE FROM mock_records WHERE record_id = 'rec_seed_1'",
            [],
        )
        .expect("delete seed row");
        drop(conn);

        let reopened = open_at(&path).expect("second open");
        let records: i64 = reopened
            .query_row("SELECT COUNT(*) FROM mock_records", [], |row| row.get(0))
            .expect("count");
        assert_eq!(records, 2, "deleted seed row must stay deleted");
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
    fn iso_before_is_in_the_past_and_ordered() {
        let earlier = iso_before(30_000);
        let now = now_iso();
        assert!(earlier < now, "{earlier} must sort before {now}");
        assert_eq!(earlier.len(), 24);
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
