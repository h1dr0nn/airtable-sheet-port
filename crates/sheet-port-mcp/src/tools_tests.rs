//! Behavior tests against an isolated temp database. Fresh databases start
//! empty since seed v2, so every test state installs its own demo workspace
//! first (mirror of the core crate's test fixture). The protocol e2e
//! (scripts/e2e-smoke.mjs) covers the wire format; these tests pin the tool
//! semantics: direct writes vs dryRun staging, payload hiding, source
//! resolution, permission wording, and audit self-recording.

use serde_json::Value;
use sheet_port_core::db;
use sheet_port_core::rusqlite::{params, Connection};

use super::*;
use crate::state::BrokerState;

const SOURCE: &str = "mock-source";
const TABLE: &str = "customers";

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
/// with rec_seed_1..3, and a read+write (no delete) rule on the table.
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
             (source_id, table_id, can_read, can_write, can_delete, updated_at)
         VALUES ('mock-source', 'customers', 1, 1, 0, ?1)",
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

fn exec(state: &BrokerState, sql: &str) {
    state
        .with_conn(|conn, _| {
            conn.execute(sql, []).expect("sql");
            Ok(())
        })
        .expect("exec");
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("tool output should be valid JSON")
}

fn source() -> Option<String> {
    Some(SOURCE.to_string())
}

fn patch_args(patch_count: usize, dry_run: bool) -> UpdateRecordsArgs {
    let mut fields = sheet_port_core::types::JsonMap::new();
    fields.insert("Seats".to_string(), Value::from(25));
    UpdateRecordsArgs {
        source_id: source(),
        table_id: TABLE.to_string(),
        patches: (0..patch_count)
            .map(|index| crate::args::PatchArg {
                record_id: format!("rec_seed_{}", (index % 3) + 1),
                fields: fields.clone(),
            })
            .collect(),
        dry_run,
    }
}

fn read_args() -> ReadTableArgs {
    ReadTableArgs {
        source_id: source(),
        table_id: TABLE.to_string(),
        limit: None,
        offset: None,
    }
}

fn read_records(state: &BrokerState) -> Vec<Value> {
    parse(&read_table(state, &read_args()).expect("read_table"))["records"]
        .as_array()
        .expect("records")
        .clone()
}

fn append_args(dry_run: bool) -> AppendRecordsArgs {
    AppendRecordsArgs {
        source_id: source(),
        table_id: TABLE.to_string(),
        records: vec![sheet_port_core::types::JsonMap::new()],
        format: crate::args::FormatSpec::default(),
        dry_run,
    }
}

fn audit_events(state: &BrokerState) -> Vec<Value> {
    parse(&get_audit_log(state, &GetAuditLogArgs { limit: Some(50) }).expect("get_audit_log"))
        ["events"]
        .as_array()
        .expect("events")
        .clone()
}

#[test]
fn list_sources_returns_only_connector_backed_sources() {
    let state = temp_state();
    let output = parse(&list_sources(&state).expect("list_sources"));
    let sources = output["sources"].as_array().expect("sources array");
    assert_eq!(sources.len(), 1, "placeholder sources must be hidden");
    assert_eq!(sources[0]["id"], SOURCE);
    assert_eq!(sources[0]["kind"], "mock");
}

#[test]
fn omitted_source_id_routes_to_google_and_reports_when_none_is_connected() {
    let state = temp_state();
    let error = read_table(
        &state,
        &ReadTableArgs {
            source_id: None,
            ..read_args()
        },
    )
    .expect_err("no bridge is connected");
    assert!(
        error.to_string().contains("Google Sheets is not connected"),
        "unexpected error: {error}"
    );
}

#[test]
fn update_records_applies_immediately_and_returns_the_diff() {
    let state = temp_state();
    let output = parse(&update_records(&state, patch_args(1, false)).expect("update"));
    assert_eq!(output["committed"], true);
    assert_eq!(output["change"]["type"], "update");
    let change = output["change"].as_object().expect("change object");
    assert!(
        !change.contains_key("payload"),
        "payload must never reach agents"
    );
    assert!(!change.contains_key("requiresConfirmation"));
    let diff = output["change"]["diff"].as_array().expect("diff array");
    assert_eq!(diff[0]["recordId"], "rec_seed_1");
    assert_eq!(diff[0]["before"]["Seats"], 24);
    assert_eq!(diff[0]["after"]["Seats"], 25);
    // Lean shape: the committed change appears once, outcome fields flattened.
    assert_eq!(output["change"]["status"], "committed");
    assert_eq!(output["records"][0]["fields"]["Seats"], 25);
    assert!(output.get("outcome").is_none(), "no nested outcome");
    let keys: Vec<&str> = output
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["change", "committed", "records"]);

    let seed1 = read_records(&state)
        .into_iter()
        .find(|record| record["id"] == "rec_seed_1")
        .expect("rec_seed_1 present");
    assert_eq!(seed1["fields"]["Seats"], 25, "patch visible in table data");
}

#[test]
fn dry_run_stages_without_writing_then_commit_change_applies_once() {
    let state = temp_state();
    let output = parse(&update_records(&state, patch_args(1, true)).expect("dry run"));
    assert_eq!(output["committed"], false);
    assert_eq!(output["change"]["status"], "pending");
    assert_eq!(
        output.as_object().expect("object").len(),
        2,
        "a dry run is just change + committed"
    );
    let seed1 = read_records(&state)
        .into_iter()
        .find(|record| record["id"] == "rec_seed_1")
        .expect("rec_seed_1 present");
    assert_eq!(seed1["fields"]["Seats"], 24, "a dry run writes nothing");

    let commit_args = CommitChangeArgs {
        change_id: output["change"]["id"].as_str().map(str::to_string),
        change_ids: None,
    };
    let committed = parse(&commit_change(&state, &commit_args).expect("commit"));
    assert_eq!(committed["committed"], true);
    assert_eq!(committed["change"]["status"], "committed");
    assert_eq!(committed["records"][0]["fields"]["Seats"], 25);

    let again = commit_change(&state, &commit_args).expect_err("double commit must fail");
    assert!(again.to_string().contains("already committed"));
}

#[test]
fn large_updates_still_apply_as_bulk_updates() {
    let state = temp_state();
    let output = parse(&update_records(&state, patch_args(21, false)).expect("bulk update"));
    assert_eq!(output["committed"], true);
    assert_eq!(output["change"]["diff"].as_array().expect("diff").len(), 21);
}

#[test]
fn commit_unknown_change_reports_contract_message() {
    let state = temp_state();
    let error = commit_change(
        &state,
        &CommitChangeArgs {
            change_id: Some("chg_missing".to_string()),
            change_ids: None,
        },
    )
    .expect_err("unknown change must fail");
    assert_eq!(error.to_string(), "Unknown change chg_missing");
}

#[test]
fn write_tools_record_one_audit_event_named_after_the_tool() {
    let state = temp_state();
    update_records(&state, patch_args(1, true)).expect("dry-run update");
    append_records(&state, append_args(false)).expect("append");

    let events = audit_events(&state);
    let actions: Vec<&str> = events
        .iter()
        .filter_map(|event| event["action"].as_str())
        .collect();
    assert_eq!(actions[0], "get_audit_log", "newest first, self-audited");
    assert_eq!(
        actions
            .iter()
            .filter(|action| **action == "append_records")
            .count(),
        1,
        "one audit event per call"
    );
    let update = events
        .iter()
        .find(|event| event["action"] == "update_records")
        .expect("update_records audited");
    assert_eq!(update["sourceId"], SOURCE, "audit uses the resolved source");
    let metadata = update["metadata"].to_string();
    assert!(metadata.contains("\"dryRun\":true"), "{metadata}");
}

#[test]
fn list_sheets_returns_the_tabs_of_the_spreadsheet() {
    let state = temp_state();
    // Spreadsheet ids have no table rule in the demo; grant source-wide read.
    state
        .with_conn(|conn, _| {
            conn.execute(
                "INSERT INTO permission_rules
                     (source_id, table_id, can_read, can_write, can_delete, updated_at)
                 VALUES ('mock-source', NULL, 1, 0, 0, ?1)",
                params![db::now_iso()],
            )
            .expect("insert source rule");
            Ok(())
        })
        .expect("source rule");
    let spreadsheet_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
    let output = parse(
        &list_sheets(
            &state,
            &SourceTableArgs {
                source_id: source(),
                table_id: format!(
                    "https://docs.google.com/spreadsheets/d/{spreadsheet_id}/edit#gid=0"
                ),
            },
        )
        .expect("list_sheets"),
    );
    assert_eq!(output["spreadsheetId"], spreadsheet_id);
    assert_eq!(output["locale"], "en_US");
    assert_eq!(output["timeZone"], "Etc/GMT");
    assert_eq!(output["sheets"][0]["gid"], "0");
    assert_eq!(output["sheets"][0]["title"], "Sheet1");
    assert!(output["sheets"][0].get("index").is_none());
}

#[test]
fn read_formulas_is_unsupported_on_the_mock_connector() {
    let state = temp_state();
    let error = read_formulas(&state, &read_args()).expect_err("no formula reads on the mock");
    assert!(
        error
            .to_string()
            .contains("does not support reading formulas"),
        "unexpected error: {error}"
    );
}

fn cells_args(range: Option<&str>, limit: Option<i64>, offset: Option<i64>) -> ReadCellsArgs {
    ReadCellsArgs {
        source_id: source(),
        table_id: TABLE.to_string(),
        range: range.map(str::to_string),
        limit,
        offset,
    }
}

#[test]
fn read_cells_exposes_the_raw_grid_with_row_numbers() {
    let state = temp_state();
    let output = parse(&read_cells(&state, &cells_args(None, Some(2), None)).expect("read cells"));

    assert_eq!(output["columns"][0], "A", "columns are A1 letters");
    assert_eq!(output["rows"][0]["row"], 1, "row numbers are 1-based");
    assert_eq!(
        output["rows"][0]["cells"]["A"], "Name",
        "row 1 is the raw header row, not interpreted away"
    );
    assert_eq!(output["rows"][1]["row"], 2);
    assert_eq!(output["rows"][1]["cells"]["A"], "Aurora Labs");
    assert_eq!(output["totalRows"], 4, "header + 3 records");
}

#[test]
fn read_cells_with_a_range_returns_only_that_window() {
    let state = temp_state();
    let output =
        parse(&read_cells(&state, &cells_args(Some("B3:C4"), None, None)).expect("range read"));
    assert_eq!(output["columns"], serde_json::json!(["B", "C"]));
    assert_eq!(output["totalRows"], 2);
    assert_eq!(
        output["rows"][0]["row"], 3,
        "rows keep their sheet row number"
    );
    assert_eq!(output["rows"][0]["cells"]["B"], "it@basalt.co");
    assert_eq!(output["rows"][1]["cells"]["C"], "enterprise");
    assert!(output["rows"][0]["cells"].get("A").is_none());

    let paged =
        parse(&read_cells(&state, &cells_args(Some("A:A"), Some(1), Some(2))).expect("paged"));
    assert_eq!(paged["rows"].as_array().expect("rows").len(), 1);
    assert_eq!(paged["rows"][0]["row"], 3);
    assert_eq!(paged["rows"][0]["cells"]["A"], "Basalt Co");
    assert_eq!(paged["totalRows"], 4);

    let error = read_cells(&state, &cells_args(Some("Sheet1!A1:B2"), None, None))
        .expect_err("a sheet-qualified range is refused");
    assert!(error.to_string().contains("tableId"), "{error}");
}

#[test]
fn update_cells_writes_the_cell_immediately() {
    let state = temp_state();
    let output = parse(
        &update_cells(
            &state,
            UpdateCellsArgs {
                source_id: source(),
                table_id: TABLE.to_string(),
                cells: vec![crate::args::CellWriteArg {
                    cell: "B2".to_string(),
                    value: "edited@cell.dev".to_string(),
                }],
                dry_run: false,
            },
        )
        .expect("update cells"),
    );
    assert_eq!(output["committed"], true);
    assert_eq!(output["change"]["type"], "update_cells");
    assert_eq!(output["change"]["status"], "committed");
    assert!(
        output.get("records").is_none(),
        "empty records are omitted from the lean output"
    );
    assert!(output.get("outcome").is_none());
    assert_eq!(
        output["change"]["diff"]["cells"][0]["cell"], "B2",
        "the diff lists each targeted cell"
    );
    assert_eq!(
        read_records(&state)[0]["fields"]["Email"],
        "edited@cell.dev",
        "the cell write lands on the record"
    );
}

#[test]
fn append_records_bundles_a_format_plan_into_the_change() {
    let state = temp_state();
    let output = parse(
        &append_records(
            &state,
            AppendRecordsArgs {
                format: crate::args::FormatSpec {
                    freeze_rows: Some(1),
                    ..Default::default()
                },
                ..append_args(true)
            },
        )
        .expect("append dry run"),
    );
    assert_eq!(
        output["change"]["diff"]["format"]["freezeRows"], 1,
        "the bundled plan is part of the staged change"
    );
}

#[test]
fn commit_change_commits_a_batch_of_changes_in_one_call() {
    let state = temp_state();
    let stage = || {
        parse(&append_records(&state, append_args(true)).expect("stage"))["change"]["id"]
            .as_str()
            .expect("change id")
            .to_string()
    };
    let ids = vec![stage(), stage()];

    let output = parse(
        &commit_change(
            &state,
            &CommitChangeArgs {
                change_id: None,
                change_ids: Some(ids),
            },
        )
        .expect("batch commit"),
    );

    let committed = output["committed"].as_array().expect("committed array");
    assert_eq!(committed.len(), 2);
    assert!(
        committed
            .iter()
            .all(|outcome| outcome["change"]["status"] == "committed"
                && outcome["committed"] == true
                && outcome["records"].is_array()
                && outcome.get("outcome").is_none()),
        "every change in the batch is committed, in the lean shape"
    );
}

/// A representative formatting plan: a bold, filled, underlined header plus a
/// frozen header row.
fn format_args(dry_run: bool) -> FormatTableArgs {
    FormatTableArgs {
        source_id: source(),
        table_id: TABLE.to_string(),
        format: crate::args::FormatSpec {
            formats: vec![crate::args::CellFormatArg {
                range: "A1:E1".to_string(),
                bold: Some(true),
                italic: None,
                font_size: None,
                font_color: None,
                background_color: Some("#f3f4f6".to_string()),
                horizontal_alignment: None,
                number_format: None,
                number_format_type: None,
                wrap: None,
                border: Some("bottom".to_string()),
            }],
            freeze_rows: Some(1),
            ..Default::default()
        },
        dry_run,
    }
}

#[test]
fn format_table_dry_run_stages_a_format_change_without_payload() {
    let state = temp_state();
    let output = parse(&format_table(&state, format_args(true)).expect("format dry run"));
    assert_eq!(output["committed"], false);
    assert_eq!(output["change"]["type"], "format");
    assert_eq!(output["change"]["status"], "pending");
    let change = output["change"].as_object().expect("change object");
    assert!(
        !change.contains_key("payload"),
        "payload must never reach agents"
    );
    // The agent-visible diff is the plan itself.
    assert_eq!(output["change"]["diff"]["freezeRows"], 1);
    assert_eq!(output["change"]["diff"]["formats"][0]["range"], "A1:E1");
    assert_eq!(output["change"]["diff"]["formats"][0]["bold"], true);
    assert_eq!(output["change"]["diff"]["formats"][0]["border"], "bottom");
}

#[test]
fn format_table_surfaces_an_unsupported_connector_on_apply() {
    let state = temp_state();
    let error = format_table(&state, format_args(false)).expect_err("mock cannot format");
    assert!(
        error
            .to_string()
            .contains("does not support cell formatting"),
        "unexpected error: {error}"
    );
    // The failed apply leaves the change pending (staged), as the error says.
    assert!(error.to_string().contains("is still staged"), "{error}");
    let staged = state
        .with_conn(|conn, _| changes::list_changes(conn, Some("pending")))
        .expect("list");
    assert_eq!(staged.len(), 1, "the change stays pending");
    assert_eq!(staged[0].decided_by, None);
}

#[test]
fn format_table_requires_write_permission() {
    let state = temp_state();
    exec(
        &state,
        "UPDATE permission_rules SET can_write=0 WHERE source_id='mock-source'",
    );
    let error = format_table(&state, format_args(true)).expect_err("write must be denied");
    assert!(
        error
            .to_string()
            .to_lowercase()
            .contains("write access denied"),
        "unexpected error: {error}"
    );
}

#[test]
fn delete_sheet_needs_confirm_and_the_delete_permission() {
    let state = temp_state();
    let args = |confirm| DeleteSheetArgs {
        source_id: source(),
        table_id: TABLE.to_string(),
        confirm,
        dry_run: true,
    };
    let unconfirmed = delete_sheet(&state, args(false)).expect_err("confirm required");
    assert_eq!(unconfirmed.to_string(), "delete_sheet needs confirm: true");

    let denied = delete_sheet(&state, args(true)).expect_err("no delete permission");
    assert!(
        denied.to_string().contains("Delete access denied"),
        "unexpected error: {denied}"
    );

    exec(
        &state,
        "UPDATE permission_rules SET can_delete=1 WHERE source_id='mock-source'",
    );
    let staged = parse(&delete_sheet(&state, args(true)).expect("dry-run delete"));
    assert_eq!(staged["committed"], false);
    assert_eq!(staged["change"]["type"], "delete_sheet");
}

#[test]
fn get_table_style_is_unsupported_on_the_mock_connector() {
    let state = temp_state();
    let error = get_table_style(
        &state,
        &GetTableStyleArgs {
            source_id: source(),
            table_id: TABLE.to_string(),
            header_row: Some(9),
        },
    )
    .expect_err("mock connector has no style read");
    assert!(
        error
            .to_string()
            .contains("does not support reading cell formatting"),
        "unexpected error: {error}"
    );
}

#[test]
fn format_table_stages_validations_and_conditional_formats_in_the_diff() {
    let mut args = format_args(true);
    args.format.validations = vec![crate::args::ValidationArg {
        range: "D2:D21".to_string(),
        kind: "list".to_string(),
        values: Some(vec!["Todo".to_string(), "Done".to_string()]),
        strict: None,
        show_dropdown: None,
    }];
    args.format.conditional_formats = vec![crate::args::ConditionalFormatArg {
        range: "D2:D21".to_string(),
        when: crate::args::ConditionWhenArg {
            text_eq: Some("Done".to_string()),
            ..Default::default()
        },
        background_color: Some("#d1fae5".to_string()),
        font_color: None,
        bold: None,
    }];
    let state = temp_state();
    let output = parse(&format_table(&state, args).expect("format dry run"));
    let diff = &output["change"]["diff"];
    assert_eq!(
        diff["validations"][0],
        json!({
            "range": "D2:D21",
            "type": "list",
            "values": ["Todo", "Done"],
            "strict": true,
            "showDropdown": true,
        })
    );
    assert_eq!(
        diff["conditionalFormats"][0],
        json!({
            "range": "D2:D21",
            "when": { "textEq": "Done" },
            "backgroundColor": "#d1fae5",
        })
    );
}
