//! MCP server heartbeat rows and the desktop status readout. The server
//! upserts its own row every HEARTBEAT_INTERVAL_MS and deletes rows older
//! than HEARTBEAT_STALE_MS on startup; the desktop treats the server as
//! running while any row is fresh. ISO timestamps compare lexicographically,
//! so freshness checks stay in plain SQL.

use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::HEARTBEAT_STALE_MS;
use crate::db::{iso_before, now_iso};
use crate::error::{db_error, CoreError};
use crate::types::{AppStatus, HeartbeatStatus, SidecarHeartbeat};

/// Who is running a sidecar, written next to its heartbeat (schema_version
/// 6). Every field is optional: the client is only known once `initialize`
/// arrives, and the exe path can fail to resolve.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeartbeatIdentity {
    /// `clientInfo.name` from the MCP `initialize` request (e.g. "claude-code").
    pub client_name: Option<String>,
    /// `clientInfo.version` from the MCP `initialize` request.
    pub client_version: Option<String>,
    /// The sidecar's own executable (`std::env::current_exe`).
    pub exe_path: Option<String>,
    /// The executable of the process that spawned the sidecar (normally the
    /// MCP client itself, e.g. Claude Code's `claude.exe`).
    pub parent_exe_path: Option<String>,
}

/// Upserts this sidecar's row. `version` is the sidecar's package version
/// (`CARGO_PKG_VERSION`) so the desktop can spot sidecars left running from
/// an older install. Leaves any recorded client info untouched.
pub fn upsert_own(conn: &Connection, pid: i64, version: &str) -> Result<(), CoreError> {
    upsert_own_with_identity(conn, pid, version, &HeartbeatIdentity::default())
}

/// [`upsert_own`] plus the client/exe identity. A `None` field keeps the
/// stored value, so a heartbeat tick never erases what `initialize` wrote,
/// and a row recreated after deletion gets the full identity back.
pub fn upsert_own_with_identity(
    conn: &Connection,
    pid: i64,
    version: &str,
    identity: &HeartbeatIdentity,
) -> Result<(), CoreError> {
    let now = now_iso();
    conn.execute(
        "INSERT INTO mcp_heartbeat
           (pid, started_at, last_seen, version, client_name, client_version, exe_path,
            parent_exe_path)
         VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(pid) DO UPDATE SET
           last_seen = excluded.last_seen,
           version = excluded.version,
           client_name = COALESCE(excluded.client_name, client_name),
           client_version = COALESCE(excluded.client_version, client_version),
           exe_path = COALESCE(excluded.exe_path, exe_path),
           parent_exe_path = COALESCE(excluded.parent_exe_path, parent_exe_path)",
        params![
            pid,
            now,
            version,
            identity.client_name,
            identity.client_version,
            identity.exe_path,
            identity.parent_exe_path
        ],
    )
    .map_err(|error| db_error("Could not upsert heartbeat", error))?;
    Ok(())
}

/// The fresh heartbeat row for `pid` (seen within `ttl_ms`), if any. The
/// desktop's stop action only targets PIDs that pass this check.
pub fn fresh_sidecar(
    conn: &Connection,
    pid: i64,
    ttl_ms: i64,
) -> Result<Option<SidecarHeartbeat>, CoreError> {
    let read_error = |error| db_error("Could not read MCP heartbeat", error);
    conn.query_row(
        &format!("{SIDECAR_COLUMNS} WHERE pid = ?1 AND last_seen >= ?2"),
        params![pid, iso_before(ttl_ms)],
        sidecar_from_row,
    )
    .optional()
    .map_err(read_error)
}

pub fn delete_stale(conn: &Connection, ttl_ms: i64) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM mcp_heartbeat WHERE last_seen < ?1",
        [iso_before(ttl_ms)],
    )
    .map_err(|error| db_error("Could not delete stale heartbeats", error))?;
    Ok(())
}

pub fn delete_own(conn: &Connection, pid: i64) -> Result<(), CoreError> {
    conn.execute("DELETE FROM mcp_heartbeat WHERE pid = ?1", [pid])
        .map_err(|error| db_error("Could not delete heartbeat", error))?;
    Ok(())
}

/// The freshest live row within the TTL; not running when every row is stale.
pub fn status(conn: &Connection, ttl_ms: i64) -> Result<HeartbeatStatus, CoreError> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT pid, last_seen FROM mcp_heartbeat
             WHERE last_seen >= ?1 ORDER BY last_seen DESC LIMIT 1",
            [iso_before(ttl_ms)],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not read MCP heartbeat", error))?;
    Ok(match row {
        Some((pid, last_seen)) => HeartbeatStatus {
            running: true,
            pid: Some(pid),
            last_seen: Some(last_seen),
        },
        None => HeartbeatStatus {
            running: false,
            pid: None,
            last_seen: None,
        },
    })
}

/// Desktop status readout (docs/ipc.md get_app_status). Unlike [`status`],
/// the newest pid/last_seen are reported even when stale so the UI can show
/// when the server was last alive. `sidecars` lists only the fresh rows.
pub fn app_status(
    conn: &Connection,
    app_version: String,
    db_path: String,
) -> Result<AppStatus, CoreError> {
    let newest: Option<(i64, String)> = conn
        .query_row(
            "SELECT pid, last_seen FROM mcp_heartbeat ORDER BY last_seen DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not read MCP heartbeat", error))?;

    let freshness_floor = iso_before(HEARTBEAT_STALE_MS);
    let (mcp_pid, mcp_last_seen, mcp_running) = match newest {
        Some((pid, last_seen)) => {
            let running = last_seen >= freshness_floor;
            (Some(pid), Some(last_seen), running)
        }
        None => (None, None, false),
    };

    let sidecars = fresh_sidecars(conn, &freshness_floor)?;

    Ok(AppStatus {
        app_version,
        db_path,
        mcp_running,
        mcp_pid,
        mcp_last_seen,
        sidecars,
        // Filled in by the desktop shell, which knows its install layout and
        // can enumerate processes; the core only reads the database.
        bundled_sidecar_path: None,
        claude_desktop_running: false,
        managed_sidecar_pid: None,
    })
}

const SIDECAR_COLUMNS: &str = "SELECT pid, version, last_seen, client_name, client_version,
       exe_path, parent_exe_path
     FROM mcp_heartbeat";

fn sidecar_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SidecarHeartbeat> {
    Ok(SidecarHeartbeat {
        pid: row.get(0)?,
        version: row.get(1)?,
        last_seen: row.get(2)?,
        client_name: row.get(3)?,
        client_version: row.get(4)?,
        exe_path: row.get(5)?,
        parent_exe_path: row.get(6)?,
    })
}

/// Heartbeat rows at or after `freshness_floor`, newest first.
fn fresh_sidecars(
    conn: &Connection,
    freshness_floor: &str,
) -> Result<Vec<SidecarHeartbeat>, CoreError> {
    let read_error = |error| db_error("Could not read MCP heartbeats", error);
    let mut statement = conn
        .prepare(&format!(
            "{SIDECAR_COLUMNS} WHERE last_seen >= ?1 ORDER BY last_seen DESC, pid"
        ))
        .map_err(read_error)?;
    let rows = statement
        .query_map([freshness_floor], sidecar_from_row)
        .map_err(read_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

#[cfg(test)]
#[path = "heartbeat_tests.rs"]
mod tests;
