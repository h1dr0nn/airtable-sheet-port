//! Ports the MockDataStore vitest suite plus the desktop read_table paging
//! tests. Fresh databases are empty since schema v2, so each test installs
//! the demo-workspace fixture (mock-source/customers with rec_seed_1..3).

use serde_json::json;

use super::*;
use crate::test_fixtures::{demo_db, DEMO_SOURCE_ID, DEMO_TABLE_ID};

const SOURCE: &str = DEMO_SOURCE_ID;
const TABLE: &str = DEMO_TABLE_ID;

fn fields(pairs: &[(&str, serde_json::Value)]) -> JsonMap {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

#[test]
fn paginates_records_by_position_with_unpaged_total() {
    let conn = demo_db();
    let page = list_records(
        &conn,
        SOURCE,
        TABLE,
        ReadOptions {
            limit: Some(2),
            offset: Some(1),
        },
    )
    .expect("page");

    let ids: Vec<&str> = page
        .records
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    assert_eq!(ids, ["rec_seed_2", "rec_seed_3"]);
    assert_eq!(page.total, 3, "total must ignore limit/offset");
}

#[test]
fn returns_all_records_when_no_options_given() {
    let conn = demo_db();
    let page = list_records(&conn, SOURCE, TABLE, ReadOptions::default()).expect("page");
    assert_eq!(page.records.len(), 3);
    assert_eq!(page.total, 3);
}

#[test]
fn appends_records_with_generated_ids_after_existing_ones() {
    let conn = demo_db();
    let appended = append_records(
        &conn,
        SOURCE,
        TABLE,
        &[
            fields(&[("Name", json!("Delta"))]),
            fields(&[("Name", json!("Echo"))]),
        ],
    )
    .expect("append");

    assert_eq!(appended.len(), 2);
    for record in &appended {
        assert!(record.id.starts_with("rec_"), "generated id: {}", record.id);
    }

    let page = list_records(&conn, SOURCE, TABLE, ReadOptions::default()).expect("page");
    assert_eq!(page.total, 5);
    // Position ordering: the new records come last, in append order.
    let tail_ids: Vec<&str> = page.records[3..]
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    let appended_ids: Vec<&str> = appended.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(tail_ids, appended_ids);
    assert_eq!(page.records[3].fields.get("Name"), Some(&json!("Delta")));
    assert_eq!(page.records[4].fields.get("Name"), Some(&json!("Echo")));
}

#[test]
fn shallow_merges_patch_fields_into_stored_record() {
    let conn = demo_db();
    let updated = update_records(
        &conn,
        SOURCE,
        TABLE,
        &[RecordPatch {
            record_id: "rec_seed_1".to_string(),
            fields: fields(&[("Seats", json!(99))]),
        }],
    )
    .expect("update");

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].fields.get("Name"), Some(&json!("Aurora Labs")));
    assert_eq!(updated[0].fields.get("Seats"), Some(&json!(99)));
    assert_eq!(updated[0].fields.get("Plan"), Some(&json!("pro")));

    let page = list_records(
        &conn,
        SOURCE,
        TABLE,
        ReadOptions {
            limit: Some(1),
            offset: None,
        },
    )
    .expect("page");
    assert_eq!(page.records[0].fields.get("Seats"), Some(&json!(99)));
}

#[test]
fn skips_unknown_record_ids_on_update() {
    let conn = demo_db();
    let updated = update_records(
        &conn,
        SOURCE,
        TABLE,
        &[RecordPatch {
            record_id: "rec_missing".to_string(),
            fields: fields(&[("Seats", json!(1))]),
        }],
    )
    .expect("update");
    assert!(updated.is_empty());
}

#[test]
fn unknown_table_gives_none_schema_and_empty_page() {
    let conn = demo_db();
    assert!(get_table(&conn, SOURCE, "no-such-table")
        .expect("get")
        .is_none());
    let page = list_records(&conn, SOURCE, "no-such-table", ReadOptions::default()).expect("page");
    assert!(page.records.is_empty());
    assert_eq!(page.total, 0);
}

#[test]
fn exposes_seeded_table_schema() {
    let conn = demo_db();
    let tables = list_tables(&conn, SOURCE).expect("tables");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].table_id, TABLE);
    assert_eq!(tables[0].name, "Customers");

    let schema = get_table(&conn, SOURCE, TABLE)
        .expect("get")
        .expect("schema");
    let names: Vec<&str> = schema
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(names, ["Name", "Email", "Plan", "Seats", "Active"]);
    assert_eq!(schema.fields[0].field_type, "string");
    assert_eq!(schema.fields[0].required, Some(true));
}

#[test]
fn list_tables_returns_empty_for_unknown_source() {
    let conn = demo_db();
    let tables = list_tables(&conn, "does-not-exist").expect("list");
    assert!(tables.is_empty());
}

#[test]
fn describe_table_errors_on_unknown_table() {
    let conn = demo_db();
    let error = describe_table(&conn, SOURCE, "nope").expect_err("must fail");
    assert_eq!(error.to_string(), format!("Unknown table {SOURCE}/nope"));
}

#[test]
fn read_table_page_paginates_and_reports_full_total() {
    let conn = demo_db();

    let first = read_table_page(&conn, SOURCE, TABLE, Some(2), Some(0)).expect("page 1");
    assert_eq!(first.records.len(), 2);
    assert_eq!(first.total, 3, "total must ignore limit/offset");
    assert_eq!(first.records[0].id, "rec_seed_1", "must order by position");

    let second = read_table_page(&conn, SOURCE, TABLE, Some(2), Some(2)).expect("page 2");
    assert_eq!(second.records.len(), 1);
    assert_eq!(second.records[0].id, "rec_seed_3");
    assert_eq!(second.total, 3);
}

#[test]
fn read_table_page_clamps_limit_and_offset() {
    let conn = demo_db();

    let clamped_low = read_table_page(&conn, SOURCE, TABLE, Some(0), Some(-5)).expect("read");
    assert_eq!(clamped_low.records.len(), 1, "limit must clamp up to 1");
    assert_eq!(
        clamped_low.records[0].id, "rec_seed_1",
        "offset must clamp to 0"
    );

    let defaulted = read_table_page(&conn, SOURCE, TABLE, None, None).expect("read");
    assert_eq!(
        defaulted.records.len(),
        3,
        "default limit covers all seed rows"
    );
}
