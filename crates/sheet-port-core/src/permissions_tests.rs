//! Ports the PermissionService and PermissionStore vitest suites onto the
//! real SQLite layer. Every test opens an isolated temp-file database.

use super::*;
use crate::audit;
use crate::db::test_support::open_temp_db;

const SOURCE: &str = "src-a";
const TABLE: &str = "t1";

fn make_rule(table_id: Option<&str>) -> SavePermissionRule {
    SavePermissionRule {
        id: None,
        source_id: SOURCE.to_string(),
        table_id: table_id.map(str::to_string),
        read: true,
        write: true,
        delete_records: false,
    }
}

fn save(conn: &rusqlite::Connection, rule: &SavePermissionRule) -> PermissionRuleRow {
    save_rule(conn, rule).expect("save rule")
}

// ---------------------------------------------------------------------------
// Read gating
// ---------------------------------------------------------------------------

#[test]
fn read_denied_when_no_rule_exists() {
    let conn = open_temp_db();
    let error = assert_can_read(&conn, SOURCE, Some(TABLE)).expect_err("must deny");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
    assert_eq!(
        error.to_string(),
        format!("Read access denied for {SOURCE}/{TABLE}")
    );
}

#[test]
fn read_denied_when_rule_disables_read() {
    let conn = open_temp_db();
    let mut rule = make_rule(None);
    rule.read = false;
    save(&conn, &rule);

    let error = assert_can_read(&conn, SOURCE, Some(TABLE)).expect_err("must deny");
    assert_eq!(
        error.to_string(),
        format!("Read access denied for {SOURCE}/{TABLE}")
    );
}

#[test]
fn read_allowed_when_rule_enables_read() {
    let conn = open_temp_db();
    save(&conn, &make_rule(None));
    assert_can_read(&conn, SOURCE, Some(TABLE)).expect("read allowed");
}

#[test]
fn read_error_omits_table_when_checking_source_scope() {
    let conn = open_temp_db();
    let error = assert_can_read(&conn, SOURCE, None).expect_err("must deny");
    assert_eq!(
        error.to_string(),
        format!("Read access denied for {SOURCE}")
    );
}

#[test]
fn table_specific_rule_beats_source_wide_rule() {
    let conn = open_temp_db();
    save(&conn, &make_rule(None));
    let mut table_rule = make_rule(Some(TABLE));
    table_rule.read = false;
    save(&conn, &table_rule);

    assert!(assert_can_read(&conn, SOURCE, Some(TABLE)).is_err());
    assert_can_read(&conn, SOURCE, Some("other-table")).expect("falls back to source-wide");
}

#[test]
fn find_rule_returns_source_wide_when_no_table_given() {
    let conn = open_temp_db();
    let mut source_rule = make_rule(None);
    source_rule.read = false;
    save(&conn, &source_rule);
    save(&conn, &make_rule(Some(TABLE)));

    let rule = find_rule(&conn, SOURCE, None)
        .expect("find")
        .expect("rule exists");
    assert!(rule.table_id.is_none());
    assert!(!rule.read);
}

// ---------------------------------------------------------------------------
// Write gating
// ---------------------------------------------------------------------------

#[test]
fn write_denied_when_rule_disables_write() {
    let conn = open_temp_db();
    let mut rule = make_rule(None);
    rule.write = false;
    save(&conn, &rule);

    let evaluation = evaluate_write(&conn, SOURCE, TABLE, WriteAction::Update).expect("evaluate");
    assert!(!evaluation.allowed);
    assert_eq!(
        evaluation.reason.as_deref(),
        Some(format!("Write access denied for {SOURCE}/{TABLE}").as_str())
    );
}

#[test]
fn delete_denied_without_delete_permission_even_with_write() {
    let conn = open_temp_db();
    save(&conn, &make_rule(None));

    let evaluation = evaluate_write(&conn, SOURCE, TABLE, WriteAction::Delete).expect("evaluate");
    assert!(!evaluation.allowed);
    assert_eq!(
        evaluation.reason.as_deref(),
        Some(format!("Delete access denied for {SOURCE}/{TABLE}").as_str())
    );
}

#[test]
fn delete_allowed_when_delete_records_enabled() {
    let conn = open_temp_db();
    let mut rule = make_rule(None);
    rule.delete_records = true;
    save(&conn, &rule);

    let evaluation = evaluate_write(&conn, SOURCE, TABLE, WriteAction::Delete).expect("evaluate");
    assert!(evaluation.allowed);
}

#[test]
fn delete_sheet_is_gated_on_the_delete_permission_like_a_record_delete() {
    let conn = open_temp_db();
    save(&conn, &make_rule(None));

    // Write-only rule: deleting a whole tab is refused, since write access
    // alone (without delete_records) cannot authorize it.
    let denied = evaluate_write(&conn, SOURCE, TABLE, WriteAction::DeleteSheet).expect("evaluate");
    assert!(!denied.allowed);

    let mut with_delete = make_rule(None);
    with_delete.delete_records = true;
    save(&conn, &with_delete);
    let allowed = evaluate_write(&conn, SOURCE, TABLE, WriteAction::DeleteSheet).expect("evaluate");
    assert!(
        allowed.allowed,
        "the Bypass delete permission authorizes it"
    );
}

#[test]
fn create_spreadsheet_resolves_the_source_wide_rule_with_an_empty_table() {
    let conn = open_temp_db();

    // No rule yet: a source-level create is denied and the reason names the
    // source, not a source/table pair.
    let denied =
        evaluate_write(&conn, SOURCE, "", WriteAction::CreateSpreadsheet).expect("evaluate");
    assert!(!denied.allowed);
    assert_eq!(
        denied.reason.as_deref(),
        Some(format!("Write access denied for {SOURCE}").as_str())
    );

    // A source-wide write rule (table_id = null) authorizes it.
    save(&conn, &make_rule(None));
    let allowed =
        evaluate_write(&conn, SOURCE, "", WriteAction::CreateSpreadsheet).expect("evaluate");
    assert!(allowed.allowed);
}

#[test]
fn assert_can_write_returns_typed_permission_denied() {
    let conn = open_temp_db();
    let mut rule = make_rule(None);
    rule.write = false;
    save(&conn, &rule);

    let error = assert_can_write(&conn, SOURCE, TABLE, WriteAction::Append).expect_err("deny");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
}

#[test]
fn legacy_confirmation_list_does_not_affect_write_evaluation() {
    let conn = open_temp_db();
    save(&conn, &make_rule(None));
    // Rules saved before the approval gate was removed may still list actions
    // in the legacy column; evaluation ignores them.
    conn.execute(
        "UPDATE permission_rules SET require_confirmation = '[\"update\",\"bulk_update\"]'
         WHERE source_id = ?1",
        [SOURCE],
    )
    .expect("legacy confirmation list");

    for action in [
        WriteAction::Update,
        WriteAction::BulkUpdate,
        WriteAction::Append,
    ] {
        let evaluation = evaluate_write(&conn, SOURCE, TABLE, action).expect("evaluate");
        assert_eq!(
            evaluation,
            WriteEvaluation {
                allowed: true,
                reason: None
            }
        );
    }
    assert_can_write(&conn, SOURCE, TABLE, WriteAction::BulkUpdate).expect("allowed");
}

#[test]
fn rule_changes_apply_between_calls_without_caching() {
    let conn = open_temp_db();
    save(&conn, &make_rule(None));
    assert_can_write(&conn, SOURCE, TABLE, WriteAction::Update).expect("allowed");

    // The desktop app revokes write access; the next call sees the new rule.
    let mut revoked = make_rule(None);
    revoked.write = false;
    save(&conn, &revoked);

    let error = assert_can_write(&conn, SOURCE, TABLE, WriteAction::Update).expect_err("deny");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
}

// ---------------------------------------------------------------------------
// Desktop-side rule management
// ---------------------------------------------------------------------------

#[test]
fn save_rule_inserts_then_updates_same_source_table() {
    let conn = open_temp_db();

    let mut first = make_rule(Some("orders"));
    first.write = false;
    let inserted = save(&conn, &first);
    assert!(!inserted.write);

    let updated = save(&conn, &make_rule(Some("orders")));
    assert_eq!(
        updated.id, inserted.id,
        "same source/table must update, not insert"
    );
    assert!(updated.write);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM permission_rules WHERE source_id = ?1",
            [SOURCE],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 1);

    let audits = audit::list(&conn, None, None).expect("audit list");
    assert!(audits
        .iter()
        .any(|event| event.action == "permission_rule_saved" && event.actor == AuditActor::User));
}

#[test]
fn save_rule_upserts_source_wide_null_table_rule() {
    let conn = open_temp_db();

    let mut first = make_rule(None);
    first.write = false;
    let inserted = save(&conn, &first);
    let updated = save(&conn, &make_rule(None));

    assert_eq!(
        updated.id, inserted.id,
        "NULL table_id rules must upsert too"
    );
    assert!(updated.table_id.is_none());
    assert!(updated.write);
}

#[test]
fn save_rule_stores_an_empty_legacy_confirmation_list() {
    let conn = open_temp_db();
    let inserted = save(&conn, &make_rule(Some("orders")));
    save(&conn, &make_rule(Some("orders")));

    let stored: String = conn
        .query_row(
            "SELECT require_confirmation FROM permission_rules WHERE id = ?1",
            [inserted.id],
            |row| row.get(0),
        )
        .expect("stored list");
    assert_eq!(stored, "[]", "insert and update both write an empty list");
}

#[test]
fn delete_rule_removes_row_and_audits() {
    let conn = open_temp_db();
    let inserted = save(&conn, &make_rule(Some("orders")));

    delete_rule(&conn, inserted.id).expect("delete");
    let error = delete_rule(&conn, inserted.id).expect_err("second delete must fail");
    assert!(
        error.to_string().contains("not found"),
        "unexpected error: {error}"
    );

    let audits = audit::list(&conn, None, None).expect("audit list");
    assert!(audits
        .iter()
        .any(|event| event.action == "permission_rule_deleted"));
}

#[test]
fn fresh_database_has_no_permission_rules() {
    let conn = open_temp_db();
    assert!(list_rules(&conn).expect("list").is_empty());
}

#[test]
fn list_rules_includes_demo_fixture_customers_rule() {
    let conn = crate::test_fixtures::demo_db();
    let rules = list_rules(&conn).expect("list");
    assert!(rules.iter().any(|rule| {
        rule.source_id == crate::test_fixtures::DEMO_SOURCE_ID
            && rule.table_id.as_deref() == Some(crate::test_fixtures::DEMO_TABLE_ID)
            && rule.read
            && rule.write
            && !rule.delete_records
    }));
}
