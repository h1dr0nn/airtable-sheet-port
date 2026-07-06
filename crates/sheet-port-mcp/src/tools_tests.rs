//! Behavior tests against an isolated temp database. Fresh databases start
//! empty since seed v2, so every test state installs its own demo workspace
//! first (mirror of the core crate's test fixture). The protocol e2e
//! (scripts/e2e-smoke.mjs) covers the wire format; these tests pin the tool
//! semantics: statuses, payload hiding, approval enforcement wording, bulk
//! escalation, and audit self-recording.

use serde_json::Value;
use sheet_port_core::db;
use sheet_port_core::rusqlite::{params, Connection};

use super::*;
use crate::state::BrokerState;

const DEMO_TABLE_FIELDS: &str = r#"[{"name":"Name","type":"string","required":true},{"name":"Email","type":"email"},{"name":"Plan","type":"enum","enumValues":["free","pro","enterprise"]},{"name":"Seats","type":"number"},{"name":"Active","type":"boolean"}]"#;

const DEMO_RECORDS: [(&str, &str); 3] = [
    (
        "rec_seed_1",
        r#"{"Name":"Aurora Labs","Email":"ops@auroralabs.dev","Plan":"pro","Seats":24,"Active":true}"#,
    ),
    (
        "rec_seed_2",
        r#"{"Name":"Basalt Co","Email":"it@basalt.co","Plan":"free","Seats":3,"Active":true}"#,
    ),
    (
        "rec_seed_3",
        r#"{"Name":"Cirrus Retail","Email":"admin@cirrus.shop","Plan":"enterprise","Seats":180,"Active":false}"#,
    ),
];

/// The demo workspace the v1 seed used to ship: mock source, Customers table
/// with rec_seed_1..3, and a read+write rule requiring confirmation for
/// every write action.
fn install_demo_workspace(conn: &Connection) {
    conn.execute(
        "INSERT INTO sources (id, kind, name, status)
         VALUES ('mock-source', 'mock', 'Test Workspace', 'connected')",
        [],
    )
    .expect("insert demo source");
    conn.execute(
        "INSERT INTO mock_tables (source_id, table_id, name, fields)
         VALUES ('mock-source', 'customers', 'Customers', ?1)",
        params![DEMO_TABLE_FIELDS],
    )
    .expect("insert demo table");
    for (position, (record_id, fields)) in DEMO_RECORDS.iter().enumerate() {
        conn.execute(
            "INSERT INTO mock_records (source_id, table_id, record_id, fields, position)
             VALUES ('mock-source', 'customers', ?1, ?2, ?3)",
            params![record_id, fields, position as i64 + 1],
        )
        .expect("insert demo record");
    }
    conn.execute(
        "INSERT INTO permission_rules
             (source_id, table_id, can_read, can_write, can_delete,
              require_confirmation, updated_at)
         VALUES ('mock-source', 'customers', 1, 1, 0,
                 '[\"append\",\"update\",\"delete\",\"bulk_update\"]', ?1)",
        params![db::now_iso()],
    )
    .expect("insert demo rule");
}

fn temp_state() -> BrokerState {
    let path = std::env::temp_dir()
        .join("sheet-port-mcp-tests")
        .join(format!("{}.db", uuid::Uuid::new_v4()));
    let conn = db::open_at(&path).expect("temp db should open");
    install_demo_workspace(&conn);
    BrokerState::new(conn)
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("tool output should be valid JSON")
}

fn patch_args(patch_count: usize) -> PreviewUpdateArgs {
    let mut fields = sheet_port_core::types::JsonMap::new();
    fields.insert("Seats".to_string(), Value::from(25));
    PreviewUpdateArgs {
        source_id: "mock-source".to_string(),
        table_id: "customers".to_string(),
        patches: (0..patch_count)
            .map(|index| crate::args::PatchArg {
                record_id: format!("rec_seed_{}", (index % 3) + 1),
                fields: fields.clone(),
            })
            .collect(),
    }
}

fn preview(state: &BrokerState, patch_count: usize) -> Value {
    let text =
        preview_update_records(state, patch_args(patch_count)).expect("preview should succeed");
    parse(&text)
}

#[test]
fn list_sources_returns_only_connector_backed_sources() {
    let state = temp_state();
    let output = parse(&list_sources(&state).expect("list_sources"));
    let sources = output["sources"].as_array().expect("sources array");
    assert_eq!(sources.len(), 1, "placeholder sources must be hidden");
    assert_eq!(sources[0]["id"], "mock-source");
    assert_eq!(sources[0]["kind"], "mock");
}

#[test]
fn preview_update_creates_pending_change_without_payload() {
    let state = temp_state();
    let output = preview(&state, 1);
    assert_eq!(output["requiresConfirmation"], true);
    assert_eq!(output["change"]["status"], "pending");
    assert_eq!(output["change"]["type"], "update");
    let change = output["change"].as_object().expect("change object");
    assert!(
        !change.contains_key("payload"),
        "payload must never reach agents"
    );
    let diff = output["change"]["diff"].as_array().expect("diff array");
    assert_eq!(diff[0]["recordId"], "rec_seed_1");
    assert_eq!(diff[0]["after"]["Seats"], 25);
}

#[test]
fn commit_requires_approval_then_commits_once() {
    let state = temp_state();
    let change_id = preview(&state, 1)["change"]["id"]
        .as_str()
        .expect("change id")
        .to_string();
    let commit_args = CommitChangeArgs {
        change_id: change_id.clone(),
    };

    let blocked = commit_change(&state, &commit_args).expect_err("commit must be blocked");
    assert!(
        blocked.to_string().to_lowercase().contains("approval"),
        "error must explain approval is needed: {blocked}"
    );

    // Simulate the desktop app approving the change (same SQL the desktop
    // command runs).
    state
        .with_conn(|conn, _| {
            let updated = conn
                .execute(
                    "UPDATE pending_changes SET status='approved', decided_at=?1, \
                     decided_by='user' WHERE id=?2 AND status='pending'",
                    params![db::now_iso(), change_id],
                )
                .expect("approval update");
            assert_eq!(updated, 1, "approval should hit the pending row");
            Ok(())
        })
        .expect("approve via SQL");

    let committed = parse(&commit_change(&state, &commit_args).expect("commit after approval"));
    assert_eq!(committed["change"]["status"], "committed");
    assert_eq!(committed["records"][0]["fields"]["Seats"], 25);

    let again = commit_change(&state, &commit_args).expect_err("double commit must fail");
    assert!(again.to_string().contains("already committed"));

    let table = parse(
        &read_table(
            &state,
            &ReadTableArgs {
                source_id: "mock-source".to_string(),
                table_id: "customers".to_string(),
                limit: Some(10),
                offset: None,
            },
        )
        .expect("read_table"),
    );
    let records = table["records"].as_array().expect("records");
    let seed1 = records
        .iter()
        .find(|record| record["id"] == "rec_seed_1")
        .expect("rec_seed_1 present");
    assert_eq!(seed1["fields"]["Seats"], 25, "patch visible in table data");
}

#[test]
fn commit_unknown_change_reports_contract_message() {
    let state = temp_state();
    let error = commit_change(
        &state,
        &CommitChangeArgs {
            change_id: "chg_missing".to_string(),
        },
    )
    .expect_err("unknown change must fail");
    assert_eq!(error.to_string(), "Unknown change chg_missing");
}

#[test]
fn large_updates_escalate_to_bulk_update_action() {
    let state = temp_state();
    // Rule change: only bulk_update needs confirmation, so the escalation is
    // observable through requiresConfirmation.
    state
        .with_conn(|conn, _| {
            conn.execute(
                "UPDATE permission_rules SET require_confirmation='[\"bulk_update\"]' \
                 WHERE source_id='mock-source'",
                [],
            )
            .expect("rule update");
            Ok(())
        })
        .expect("update rule");

    assert_eq!(
        preview(&state, 1)["requiresConfirmation"],
        false,
        "small updates evaluate the update action"
    );
    assert_eq!(
        preview(&state, 21)["requiresConfirmation"],
        true,
        "updates above the threshold evaluate bulk_update"
    );
}

#[test]
fn tools_self_record_audit_events() {
    let state = temp_state();
    let _ = preview(&state, 1);
    append_records(
        &state,
        AppendRecordsArgs {
            source_id: "mock-source".to_string(),
            table_id: "customers".to_string(),
            records: vec![sheet_port_core::types::JsonMap::new()],
        },
    )
    .expect("append preview");

    let output =
        parse(&get_audit_log(&state, &GetAuditLogArgs { limit: Some(50) }).expect("get_audit_log"));
    let actions: Vec<&str> = output["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter_map(|event| event["action"].as_str())
        .collect();
    assert_eq!(actions[0], "get_audit_log", "newest first, self-audited");
    assert!(actions.contains(&"preview_update_records"));
    assert!(actions.contains(&"append_records_preview"));
}
