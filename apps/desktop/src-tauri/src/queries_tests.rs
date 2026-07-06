//! Unit tests for the query layer. Every test opens an isolated temp-file
//! database via `db::open_at`, so tests never touch the `SHEET_PORT_DB` env
//! var and can run in parallel.

use rusqlite::{params, Connection};

use super::*;
use crate::db::test_support::open_temp_db;
use crate::models::SavePermissionRule;

const TEST_SOURCE: &str = "mock-source";
const TEST_TABLE: &str = "customers";

fn insert_pending_change(conn: &Connection, id: &str, status: &str) {
    conn.execute(
        "INSERT INTO pending_changes
             (id, source_id, table_id, change_type, created_at, status,
              requires_confirmation, diff, payload)
         VALUES (?1, ?2, ?3, 'update', ?4, ?5, 1,
                 '[{\"recordId\":\"rec_seed_1\",\"before\":{},\"after\":{}}]',
                 '{\"patches\":[]}')",
        params![id, TEST_SOURCE, TEST_TABLE, crate::db::now_iso(), status],
    )
    .expect("insert pending change");
}

fn sample_rule(table_id: Option<&str>, write: bool) -> SavePermissionRule {
    SavePermissionRule {
        id: None,
        source_id: "rule-source".to_string(),
        table_id: table_id.map(str::to_string),
        read: true,
        write,
        delete_records: false,
        require_confirmation_for: vec!["update".to_string(), "bulk_update".to_string()],
    }
}

#[test]
fn approve_transitions_pending_to_approved() {
    let conn = open_temp_db();
    insert_pending_change(&conn, "chg_1", "pending");

    let change = decide_change(&conn, "chg_1", ChangeDecision::Approve).expect("approve");

    assert_eq!(change.status, "approved");
    assert_eq!(change.decided_by.as_deref(), Some("user"));
    assert!(change.decided_at.is_some());
    assert!(change.requires_confirmation);

    let audits = list_audit_events(&conn, None, None).expect("audit list");
    assert!(audits.iter().any(|event| {
        event.action == "change_approved"
            && event.actor == "user"
            && event.source_id.as_deref() == Some(TEST_SOURCE)
    }));
}

#[test]
fn approve_refuses_already_approved_change() {
    let conn = open_temp_db();
    insert_pending_change(&conn, "chg_2", "pending");
    decide_change(&conn, "chg_2", ChangeDecision::Approve).expect("first approve");

    let error = decide_change(&conn, "chg_2", ChangeDecision::Approve)
        .expect_err("second approve must fail");
    assert!(
        error.contains("'approved'"),
        "error should name the actual status: {error}"
    );
}

#[test]
fn reject_refuses_non_pending_change() {
    let conn = open_temp_db();
    insert_pending_change(&conn, "chg_3", "committed");

    let error =
        decide_change(&conn, "chg_3", ChangeDecision::Reject).expect_err("reject must fail");
    assert!(
        error.contains("'committed'"),
        "error should name the actual status: {error}"
    );
}

#[test]
fn decide_change_reports_missing_change() {
    let conn = open_temp_db();
    let error =
        decide_change(&conn, "chg_missing", ChangeDecision::Approve).expect_err("must fail");
    assert!(error.contains("not found"), "unexpected error: {error}");
}

#[test]
fn list_changes_parses_diff_and_never_exposes_payload() {
    let conn = open_temp_db();
    insert_pending_change(&conn, "chg_4", "pending");

    let changes = list_changes(&conn, Some("pending")).expect("list");
    assert_eq!(changes.len(), 1);
    assert!(changes[0].diff.is_array(), "diff must be parsed JSON");

    let serialized = serde_json::to_string(&changes[0]).expect("serialize");
    assert!(
        !serialized.contains("payload"),
        "payload must never serialize"
    );
    assert!(
        !serialized.contains("patches"),
        "payload content must never leak"
    );
}

#[test]
fn list_changes_rejects_unknown_status() {
    let conn = open_temp_db();
    let error = list_changes(&conn, Some("bogus")).expect_err("must fail");
    assert!(
        error.contains("Unknown change status"),
        "unexpected error: {error}"
    );
}

#[test]
fn save_permission_rule_inserts_then_updates_same_source_table() {
    let conn = open_temp_db();

    let inserted =
        save_permission_rule(&conn, &sample_rule(Some("orders"), false)).expect("insert");
    assert!(!inserted.write);

    let updated = save_permission_rule(&conn, &sample_rule(Some("orders"), true)).expect("upsert");
    assert_eq!(
        updated.id, inserted.id,
        "same source/table must update, not insert"
    );
    assert!(updated.write);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM permission_rules WHERE source_id = 'rule-source'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 1);
}

#[test]
fn save_permission_rule_upserts_source_wide_null_table_rule() {
    let conn = open_temp_db();

    let inserted = save_permission_rule(&conn, &sample_rule(None, false)).expect("insert");
    let updated = save_permission_rule(&conn, &sample_rule(None, true)).expect("upsert");

    assert_eq!(
        updated.id, inserted.id,
        "NULL table_id rules must upsert too"
    );
    assert!(updated.table_id.is_none());
}

#[test]
fn save_permission_rule_rejects_unknown_confirmation_action() {
    let conn = open_temp_db();
    let mut rule = sample_rule(Some("orders"), true);
    rule.require_confirmation_for = vec!["drop_table".to_string()];

    let error = save_permission_rule(&conn, &rule).expect_err("must fail");
    assert!(error.contains("drop_table"), "unexpected error: {error}");
}

#[test]
fn delete_permission_rule_removes_row_and_audits() {
    let conn = open_temp_db();
    let inserted = save_permission_rule(&conn, &sample_rule(Some("orders"), true)).expect("insert");

    delete_permission_rule(&conn, inserted.id).expect("delete");
    let error = delete_permission_rule(&conn, inserted.id).expect_err("second delete must fail");
    assert!(error.contains("not found"), "unexpected error: {error}");

    let audits = list_audit_events(&conn, None, None).expect("audit list");
    assert!(audits
        .iter()
        .any(|event| event.action == "permission_rule_deleted"));
}

#[test]
fn read_table_paginates_and_reports_full_total() {
    let conn = open_temp_db();

    // Seed provides 3 records in mock-source/customers.
    let first_page = read_table(&conn, TEST_SOURCE, TEST_TABLE, Some(2), Some(0)).expect("page 1");
    assert_eq!(first_page.records.len(), 2);
    assert_eq!(first_page.total, 3, "total must ignore limit/offset");
    assert_eq!(
        first_page.records[0].id, "rec_seed_1",
        "must order by position"
    );

    let second_page = read_table(&conn, TEST_SOURCE, TEST_TABLE, Some(2), Some(2)).expect("page 2");
    assert_eq!(second_page.records.len(), 1);
    assert_eq!(second_page.records[0].id, "rec_seed_3");
    assert_eq!(second_page.total, 3);
}

#[test]
fn read_table_clamps_limit_and_offset() {
    let conn = open_temp_db();

    let clamped_low = read_table(&conn, TEST_SOURCE, TEST_TABLE, Some(0), Some(-5)).expect("read");
    assert_eq!(clamped_low.records.len(), 1, "limit must clamp up to 1");
    assert_eq!(
        clamped_low.records[0].id, "rec_seed_1",
        "offset must clamp to 0"
    );

    let defaulted = read_table(&conn, TEST_SOURCE, TEST_TABLE, None, None).expect("read");
    assert_eq!(
        defaulted.records.len(),
        3,
        "default limit covers all seed rows"
    );
}

#[test]
fn app_status_flags_fresh_and_stale_heartbeats() {
    let conn = open_temp_db();

    let stale = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(!stale.mcp_running, "no heartbeat row means not running");
    assert_eq!(stale.pending_count, 0);

    conn.execute(
        "INSERT INTO mcp_heartbeat (pid, started_at, last_seen) VALUES (4242, ?1, ?1)",
        [crate::db::now_iso()],
    )
    .expect("insert heartbeat");
    insert_pending_change(&conn, "chg_status", "pending");

    let fresh = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(fresh.mcp_running);
    assert_eq!(fresh.mcp_pid, Some(4242));
    assert_eq!(fresh.pending_count, 1);

    conn.execute(
        "UPDATE mcp_heartbeat SET last_seen = '2020-01-01T00:00:00.000Z' WHERE pid = 4242",
        [],
    )
    .expect("stale heartbeat");
    let stale_again = app_status(&conn, "0.0.0".into(), "test.db".into()).expect("status");
    assert!(
        !stale_again.mcp_running,
        "heartbeat older than 30s means not running"
    );
}

#[test]
fn describe_table_errors_on_unknown_table() {
    let conn = open_temp_db();
    let error = describe_table(&conn, TEST_SOURCE, "nope").expect_err("must fail");
    assert!(error.contains("Unknown table"), "unexpected error: {error}");
}

#[test]
fn list_tables_returns_empty_for_unknown_source() {
    let conn = open_temp_db();
    let tables = list_tables(&conn, "does-not-exist").expect("list");
    assert!(tables.is_empty());
}
