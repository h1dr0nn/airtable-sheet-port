//! MCP server heartbeat rows and the desktop status readout. The server
//! upserts its own row every HEARTBEAT_INTERVAL_MS and deletes rows older
//! than HEARTBEAT_STALE_MS on startup; the desktop treats the server as
//! running while any row is fresh. ISO timestamps compare lexicographically,
//! so freshness checks stay in plain SQL.

use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::HEARTBEAT_STALE_MS;
use crate::db::{iso_before, now_iso};
use crate::error::{db_error, CoreError};
use crate::types::{AppStatus, HeartbeatStatus};

pub fn upsert_own(conn: &Connection, pid: i64) -> Result<(), CoreError> {
    let now = now_iso();
    conn.execute(
        "INSERT INTO mcp_heartbeat (pid, started_at, last_seen) VALUES (?1, ?2, ?2)
         ON CONFLICT(pid) DO UPDATE SET last_seen = excluded.last_seen",
        params![pid, now],
    )
    .map_err(|error| db_error("Could not upsert heartbeat", error))?;
    Ok(())
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
/// when the server was last alive.
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

    let pending_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pending_changes WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| db_error("Could not count pending changes", error))?;

    let freshness_floor = iso_before(HEARTBEAT_STALE_MS);
    let (mcp_pid, mcp_last_seen, mcp_running) = match newest {
        Some((pid, last_seen)) => {
            let running = last_seen >= freshness_floor;
            (Some(pid), Some(last_seen), running)
        }
        None => (None, None, false),
    };

    Ok(AppStatus {
        app_version,
        db_path,
        mcp_running,
        mcp_pid,
        mcp_last_seen,
        pending_count,
    })
}

#[cfg(test)]
#[path = "heartbeat_tests.rs"]
mod tests;
