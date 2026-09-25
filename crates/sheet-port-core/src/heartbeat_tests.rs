//! Ports the HeartbeatStore vitest suite plus the desktop app_status test.

use rusqlite::{params, Connection};

use super::*;
use crate::db::test_support::open_temp_db;

const TTL_MS: i64 = 30_000;
const STALE_AGE_MS: i64 = 60_000;

fn insert_stale_row(conn: &Connection, pid: i64) {
    let stale_iso = iso_before(STALE_AGE_MS);
    conn.execute(
        "INSERT INTO mcp_heartbeat (pid, started_at, last_seen) VALUES (?1, ?2, ?2)",
        params![pid, stale_iso],
    )
    .expect("insert stale heartbeat");
}

fn count_rows(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM mcp_heartbeat", [], |row| row.get(0))
        .expect("count")
}

#[test]
fn upserts_own_row_in_place_on_repeated_heartbeats() {
    let conn = open_temp_db();
    upsert_own(&conn, 111, "1.0.0").expect("first");
    upsert_own(&conn, 111, "1.0.0").expect("second");

    assert_eq!(count_rows(&conn), 1);
    let status = status(&conn, TTL_MS).expect("status");
    assert!(status.running);
    assert_eq!(status.pid, Some(111));
    assert!(status.last_seen.is_some());
}

#[test]
fn deletes_stale_rows_but_keeps_fresh_ones() {
    let conn = open_temp_db();
    insert_stale_row(&conn, 222);
    upsert_own(&conn, 111, "1.0.0").expect("upsert");

    delete_stale(&conn, TTL_MS).expect("delete stale");

    assert_eq!(count_rows(&conn), 1);
    assert_eq!(status(&conn, TTL_MS).expect("status").pid, Some(111));
}

#[test]
fn reports_not_running_when_only_stale_heartbeats_exist() {
    let conn = open_temp_db();
    insert_stale_row(&conn, 222);

    let status = status(&conn, TTL_MS).expect("status");
    assert!(!status.running);
    assert_eq!(status.pid, None);
    assert_eq!(status.last_seen, None);
}

#[test]
fn removes_own_row_on_shutdown() {
    let conn = open_temp_db();
    upsert_own(&conn, 111, "1.0.0").expect("upsert");

    delete_own(&conn, 111).expect("delete own");

    assert!(!status(&conn, TTL_MS).expect("status").running);
    assert_eq!(count_rows(&conn), 0);
}

#[test]
fn app_status_flags_fresh_and_stale_heartbeats() {
    let conn = open_temp_db();

    let empty = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(!empty.mcp_running, "no heartbeat row means not running");

    upsert_own(&conn, 4242, "1.0.0").expect("upsert");

    let fresh = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(fresh.mcp_running);
    assert_eq!(fresh.mcp_pid, Some(4242));

    conn.execute(
        "UPDATE mcp_heartbeat SET last_seen = '2020-01-01T00:00:00.000Z' WHERE pid = 4242",
        [],
    )
    .expect("stale heartbeat");
    let stale = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(
        !stale.mcp_running,
        "heartbeat older than 30s means not running"
    );
    assert_eq!(
        stale.mcp_pid,
        Some(4242),
        "stale pid still reported for the UI"
    );
}

#[test]
fn upsert_own_records_and_refreshes_the_sidecar_version() {
    let conn = open_temp_db();
    upsert_own(&conn, 111, "2.1.0").expect("first");
    upsert_own(&conn, 111, "2.2.1").expect("second");

    let version: Option<String> = conn
        .query_row(
            "SELECT version FROM mcp_heartbeat WHERE pid = 111",
            [],
            |row| row.get(0),
        )
        .expect("row");
    assert_eq!(version.as_deref(), Some("2.2.1"));
}

#[test]
fn app_status_lists_only_fresh_sidecars_with_versions() {
    let conn = open_temp_db();
    let empty = app_status(&conn, "2.2.1".into(), "test.db".into()).expect("status");
    assert!(empty.sidecars.is_empty());

    upsert_own(&conn, 100, "2.2.1").expect("current sidecar");
    // A fresh row from a sidecar that predates the version column.
    let now = crate::db::now_iso();
    conn.execute(
        "INSERT INTO mcp_heartbeat (pid, started_at, last_seen) VALUES (?1, ?2, ?2)",
        params![200, now],
    )
    .expect("legacy sidecar");
    insert_stale_row(&conn, 300);

    let status = app_status(&conn, "2.2.1".into(), "test.db".into()).expect("status");
    let mut sidecars: Vec<(i64, Option<String>)> = status
        .sidecars
        .iter()
        .map(|sidecar| (sidecar.pid, sidecar.version.clone()))
        .collect();
    sidecars.sort();
    assert_eq!(
        sidecars,
        vec![(100, Some("2.2.1".to_string())), (200, None)],
        "stale pid 300 is excluded; legacy row reports no version"
    );
    assert!(status
        .sidecars
        .iter()
        .all(|sidecar| !sidecar.last_seen.is_empty()));
    assert!(status.mcp_running);
}

fn identity(client: Option<&str>, exe: Option<&str>) -> HeartbeatIdentity {
    HeartbeatIdentity {
        client_name: client.map(str::to_string),
        client_version: client.map(|_| "1.2.3".to_string()),
        exe_path: exe.map(str::to_string),
        parent_exe_path: exe.map(|_| r"C:\Users\me\.local\bin\claude.exe".to_string()),
    }
}

#[test]
fn identity_is_written_and_survives_plain_heartbeat_ticks() {
    let conn = open_temp_db();
    let exe = r"C:\Program Files\Airtable - Sheet Port\sheet-port-mcp.exe";
    // Startup: exe known, client not yet initialized.
    upsert_own_with_identity(&conn, 500, "2.3.0", &identity(None, Some(exe))).expect("startup");
    let before = fresh_sidecar(&conn, 500, TTL_MS)
        .expect("read")
        .expect("fresh");
    assert_eq!(before.client_name, None);
    assert_eq!(before.exe_path.as_deref(), Some(exe));

    // initialize arrives: only the client fields are passed.
    upsert_own_with_identity(&conn, 500, "2.3.0", &identity(Some("claude-code"), None))
        .expect("initialize");
    // A plain tick afterwards must not erase anything.
    upsert_own(&conn, 500, "2.3.0").expect("tick");

    let status = app_status(&conn, "2.3.0".into(), "test.db".into()).expect("status");
    let row = &status.sidecars[0];
    assert_eq!(row.client_name.as_deref(), Some("claude-code"));
    assert_eq!(row.client_version.as_deref(), Some("1.2.3"));
    assert_eq!(row.exe_path.as_deref(), Some(exe));
    assert_eq!(
        row.parent_exe_path.as_deref(),
        Some(r"C:\Users\me\.local\bin\claude.exe")
    );
}

#[test]
fn app_status_reports_null_identity_for_older_sidecars() {
    let conn = open_temp_db();
    // What a v2.2.1 sidecar writes: version only.
    upsert_own(&conn, 600, "2.2.1").expect("old sidecar");
    let status = app_status(&conn, "2.3.0".into(), "test.db".into()).expect("status");
    let row = &status.sidecars[0];
    assert_eq!(row.pid, 600);
    assert_eq!(row.client_name, None);
    assert_eq!(row.client_version, None);
    assert_eq!(row.exe_path, None);
    assert_eq!(row.parent_exe_path, None);
    assert_eq!(status.bundled_sidecar_path, None, "filled in by the shell");
    assert!(!status.claude_desktop_running, "filled in by the shell");
    assert_eq!(status.managed_sidecar_pid, None, "filled in by the shell");
}

#[test]
fn fresh_sidecar_only_returns_fresh_rows_for_that_pid() {
    let conn = open_temp_db();
    upsert_own(&conn, 700, "2.3.0").expect("fresh");
    insert_stale_row(&conn, 701);

    assert_eq!(
        fresh_sidecar(&conn, 700, TTL_MS)
            .expect("read")
            .map(|row| row.pid),
        Some(700)
    );
    assert!(fresh_sidecar(&conn, 701, TTL_MS).expect("read").is_none());
    assert!(fresh_sidecar(&conn, 702, TTL_MS).expect("read").is_none());
}
