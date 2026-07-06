//! Ports the AuditStore vitest suite plus the AuditService id/timestamp
//! generation checks.

use rusqlite::params;
use serde_json::json;

use super::*;
use crate::db::test_support::open_temp_db;

fn insert_raw(conn: &rusqlite::Connection, id: &str, timestamp: &str) {
    conn.execute(
        "INSERT INTO audit_events (id, timestamp, actor, action) VALUES (?1, ?2, 'agent', 'read_table')",
        params![id, timestamp],
    )
    .expect("insert audit row");
}

#[test]
fn record_generates_evt_id_and_iso_timestamp() {
    let conn = open_temp_db();
    let event = record(
        &conn,
        AuditActor::Agent,
        "read_table",
        Some("mock-source"),
        Some("customers"),
        Some(&json!({ "count": 2 })),
    )
    .expect("record");

    assert!(event.id.starts_with("evt_"));
    assert_eq!(event.timestamp.len(), 24, "ISO ms UTC shape");
    assert!(event.timestamp.ends_with('Z'));

    let listed = list(&conn, None, None).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, event.id);
    assert_eq!(listed[0].actor, AuditActor::Agent);
    assert_eq!(listed[0].metadata, Some(json!({ "count": 2 })));
}

#[test]
fn lists_events_newest_first_with_limit_and_offset() {
    let conn = open_temp_db();
    insert_raw(&conn, "evt_1", "2026-01-01T00:00:00.000Z");
    insert_raw(&conn, "evt_2", "2026-01-02T00:00:00.000Z");
    insert_raw(&conn, "evt_3", "2026-01-03T00:00:00.000Z");

    let ids =
        |events: Vec<AuditEvent>| events.into_iter().map(|event| event.id).collect::<Vec<_>>();
    assert_eq!(
        ids(list(&conn, Some(10), None).expect("list")),
        ["evt_3", "evt_2", "evt_1"]
    );
    assert_eq!(ids(list(&conn, Some(1), None).expect("list")), ["evt_3"]);
    assert_eq!(
        ids(list(&conn, Some(2), Some(1)).expect("list")),
        ["evt_2", "evt_1"]
    );
}

#[test]
fn same_timestamp_ties_break_by_insertion_order_newest_first() {
    let conn = open_temp_db();
    let timestamp = "2026-01-01T00:00:00.000Z";
    insert_raw(&conn, "evt_first", timestamp);
    insert_raw(&conn, "evt_second", timestamp);

    let events = list(&conn, Some(10), None).expect("list");
    let ids: Vec<&str> = events.iter().map(|event| event.id.as_str()).collect();
    assert_eq!(ids, ["evt_second", "evt_first"]);
}

#[test]
fn round_trips_optional_fields_and_omits_absent_ones() {
    let conn = open_temp_db();
    record(
        &conn,
        AuditActor::Agent,
        "read_table",
        Some("mock-source"),
        Some("customers"),
        Some(&json!({ "count": 2 })),
    )
    .expect("full event");
    record(&conn, AuditActor::System, "startup", None, None, None).expect("bare event");

    let events = list(&conn, None, None).expect("list");
    assert_eq!(events.len(), 2);
    let bare = &events[0];
    let full = &events[1];

    assert_eq!(full.source_id.as_deref(), Some("mock-source"));
    assert_eq!(full.table_id.as_deref(), Some("customers"));
    assert_eq!(bare.metadata, None);

    let serialized = serde_json::to_string(bare).expect("serialize");
    assert!(
        !serialized.contains("metadata"),
        "absent metadata must not serialize"
    );
    assert!(
        !serialized.contains("sourceId"),
        "absent sourceId must not serialize"
    );
}

#[test]
fn clear_removes_all_events_and_reports_count() {
    let conn = open_temp_db();
    insert_raw(&conn, "evt_1", "2026-01-01T00:00:00.000Z");
    insert_raw(&conn, "evt_2", "2026-01-02T00:00:00.000Z");

    let removed = clear(&conn).expect("clear");
    assert_eq!(removed, 2, "clear reports the number of deleted rows");
    assert!(
        list(&conn, None, None).expect("list").is_empty(),
        "no audit events remain after clear"
    );

    // Clearing an already-empty log is a no-op that removes nothing.
    assert_eq!(clear(&conn).expect("clear again"), 0);
}

#[test]
fn limit_clamps_low_values_and_offset_floors_at_zero() {
    let conn = open_temp_db();
    insert_raw(&conn, "evt_1", "2026-01-01T00:00:00.000Z");
    insert_raw(&conn, "evt_2", "2026-01-02T00:00:00.000Z");

    let clamped = list(&conn, Some(0), Some(-5)).expect("list");
    assert_eq!(clamped.len(), 1, "limit clamps up to 1, offset floors at 0");
    assert_eq!(clamped[0].id, "evt_2");

    let defaulted = list(&conn, None, None).expect("list");
    assert_eq!(defaulted.len(), 2, "default limit covers all rows");
}
