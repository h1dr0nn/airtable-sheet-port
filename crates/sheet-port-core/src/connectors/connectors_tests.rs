//! ConnectorRegistry and MockConnector behavior against the demo-workspace
//! fixture (fresh databases are empty since schema v2). GoogleSheetsConnector
//! network behavior is covered by pure-function tests in google_sheets.rs.

use serde_json::json;

use super::*;
use crate::db::test_support::open_temp_db;
use crate::mock_data;
use crate::test_fixtures::{demo_db, DEMO_SOURCE_ID, DEMO_TABLE_ID};

const SOURCE: &str = DEMO_SOURCE_ID;
const TABLE: &str = DEMO_TABLE_ID;

fn registry() -> ConnectorRegistry {
    ConnectorRegistry::with_default_connectors()
}

fn fields(pairs: &[(&str, serde_json::Value)]) -> JsonMap {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

/// Minimal stand-in connector used to observe registry routing semantics.
struct FakeConnector {
    kind: SourceKind,
    ids: Vec<&'static str>,
}

impl TableConnector for FakeConnector {
    fn kind(&self) -> SourceKind {
        self.kind
    }

    fn list_sources(&self, _conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        Ok(self
            .ids
            .iter()
            .map(|id| DataSource {
                id: (*id).to_string(),
                kind: self.kind,
                name: (*id).to_string(),
                status: None,
            })
            .collect())
    }

    fn list_tables(
        &self,
        _conn: &Connection,
        _source_id: &str,
    ) -> Result<Vec<TableRef>, CoreError> {
        Err(CoreError::Unsupported("fake connector".to_string()))
    }

    fn describe_table(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        Err(CoreError::Unsupported("fake connector".to_string()))
    }

    fn read_table(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported("fake connector".to_string()))
    }

    fn find_records(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _query: &str,
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported("fake connector".to_string()))
    }

    fn append_records(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported("fake connector".to_string()))
    }

    fn update_records(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported("fake connector".to_string()))
    }
}

#[test]
fn registry_routes_reads_through_the_mock_connector() {
    let conn = demo_db();
    let registry = registry();

    let tables = registry.list_tables(&conn, SOURCE).expect("tables");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].table_id, TABLE);

    let records = registry
        .read_table(&conn, SOURCE, TABLE, ReadOptions::default())
        .expect("records");
    assert_eq!(records.len(), 3);

    let schema = registry
        .describe_table(&conn, SOURCE, TABLE)
        .expect("schema");
    assert_eq!(schema.name, "Customers");
}

#[test]
fn registry_routes_writes_through_the_mock_connector() {
    let conn = demo_db();
    let registry = registry();

    let appended = registry
        .append_records(&conn, SOURCE, TABLE, &[fields(&[("Name", json!("New"))])])
        .expect("append");
    assert!(appended[0].id.starts_with("rec_"));

    let updated = registry
        .update_records(
            &conn,
            SOURCE,
            TABLE,
            &[RecordPatch {
                record_id: "rec_seed_1".to_string(),
                fields: fields(&[("Name", json!("Renamed"))]),
            }],
        )
        .expect("update");
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].fields.get("Name"), Some(&json!("Renamed")));

    let found = registry
        .find_records(&conn, SOURCE, TABLE, "renamed")
        .expect("find");
    let ids: Vec<&str> = found.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["rec_seed_1"]);
}

#[test]
fn registry_errors_for_unknown_source() {
    let conn = open_temp_db();
    let error = registry()
        .read_table(&conn, "src-nope", TABLE, ReadOptions::default())
        .expect_err("must fail");
    assert_eq!(error.to_string(), "Unknown source src-nope");
}

#[test]
fn registry_errors_when_kind_has_no_connector() {
    // A mock source exists in the DB but the registry only serves Google
    // Sheets, as in a build without the `mock` feature.
    let conn = demo_db();
    let mut registry = ConnectorRegistry::new();
    registry.register(Box::new(GoogleSheetsConnector::new()));

    let error = registry.list_tables(&conn, SOURCE).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        format!("No connector registered for source kind mock (source {SOURCE})")
    );
}

#[test]
fn registry_aggregates_sources_across_connectors() {
    let conn = demo_db();
    let mut registry = registry();
    // Replaces the default Google Sheets connector with a canned one.
    registry.register(Box::new(FakeConnector {
        kind: SourceKind::GoogleSheets,
        ids: vec!["google:crm"],
    }));

    let sources = registry.list_sources(&conn).expect("sources");
    let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
    assert_eq!(ids, [SOURCE, "google:crm"]);
}

#[test]
fn mock_list_sources_filters_to_mock_kind() {
    let conn = demo_db();
    crate::sources::upsert(
        &conn,
        "google-sheets",
        SourceKind::GoogleSheets,
        "Google Sheets (user@example.com)",
        crate::sources::SOURCE_STATUS_CONNECTED,
    )
    .expect("insert google source");

    let sources = MockConnector.list_sources(&conn).expect("sources");
    assert_eq!(sources.len(), 1, "non-mock sources must be excluded");
    assert_eq!(sources[0].id, SOURCE);
    assert_eq!(sources[0].kind, SourceKind::Mock);
}

#[test]
fn mock_rejects_unknown_source_and_table() {
    let conn = demo_db();
    let source_error = MockConnector
        .list_tables(&conn, "nope")
        .expect_err("unknown source");
    assert_eq!(source_error.to_string(), "Unknown mock source nope");

    let table_error = MockConnector
        .describe_table(&conn, SOURCE, "nope")
        .expect_err("unknown table");
    assert_eq!(
        table_error.to_string(),
        format!("Unknown mock table {SOURCE}/nope")
    );

    // A non-mock source id is also rejected even though it exists.
    crate::sources::upsert(
        &conn,
        "google-sheets",
        SourceKind::GoogleSheets,
        "Google Sheets (user@example.com)",
        crate::sources::SOURCE_STATUS_CONNECTED,
    )
    .expect("insert google source");
    let wrong_kind = MockConnector
        .list_tables(&conn, "google-sheets")
        .expect_err("wrong kind");
    assert_eq!(wrong_kind.to_string(), "Unknown mock source google-sheets");
}

#[test]
fn mock_connector_does_not_support_formatting() {
    let conn = demo_db();
    let style = MockConnector.read_table_style(&conn, SOURCE, TABLE);
    assert!(matches!(style, Err(CoreError::Unsupported(_))));
    let format = MockConnector.format_cells(
        &conn,
        SOURCE,
        TABLE,
        &crate::types::FormatPlan {
            freeze_rows: Some(1),
            ..Default::default()
        },
    );
    assert!(matches!(format, Err(CoreError::Unsupported(_))));
}

#[test]
fn parse_a1_range_handles_cells_columns_and_rows() {
    let cell = parse_a1_range("A1").expect("cell");
    assert_eq!(cell.start_col, Some(0));
    assert_eq!(cell.end_col, Some(1));
    assert_eq!(cell.start_row, Some(0));
    assert_eq!(cell.end_row, Some(1));

    let block = parse_a1_range("A1:D10").expect("block");
    assert_eq!(block.start_col, Some(0));
    assert_eq!(block.end_col, Some(4));
    assert_eq!(block.start_row, Some(0));
    assert_eq!(block.end_row, Some(10));

    // Reversed corners normalize to the same block.
    assert_eq!(parse_a1_range("D10:A1").expect("reversed"), block);

    let columns = parse_a1_range("B:C").expect("columns");
    assert_eq!(columns.start_col, Some(1));
    assert_eq!(columns.end_col, Some(3));
    assert_eq!(columns.start_row, None);
    assert_eq!(columns.end_row, None);

    let rows = parse_a1_range("2:4").expect("rows");
    assert_eq!(rows.start_row, Some(1));
    assert_eq!(rows.end_row, Some(4));
    assert_eq!(rows.start_col, None);

    // Lowercase and absolute markers are tolerated.
    assert_eq!(parse_a1_range("$a$1").expect("abs"), cell);
}

#[test]
fn parse_a1_range_rejects_malformed_and_mixed_shapes() {
    assert!(parse_a1_range("").is_err());
    assert!(parse_a1_range("A1:B").is_err(), "cell-to-column is mixed");
    assert!(parse_a1_range("1A").is_err(), "digits before letters");
    assert!(parse_a1_range("A0").is_err(), "row 0 is invalid");
    assert!(parse_a1_range("!!").is_err());
    assert!(
        validate_a1_range("A1:D1").is_ok(),
        "the public validator accepts a good range"
    );
    assert!(validate_a1_range("nope").is_err());
}

#[test]
fn parse_cells_range_accepts_blocks_columns_and_rows() {
    let block = parse_cells_range("B40:F60").expect("block");
    assert_eq!(block.start_col, Some(1));
    assert_eq!(block.end_col, Some(6));
    assert_eq!(block.start_row, Some(39));
    assert_eq!(block.end_row, Some(60));

    let columns = parse_cells_range("A:C").expect("columns");
    assert_eq!((columns.start_col, columns.end_col), (Some(0), Some(3)));
    assert_eq!((columns.start_row, columns.end_row), (None, None));

    let rows = parse_cells_range("5:9").expect("rows");
    assert_eq!((rows.start_row, rows.end_row), (Some(4), Some(9)));
    assert_eq!((rows.start_col, rows.end_col), (None, None));
}

#[test]
fn parse_cells_range_rejects_sheet_names_and_malformed_ranges() {
    let qualified = parse_cells_range("Sheet1!A1:B2");
    assert!(
        matches!(&qualified, Err(CoreError::InvalidInput(message)) if message.contains("tableId")),
        "a sheet-qualified range is refused: {qualified:?}"
    );
    assert!(parse_cells_range("'My Tab'!A:A").is_err());
    for bad in ["", "A1:B", "1A", "A0", "B40-F60", "A1:B2:C3", "ZZZ1"] {
        assert!(parse_cells_range(bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn window_a1_keeps_blocks_and_columns_and_bounds_row_windows() {
    let a1 = |range: &str| window_a1(&parse_cells_range(range).expect("range"));
    assert_eq!(a1("B40:F60"), "B40:F60");
    assert_eq!(a1("A:C"), "A:C");
    assert_eq!(a1("5:9"), "A5:ZZ9");
    assert_eq!(a1("c3"), "C3:C3");
}

#[test]
fn grid_window_numbers_rows_from_the_window_and_keys_by_letter() {
    let range = parse_cells_range("B40:D42").expect("range");
    let rows = vec![
        vec!["a".to_string(), "b".to_string()],
        Vec::new(),
        vec![
            "x".to_string(),
            "y".to_string(),
            "z".to_string(),
            "past".to_string(),
        ],
        vec!["beyond the window".to_string()],
    ];
    let window = grid_window(&rows, &range, None, None);
    assert_eq!(window.columns, vec!["B", "C", "D"]);
    assert_eq!(
        window.total_rows, 3,
        "rows past the window height are dropped"
    );
    let numbers: Vec<i64> = window.rows.iter().map(|row| row.row).collect();
    assert_eq!(numbers, vec![40, 41, 42]);
    assert_eq!(window.rows[0].cells["B"], "a");
    assert_eq!(window.rows[0].cells["D"], "");
    assert_eq!(window.rows[2].cells["D"], "z");
    assert!(!window.rows[2].cells.contains_key("E"));

    let paged = grid_window(&rows, &range, Some(1), Some(1));
    assert_eq!(paged.rows.len(), 1);
    assert_eq!(paged.rows[0].row, 41);
    assert_eq!(paged.total_rows, 3);
}

#[test]
fn mock_read_grid_range_slices_the_raw_mirror() {
    let conn = demo_db();
    let registry = registry();
    // Row 1 is the field-name header; B = Email, C = Plan.
    let range = parse_cells_range("B2:C3").expect("range");
    let window = registry
        .read_grid_range(&conn, SOURCE, TABLE, &range, None, None)
        .expect("window");
    assert_eq!(window.columns, vec!["B", "C"]);
    assert_eq!(window.rows.len(), 2);
    assert_eq!(window.rows[0].row, 2);
    assert_eq!(window.rows[1].row, 3);

    let header = registry
        .read_grid_range(
            &conn,
            SOURCE,
            TABLE,
            &parse_cells_range("1:1").expect("row"),
            None,
            None,
        )
        .expect("header");
    assert_eq!(header.columns, vec!["A", "B", "C", "D", "E"]);
    assert_eq!(header.rows[0].cells["A"], "Name");
    assert_eq!(header.rows[0].cells["E"], "Active");
}

#[test]
fn mock_find_records_matches_case_insensitive_substrings() {
    let conn = demo_db();
    let find = |query: &str| {
        MockConnector
            .find_records(&conn, SOURCE, TABLE, query)
            .expect("find")
            .into_iter()
            .map(|record| record.id)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        find("AURORA"),
        ["rec_seed_1"],
        "case-insensitive text match"
    );
    assert_eq!(
        find("180"),
        ["rec_seed_3"],
        "numbers match their string form"
    );
    assert_eq!(
        find("true"),
        ["rec_seed_1", "rec_seed_2"],
        "booleans match their string form"
    );
    assert!(find("no-such-value").is_empty());
}

#[test]
fn mock_find_records_caps_results_at_100() {
    let conn = demo_db();
    let batch: Vec<JsonMap> = (0..120)
        .map(|n| fields(&[("Name", json!(format!("needle-{n}")))]))
        .collect();
    mock_data::append_records(&conn, SOURCE, TABLE, &batch).expect("append");

    let found = MockConnector
        .find_records(&conn, SOURCE, TABLE, "needle")
        .expect("find");
    assert_eq!(found.len(), 100, "results must cap at 100");
}

#[test]
fn registering_the_same_kind_replaces_the_connector() {
    let conn = open_temp_db();
    let mut registry = ConnectorRegistry::new();
    registry.register(Box::new(FakeConnector {
        kind: SourceKind::GoogleSheets,
        ids: vec!["google:a"],
    }));
    registry.register(Box::new(FakeConnector {
        kind: SourceKind::GoogleSheets,
        ids: vec!["google:b", "google:c"],
    }));

    let sources = registry.list_sources(&conn).expect("sources");
    let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
    assert_eq!(ids, ["google:b", "google:c"]);
}
