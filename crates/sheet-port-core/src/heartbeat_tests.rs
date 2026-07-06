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
    upsert_own(&conn, 111).expect("first");
    upsert_own(&conn, 111).expect("second");

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
    upsert_own(&conn, 111).expect("upsert");

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
    upsert_own(&conn, 111).expect("upsert");

    delete_own(&conn, 111).expect("delete own");

    assert!(!status(&conn, TTL_MS).expect("status").running);
    assert_eq!(count_rows(&conn), 0);
}

#[test]
fn app_status_flags_fresh_and_stale_heartbeats() {
    let conn = open_temp_db();

    let empty = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(!empty.mcp_running, "no heartbeat row means not running");
    assert_eq!(empty.pending_count, 0);

    upsert_own(&conn, 4242).expect("upsert");
    conn.execute(
        "INSERT INTO pending_changes
             (id, source_id, table_id, change_type, created_at, status,
              requires_confirmation, diff, payload)
         VALUES ('chg_status', 'mock-source', 'customers', 'update', ?1, 'pending', 1,
                 '[]', '{\"type\":\"update\",\"patches\":[]}')",
        [now_iso()],
    )
    .expect("insert pending change");

    let fresh = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(fresh.mcp_running);
    assert_eq!(fresh.mcp_pid, Some(4242));
    assert_eq!(fresh.pending_count, 1);

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
