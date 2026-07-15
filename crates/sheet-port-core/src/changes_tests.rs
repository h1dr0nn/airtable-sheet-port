//! Ports the ChangeService and ChangeStore vitest suites onto the real
//! SQLite layer plus the mock connector. Covers every commit enforcement
//! branch; error wording is asserted verbatim because agents match on it.
//! Fresh databases are empty since schema v2, so each test opens a demo_db
//! with the mock-source/customers fixture installed.

use rusqlite::{params, Connection};
use serde_json::json;

use super::*;
use crate::connectors::ConnectorRegistry;
use crate::constants::META_FLAG_ON;
use crate::permissions::save_rule;
use crate::test_fixtures::{demo_db, DEMO_SOURCE_ID, DEMO_TABLE_ID};
use crate::types::SavePermissionRule;

const SOURCE: &str = DEMO_SOURCE_ID;
const TABLE: &str = DEMO_TABLE_ID;

fn registry() -> ConnectorRegistry {
    ConnectorRegistry::with_default_connectors()
}

/// Overwrites the fixture customers rule so each test controls the policy.
fn set_rule(conn: &Connection, write: bool, require_confirmation_for: &[&str]) {
    let rule = SavePermissionRule {
        id: None,
        source_id: SOURCE.to_string(),
        table_id: Some(TABLE.to_string()),
        read: true,
        write,
        delete_records: false,
        require_confirmation_for: require_confirmation_for
            .iter()
            .map(|action| action.to_string())
            .collect(),
    };
    save_rule(conn, &rule).expect("set rule");
}

fn fields(pairs: &[(&str, serde_json::Value)]) -> JsonMap {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

fn patch(record_id: &str, pairs: &[(&str, serde_json::Value)]) -> RecordPatch {
    RecordPatch {
        record_id: record_id.to_string(),
        fields: fields(pairs),
    }
}

fn seed_1_fields() -> serde_json::Value {
    json!({
        "Name": "Aurora Labs",
        "Email": "ops@auroralabs.dev",
        "Plan": "pro",
        "Seats": 24,
        "Active": true
    })
}

// ---------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------

#[test]
fn append_change_is_pending_with_after_only_diff() {
    let conn = demo_db();
    let records = vec![fields(&[("Name", json!("Delta"))])];

    let change = create_append_change(&conn, SOURCE, TABLE, records.clone(), true).expect("create");

    assert_eq!(change.status, ChangeStatus::Pending);
    assert_eq!(change.change_type, ChangeType::Append);
    assert!(change.requires_confirmation);
    assert_eq!(change.diff, json!({ "after": [{ "Name": "Delta" }] }));
    assert!(change.id.starts_with("chg_"));

    let payload = get_payload(&conn, &change.id)
        .expect("payload")
        .expect("stored");
    assert_eq!(
        payload,
        ChangePayload::Append {
            records,
            format: None
        }
    );
}

#[test]
fn update_change_builds_before_after_diff_per_record() {
    let conn = demo_db();
    let patches = vec![
        patch("rec_seed_1", &[("Seats", json!(25))]),
        patch("rec_missing", &[("Name", json!("Ghost"))]),
    ];

    let change =
        create_update_change(&conn, &registry(), SOURCE, TABLE, patches, false).expect("create");

    let mut expected_after = seed_1_fields();
    expected_after["Seats"] = json!(25);
    assert_eq!(
        change.diff,
        json!([
            { "recordId": "rec_seed_1", "before": seed_1_fields(), "after": expected_after },
            { "recordId": "rec_missing", "before": null, "after": { "Name": "Ghost" } }
        ])
    );
}

#[test]
fn change_serialization_hides_payload_and_absent_optionals() {
    let conn = demo_db();
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        true,
    )
    .expect("create");

    let listed = list_changes(&conn, Some("pending")).expect("list");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].diff.is_object(), "diff must be parsed JSON");

    let serialized = serde_json::to_string(&listed[0]).expect("serialize");
    assert!(
        !serialized.contains("payload"),
        "payload must never serialize"
    );
    assert!(
        !serialized.contains("patches"),
        "payload content must never leak"
    );
    assert!(
        !serialized.contains("decidedAt"),
        "absent optionals stay absent"
    );
    assert!(serialized.contains(&format!("\"id\":\"{}\"", change.id)));
}

// ---------------------------------------------------------------------------
// Commit enforcement
// ---------------------------------------------------------------------------

#[test]
fn commit_blocks_pending_confirmation_change_with_exact_message() {
    let conn = demo_db();
    let registry = registry();
    // Auto-approve is on by default, so opt into the confirmation gate first.
    crate::db::set_meta(&conn, META_AUTO_APPROVE_WRITES, "0").expect("disable");
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        true,
    )
    .expect("create");

    let error = commit(&conn, &registry, &change.id).expect_err("must block");
    assert_eq!(
        error.to_string(),
        format!(
            "Change {} requires user approval in the Airtable - Sheet Port desktop app before commit",
            change.id
        )
    );
    let still = get_change(&conn, &change.id).expect("get").expect("exists");
    assert_eq!(still.status, ChangeStatus::Pending);
}

#[test]
fn commit_succeeds_after_user_approval() {
    let conn = demo_db();
    let registry = registry();
    let change = create_update_change(
        &conn,
        &registry,
        SOURCE,
        TABLE,
        vec![patch("rec_seed_1", &[("Seats", json!(25))])],
        true,
    )
    .expect("create");
    assert!(transition(
        &conn,
        &change.id,
        ChangeStatus::Pending,
        ChangeStatus::Approved,
        ChangeDecider::User
    )
    .expect("approve"));

    let outcome = commit(&conn, &registry, &change.id).expect("commit");

    assert_eq!(outcome.change.status, ChangeStatus::Committed);
    assert_eq!(outcome.change.decided_by, Some(ChangeDecider::User));
    assert!(outcome.change.committed_at.is_some());
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].id, "rec_seed_1");
    assert_eq!(outcome.records[0].fields.get("Seats"), Some(&json!(25)));

    let page =
        crate::mock_data::list_records(&conn, SOURCE, TABLE, ReadOptions::default()).expect("read");
    assert_eq!(page.records[0].fields.get("Seats"), Some(&json!(25)));
}

#[test]
fn commit_auto_approves_by_policy_when_no_confirmation_needed() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        false,
    )
    .expect("create");

    let outcome = commit(&conn, &registry, &change.id).expect("commit");

    assert_eq!(outcome.change.status, ChangeStatus::Committed);
    assert_eq!(outcome.change.decided_by, Some(ChangeDecider::Policy));
    assert_eq!(outcome.records.len(), 1);
    let page =
        crate::mock_data::list_records(&conn, SOURCE, TABLE, ReadOptions::default()).expect("read");
    assert_eq!(page.total, 4, "append must land in the mock store");
}

fn freeze_only_plan() -> FormatPlan {
    FormatPlan {
        formats: Vec::new(),
        freeze_rows: Some(1),
        freeze_columns: None,
        column_widths: Vec::new(),
    }
}

#[test]
fn append_with_format_stores_the_plan_in_payload_and_diff() {
    let conn = demo_db();
    let plan = freeze_only_plan();
    let change = create_append_with_format(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        Some(plan.clone()),
        false,
    )
    .expect("create");

    assert_eq!(
        change.diff["format"]["freezeRows"],
        json!(1),
        "the format plan is part of the previewable diff"
    );
    let payload = get_payload(&conn, &change.id)
        .expect("payload")
        .expect("stored");
    match payload {
        ChangePayload::Append { format, .. } => assert_eq!(format, Some(plan)),
        other => panic!("expected an append payload, got {other:?}"),
    }
}

#[test]
fn commit_append_with_format_commits_rows_then_reports_a_format_failure() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let change = create_append_with_format(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        Some(freeze_only_plan()),
        false,
    )
    .expect("create");

    let outcome = commit(&conn, &registry, &change.id).expect("commit");

    assert_eq!(outcome.change.status, ChangeStatus::Committed);
    assert_eq!(outcome.records.len(), 1, "the rows are appended");
    assert!(
        outcome.format_error.is_some(),
        "the mock connector cannot format, so the styling failure is surfaced not swallowed"
    );
    let page =
        crate::mock_data::list_records(&conn, SOURCE, TABLE, ReadOptions::default()).expect("read");
    assert_eq!(page.total, 4, "rows land despite the format step failing");
}

#[test]
fn commit_many_commits_each_change_in_order() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let first = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("A"))])],
        false,
    )
    .expect("first");
    let second = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("B"))])],
        false,
    )
    .expect("second");

    let outcomes =
        commit_many(&conn, &registry, &[first.id.clone(), second.id.clone()]).expect("commit many");

    assert_eq!(outcomes.len(), 2);
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.change.status == ChangeStatus::Committed));
    let page =
        crate::mock_data::list_records(&conn, SOURCE, TABLE, ReadOptions::default()).expect("read");
    assert_eq!(page.total, 5, "both appends land on top of the 3 seed rows");
}

#[test]
fn commit_many_rejects_an_unknown_id_before_committing_any() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let valid = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("A"))])],
        false,
    )
    .expect("valid");

    let error = commit_many(
        &conn,
        &registry,
        &[valid.id.clone(), "chg_nope".to_string()],
    )
    .expect_err("an unknown id must fail the batch");

    assert_eq!(error.to_string(), "Unknown change chg_nope");
    let still = get_change(&conn, &valid.id).expect("get").expect("exists");
    assert_eq!(
        still.status,
        ChangeStatus::Pending,
        "the pre-flight existence check runs before any change is committed"
    );
}

#[test]
fn commit_auto_approves_confirmation_change_when_setting_on() {
    let conn = demo_db();
    let registry = registry();
    crate::db::set_meta(&conn, META_AUTO_APPROVE_WRITES, META_FLAG_ON).expect("enable");
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        true,
    )
    .expect("create");

    let outcome = commit(&conn, &registry, &change.id).expect("commit bypasses gate");

    assert_eq!(outcome.change.status, ChangeStatus::Committed);
    assert_eq!(
        outcome.change.decided_by,
        Some(ChangeDecider::Policy),
        "auto-approved writes are decided by policy, not the user"
    );
    assert_eq!(outcome.records.len(), 1);
}

#[test]
fn commit_blocks_confirmation_change_when_setting_off() {
    let conn = demo_db();
    let registry = registry();
    // Explicit "0" is the only thing that turns auto-approve off.
    crate::db::set_meta(&conn, META_AUTO_APPROVE_WRITES, "0").expect("disable");
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        true,
    )
    .expect("create");

    let error = commit(&conn, &registry, &change.id).expect_err("must block");
    assert_eq!(
        error.to_string(),
        format!(
            "Change {} requires user approval in the Airtable - Sheet Port desktop app before commit",
            change.id
        )
    );
    let still = get_change(&conn, &change.id).expect("get").expect("exists");
    assert_eq!(still.status, ChangeStatus::Pending);
}

#[test]
fn commit_rejects_change_rejected_in_desktop_app() {
    let conn = demo_db();
    let registry = registry();
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        true,
    )
    .expect("create");
    decide_change(&conn, &change.id, ChangeDecision::Reject).expect("reject");

    let error = commit(&conn, &registry, &change.id).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        format!(
            "Change {} was rejected in the desktop app and cannot be committed",
            change.id
        )
    );
}

#[test]
fn commit_rejects_double_commit() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        false,
    )
    .expect("create");
    commit(&conn, &registry, &change.id).expect("first commit");

    let error = commit(&conn, &registry, &change.id).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        format!("Change {} is already committed", change.id)
    );
}

#[test]
fn commit_rejects_unknown_change_id() {
    let conn = demo_db();
    let error = commit(&conn, &registry(), "chg_nope").expect_err("must fail");
    assert_eq!(error.to_string(), "Unknown change chg_nope");
    assert!(matches!(error, CoreError::NotFound(_)));
}

#[test]
fn commit_fails_when_write_revoked_after_preview() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        false,
    )
    .expect("create");
    set_rule(&conn, false, &[]);

    let error = commit(&conn, &registry, &change.id).expect_err("must fail");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
    let still = get_change(&conn, &change.id).expect("get").expect("exists");
    assert_eq!(
        still.status,
        ChangeStatus::Pending,
        "denied commit must not advance state"
    );
}

#[test]
fn commit_reports_missing_payload() {
    let conn = demo_db();
    let registry = registry();
    set_rule(&conn, true, &[]);
    let change = create_append_change(
        &conn,
        SOURCE,
        TABLE,
        vec![fields(&[("Name", json!("Delta"))])],
        false,
    )
    .expect("create");
    // Simulate a nullish stored payload (JSON null), the TS "no payload" case.
    conn.execute(
        "UPDATE pending_changes SET payload = 'null' WHERE id = ?1",
        [&change.id],
    )
    .expect("null payload");

    let error = commit(&conn, &registry, &change.id).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        format!("Change {} has no stored payload", change.id)
    );
}

#[test]
fn commit_action_escalates_large_updates_to_bulk() {
    let many: Vec<RecordPatch> = (0..21).map(|n| patch(&format!("rec_{n}"), &[])).collect();
    assert_eq!(
        commit_action(ChangeType::Update, &ChangePayload::Update { patches: many }),
        WriteAction::BulkUpdate
    );

    let few: Vec<RecordPatch> = (0..20).map(|n| patch(&format!("rec_{n}"), &[])).collect();
    assert_eq!(
        commit_action(ChangeType::Update, &ChangePayload::Update { patches: few }),
        WriteAction::Update
    );
    assert_eq!(
        commit_action(
            ChangeType::Append,
            &ChangePayload::Append {
                records: Vec::new(),
                format: None
            }
        ),
        WriteAction::Append
    );
    assert_eq!(
        commit_action(
            ChangeType::Delete,
            &ChangePayload::Delete {
                record_ids: Vec::new()
            }
        ),
        WriteAction::Delete
    );
}

#[test]
fn commit_refuses_delete_payloads_in_mvp() {
    let conn = demo_db();
    let registry = registry();
    let rule_write_delete = SavePermissionRule {
        id: None,
        source_id: SOURCE.to_string(),
        table_id: Some(TABLE.to_string()),
        read: true,
        write: true,
        delete_records: true,
        require_confirmation_for: Vec::new(),
    };
    save_rule(&conn, &rule_write_delete).expect("rule");
    // Insert a delete change directly: previews cannot create one yet.
    conn.execute(
        "INSERT INTO pending_changes
             (id, source_id, table_id, change_type, created_at, status,
              requires_confirmation, diff, payload)
         VALUES ('chg_del', ?1, ?2, 'delete', ?3, 'pending', 0, '{}',
                 '{\"type\":\"delete\",\"recordIds\":[\"rec_seed_1\"]}')",
        params![SOURCE, TABLE, crate::db::now_iso()],
    )
    .expect("insert delete change");

    let error = commit(&conn, &registry, "chg_del").expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "Delete changes are not implemented in the MVP"
    );
}

// ---------------------------------------------------------------------------
// Desktop decisions
// ---------------------------------------------------------------------------

fn insert_pending(conn: &Connection, id: &str, status: &str) {
    conn.execute(
        "INSERT INTO pending_changes
             (id, source_id, table_id, change_type, created_at, status,
              requires_confirmation, diff, payload)
         VALUES (?1, ?2, ?3, 'update', ?4, ?5, 1,
                 '[{\"recordId\":\"rec_seed_1\",\"before\":{},\"after\":{}}]',
                 '{\"type\":\"update\",\"patches\":[]}')",
        params![id, SOURCE, TABLE, crate::db::now_iso(), status],
    )
    .expect("insert pending change");
}

#[test]
fn approve_transitions_pending_to_approved_and_audits() {
    let conn = demo_db();
    insert_pending(&conn, "chg_1", "pending");

    let change = decide_change(&conn, "chg_1", ChangeDecision::Approve).expect("approve");

    assert_eq!(change.status, ChangeStatus::Approved);
    assert_eq!(change.decided_by, Some(ChangeDecider::User));
    assert!(change.decided_at.is_some());
    assert!(change.requires_confirmation);

    let audits = crate::audit::list(&conn, None, None).expect("audit list");
    assert!(audits.iter().any(|event| {
        event.action == "change_approved"
            && event.actor == AuditActor::User
            && event.source_id.as_deref() == Some(SOURCE)
    }));
}

#[test]
fn approve_refuses_already_approved_change() {
    let conn = demo_db();
    insert_pending(&conn, "chg_2", "pending");
    decide_change(&conn, "chg_2", ChangeDecision::Approve).expect("first approve");

    let error = decide_change(&conn, "chg_2", ChangeDecision::Approve)
        .expect_err("second approve must fail");
    assert!(
        error.to_string().contains("'approved'"),
        "error should name the actual status: {error}"
    );
}

#[test]
fn reject_refuses_non_pending_change() {
    let conn = demo_db();
    insert_pending(&conn, "chg_3", "committed");

    let error =
        decide_change(&conn, "chg_3", ChangeDecision::Reject).expect_err("reject must fail");
    assert!(
        error.to_string().contains("'committed'"),
        "error should name the actual status: {error}"
    );
}

#[test]
fn decide_change_reports_missing_change() {
    let conn = demo_db();
    let error =
        decide_change(&conn, "chg_missing", ChangeDecision::Approve).expect_err("must fail");
    assert!(
        error.to_string().contains("not found"),
        "unexpected error: {error}"
    );
}

// ---------------------------------------------------------------------------
// Store guards and listing
// ---------------------------------------------------------------------------

#[test]
fn transition_guards_by_expected_from_status() {
    let conn = demo_db();
    insert_pending(&conn, "chg_g", "pending");

    let first = transition(
        &conn,
        "chg_g",
        ChangeStatus::Pending,
        ChangeStatus::Approved,
        ChangeDecider::User,
    )
    .expect("first");
    let second = transition(
        &conn,
        "chg_g",
        ChangeStatus::Pending,
        ChangeStatus::Approved,
        ChangeDecider::User,
    )
    .expect("second");

    assert!(first);
    assert!(!second, "approving twice must miss the guard");
}

#[test]
fn mark_committed_only_from_approved_status() {
    let conn = demo_db();
    insert_pending(&conn, "chg_m", "pending");

    assert!(!mark_committed(&conn, "chg_m").expect("pending cannot commit"));
    assert!(transition(
        &conn,
        "chg_m",
        ChangeStatus::Pending,
        ChangeStatus::Approved,
        ChangeDecider::User
    )
    .expect("approve"));
    assert!(mark_committed(&conn, "chg_m").expect("approved commits"));
    assert!(!mark_committed(&conn, "chg_m").expect("double commit misses"));

    let change = get_change(&conn, "chg_m").expect("get").expect("exists");
    assert_eq!(change.status, ChangeStatus::Committed);
    assert!(change.committed_at.is_some());
}

#[test]
fn list_changes_filters_by_status_newest_first() {
    let conn = demo_db();
    conn.execute_batch(
        "INSERT INTO pending_changes
             (id, source_id, table_id, change_type, created_at, status,
              requires_confirmation, diff, payload)
         VALUES
           ('chg_a', 'mock-source', 'customers', 'update', '2026-01-01T00:00:00.000Z',
            'approved', 1, '[]', '{\"type\":\"update\",\"patches\":[]}'),
           ('chg_b', 'mock-source', 'customers', 'update', '2026-01-02T00:00:00.000Z',
            'pending', 1, '[]', '{\"type\":\"update\",\"patches\":[]}');",
    )
    .expect("insert");

    let all = list_changes(&conn, None).expect("all");
    assert_eq!(
        all.iter()
            .map(|change| change.id.as_str())
            .collect::<Vec<_>>(),
        ["chg_b", "chg_a"]
    );
    let pending = list_changes(&conn, Some("pending")).expect("pending");
    assert_eq!(
        pending
            .iter()
            .map(|change| change.id.as_str())
            .collect::<Vec<_>>(),
        ["chg_b"]
    );
}

#[test]
fn list_changes_rejects_unknown_status() {
    let conn = demo_db();
    let error = list_changes(&conn, Some("bogus")).expect_err("must fail");
    assert!(
        error.to_string().contains("Unknown change status"),
        "unexpected error: {error}"
    );
    assert!(matches!(error, CoreError::InvalidInput(_)));
}
