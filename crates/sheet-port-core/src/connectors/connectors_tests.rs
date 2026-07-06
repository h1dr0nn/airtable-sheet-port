//! Ports the ConnectorRegistry and MockConnector vitest suites, plus the
//! stub-connector TODO messages that agents see verbatim.

use serde_json::json;

use super::*;
use crate::db::test_support::open_temp_db;
use crate::mock_data;

const SOURCE: &str = "mock-source";
const TABLE: &str = "customers";

fn registry() -> ConnectorRegistry {
    ConnectorRegistry::with_default_connectors()
}

fn fields(pairs: &[(&str, serde_json::Value)]) -> JsonMap {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

#[test]
fn registry_routes_reads_through_the_mock_connector() {
    let conn = open_temp_db();
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
    let conn = open_temp_db();
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
    let conn = open_temp_db();
    // google-placeholder is seeded but only the mock connector is registered.
    let error = registry()
        .list_tables(&conn, "google-placeholder")
        .expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "No connector registered for source kind google_sheets (source google-placeholder)"
    );
}

#[test]
fn registry_aggregates_sources_across_connectors() {
    let conn = open_temp_db();
    let mut registry = registry();
    registry.register(Box::new(GoogleSheetsConnector::new(vec![
        "sheet-a".to_string()
    ])));

    let sources = registry.list_sources(&conn).expect("sources");
    let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
    assert_eq!(ids, ["mock-source", "google_sheets:sheet-a"]);
}

#[test]
fn mock_list_sources_filters_to_mock_kind() {
    let conn = open_temp_db();
    let sources = MockConnector.list_sources(&conn).expect("sources");
    assert_eq!(sources.len(), 1, "placeholder sources must be excluded");
    assert_eq!(sources[0].id, SOURCE);
    assert_eq!(sources[0].kind, SourceKind::Mock);
}

#[test]
fn mock_rejects_unknown_source_and_table() {
    let conn = open_temp_db();
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
    let wrong_kind = MockConnector
        .list_tables(&conn, "google-placeholder")
        .expect_err("wrong kind");
    assert_eq!(
        wrong_kind.to_string(),
        "Unknown mock source google-placeholder"
    );
}

#[test]
fn mock_find_records_matches_case_insensitive_substrings() {
    let conn = open_temp_db();
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
    let conn = open_temp_db();
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
fn google_sheets_stub_lists_configured_sources_and_errors_elsewhere() {
    let conn = open_temp_db();
    let connector = GoogleSheetsConnector::new(vec!["abc".to_string()]);

    let sources = connector.list_sources(&conn).expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "google_sheets:abc");
    assert_eq!(sources[0].name, "Google Sheet abc");

    let error = connector
        .list_tables(&conn, "google_sheets:abc")
        .expect_err("stub");
    assert_eq!(
        error.to_string(),
        "Google Sheets connector TODO: discover sheet tabs/ranges after OAuth is implemented"
    );
    let error = connector
        .read_table(&conn, "google_sheets:abc", "tab", ReadOptions::default())
        .expect_err("stub");
    assert_eq!(
        error.to_string(),
        "Google Sheets connector TODO: read bounded values through googleapis"
    );
}

#[test]
fn provider_stub_lists_configured_sources_and_errors_elsewhere() {
    let conn = open_temp_db();
    let connector = ProviderConnector::new(vec!["crm".to_string()]);

    let sources = connector.list_sources(&conn).expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "provider:crm");
    assert_eq!(sources[0].name, "Provider Source crm");

    let error = connector
        .append_records(&conn, "provider:crm", "t", &[])
        .expect_err("stub");
    assert_eq!(
        error.to_string(),
        "Provider connector TODO: create records after preview and policy approval"
    );
}

#[test]
fn registering_the_same_kind_replaces_the_connector() {
    let conn = open_temp_db();
    let mut registry = ConnectorRegistry::new();
    registry.register(Box::new(GoogleSheetsConnector::new(vec!["a".to_string()])));
    registry.register(Box::new(GoogleSheetsConnector::new(vec![
        "b".to_string(),
        "c".to_string(),
    ])));

    let sources = registry.list_sources(&conn).expect("sources");
    let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
    assert_eq!(ids, ["google_sheets:b", "google_sheets:c"]);
}
