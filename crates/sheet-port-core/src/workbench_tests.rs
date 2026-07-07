//! Workbench folder/item CRUD, add-spreadsheet gating + dedupe, and grid
//! read/write against the MOCK connector (no network). GoogleSheetsConnector
//! grid behavior is covered by pure-function tests in google_sheets.rs.

use rusqlite::params;

use super::*;
use crate::connectors::{column_id_for_index, column_index_for_id, ConnectorRegistry};
use crate::db::test_support::open_temp_db;
use crate::sources::{self, SOURCE_STATUS_CONNECTED};
use crate::test_fixtures::{demo_db, DEMO_SOURCE_ID, DEMO_TABLE_ID};
use crate::types::SourceKind;

/// A realistic 44-char Google document id (matches the connector's parser
/// tests) so `parse_spreadsheet_id` accepts it without a network call.
const SAMPLE_ID: &str = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";

fn registry() -> ConnectorRegistry {
    ConnectorRegistry::with_default_connectors()
}

/// Inserts a connected, keyed Google account source so `add_spreadsheet` finds
/// a source without running OAuth.
fn connect_google(conn: &rusqlite::Connection) {
    sources::upsert(
        conn,
        "google-sheets:testkey",
        SourceKind::GoogleSheets,
        "Google Sheets (test@example.com)",
        SOURCE_STATUS_CONNECTED,
    )
    .expect("insert google source");
}

/// Directly inserts a Workbench item (bypassing the network name resolution in
/// `add_spreadsheet`) so remove/move/dedupe paths can be exercised offline.
fn insert_item(
    conn: &rusqlite::Connection,
    id: &str,
    folder_id: Option<&str>,
    spreadsheet_id: &str,
    position: i64,
) {
    conn.execute(
        "INSERT INTO workbench_items
           (id, folder_id, source_id, spreadsheet_id, name, position)
         VALUES (?1, ?2, 'google-sheets:testkey', ?3, ?4, ?5)",
        params![
            id,
            folder_id,
            spreadsheet_id,
            format!("Sheet {id}"),
            position
        ],
    )
    .expect("insert item");
}

#[test]
fn column_ids_round_trip_with_indexes() {
    for index in [0usize, 1, 25, 26, 27, 51, 52, 701, 702] {
        let id = column_id_for_index(index);
        assert_eq!(column_index_for_id(&id), Some(index), "round-trip {index}");
    }
    assert_eq!(column_id_for_index(0), "A");
    assert_eq!(column_id_for_index(25), "Z");
    assert_eq!(column_id_for_index(26), "AA");
    assert_eq!(column_index_for_id(""), None);
    assert_eq!(column_index_for_id("a"), None, "lowercase is rejected");
    assert_eq!(column_index_for_id("A1"), None, "digits are rejected");
}

#[test]
fn folder_crud_assigns_positions_and_orders_the_tree() {
    let conn = open_temp_db();

    let first = create_folder(&conn, "  Sales  ").expect("create first");
    let second = create_folder(&conn, "Ops").expect("create second");
    assert_eq!(first.name, "Sales", "name is trimmed");
    assert_eq!(first.position, 1);
    assert_eq!(second.position, 2);
    assert!(first.id.starts_with("wbf_"));

    let listed = tree(&conn).expect("tree");
    let names: Vec<&str> = listed.folders.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Sales", "Ops"], "ordered by position");

    rename_folder(&conn, &first.id, "Revenue").expect("rename");
    let renamed = tree(&conn).expect("tree").folders;
    assert_eq!(renamed[0].name, "Revenue");
}

#[test]
fn folder_name_must_not_be_empty() {
    let conn = open_temp_db();
    assert!(matches!(
        create_folder(&conn, "   "),
        Err(CoreError::InvalidInput(_))
    ));
    let folder = create_folder(&conn, "Ops").expect("create");
    assert!(matches!(
        rename_folder(&conn, &folder.id, ""),
        Err(CoreError::InvalidInput(_))
    ));
}

#[test]
fn rename_and_delete_unknown_folder_error() {
    let conn = open_temp_db();
    assert!(matches!(
        rename_folder(&conn, "wbf_nope", "X"),
        Err(CoreError::NotFound(_))
    ));
    assert!(matches!(
        delete_folder(&conn, "wbf_nope"),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn deleting_a_folder_moves_its_items_to_ungrouped() {
    let conn = open_temp_db();
    let folder = create_folder(&conn, "Ops").expect("create");
    insert_item(&conn, "wbi_1", Some(&folder.id), SAMPLE_ID, 1);

    delete_folder(&conn, &folder.id).expect("delete");

    let item = get_item(&conn, "wbi_1").expect("item survives");
    assert_eq!(item.folder_id, None, "item falls back to Ungrouped");
    assert!(tree(&conn).expect("tree").folders.is_empty());
}

#[test]
fn add_spreadsheet_requires_a_connected_google_source() {
    let conn = open_temp_db();
    let error = add_spreadsheet(&conn, &registry(), None, SAMPLE_ID).expect_err("no source");
    assert!(matches!(error, CoreError::PermissionDenied(_)));
}

#[test]
fn add_spreadsheet_dedupes_an_existing_spreadsheet_in_the_folder() {
    let conn = open_temp_db();
    connect_google(&conn);
    insert_item(&conn, "wbi_existing", None, SAMPLE_ID, 1);

    // Same spreadsheet, passed as a full URL, resolves to the existing item
    // without a network call or a duplicate row.
    let url = format!("https://docs.google.com/spreadsheets/d/{SAMPLE_ID}/edit#gid=0");
    let item = add_spreadsheet(&conn, &registry(), None, &url).expect("dedupe");
    assert_eq!(item.id, "wbi_existing");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM workbench_items", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 1, "no duplicate inserted");
}

#[test]
fn remove_item_deletes_and_errors_on_unknown_id() {
    let conn = open_temp_db();
    insert_item(&conn, "wbi_1", None, SAMPLE_ID, 1);

    remove_item(&conn, "wbi_1").expect("remove");
    assert!(matches!(
        get_item(&conn, "wbi_1"),
        Err(CoreError::NotFound(_))
    ));
    assert!(matches!(
        remove_item(&conn, "wbi_1"),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn move_item_changes_folder_and_rejects_unknown_targets() {
    let conn = open_temp_db();
    let folder = create_folder(&conn, "Ops").expect("folder");
    insert_item(&conn, "wbi_1", None, SAMPLE_ID, 1);

    move_item(&conn, "wbi_1", Some(&folder.id)).expect("move into folder");
    assert_eq!(
        get_item(&conn, "wbi_1").expect("item").folder_id.as_deref(),
        Some(folder.id.as_str())
    );

    move_item(&conn, "wbi_1", None).expect("move to ungrouped");
    assert_eq!(get_item(&conn, "wbi_1").expect("item").folder_id, None);

    assert!(matches!(
        move_item(&conn, "wbi_1", Some("wbf_missing")),
        Err(CoreError::NotFound(_))
    ));
    assert!(matches!(
        move_item(&conn, "wbi_missing", None),
        Err(CoreError::NotFound(_))
    ));
}

// ---------------------------------------------------------------------------
// Grid read/write through the registry, backed by the MOCK connector.
// ---------------------------------------------------------------------------

#[test]
fn mock_list_sheet_tabs_returns_one_synthetic_tab() {
    let conn = demo_db();
    let tabs = registry()
        .list_sheet_tabs(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID)
        .expect("tabs");
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].gid, "0");
    assert_eq!(tabs[0].title, "Sheet1");
    assert_eq!(tabs[0].index, 0);
}

#[test]
fn mock_read_grid_maps_fields_to_lettered_columns() {
    let conn = demo_db();
    let grid = registry()
        .read_grid(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, None, None)
        .expect("grid");

    let columns: Vec<(&str, &str)> = grid
        .columns
        .iter()
        .map(|c| (c.id.as_str(), c.title.as_str()))
        .collect();
    assert_eq!(
        columns,
        [
            ("A", "Name"),
            ("B", "Email"),
            ("C", "Plan"),
            ("D", "Seats"),
            ("E", "Active"),
        ]
    );
    assert_eq!(grid.total_rows, 3);
    assert_eq!(grid.rows.len(), 3);
    assert_eq!(
        grid.rows[0].get("A").map(String::as_str),
        Some("Aurora Labs")
    );
    assert_eq!(
        grid.rows[0].get("D").map(String::as_str),
        Some("24"),
        "numbers stringify"
    );
    assert_eq!(
        grid.rows[0].get("E").map(String::as_str),
        Some("true"),
        "booleans stringify"
    );
}

#[test]
fn mock_write_cell_updates_the_targeted_cell() {
    let conn = demo_db();
    let reg = registry();

    reg.write_cell(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, 0, "D", "25")
        .expect("write");
    let grid = reg
        .read_grid(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, None, None)
        .expect("grid");
    assert_eq!(grid.rows[0].get("D").map(String::as_str), Some("25"));

    // A column id past the header, or a negative row, is rejected.
    assert!(matches!(
        reg.write_cell(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, 0, "Z", "x"),
        Err(CoreError::InvalidInput(_))
    ));
    assert!(matches!(
        reg.write_cell(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, -1, "A", "x"),
        Err(CoreError::InvalidInput(_))
    ));
    assert!(matches!(
        reg.write_cell(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, 99, "A", "x"),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn mock_append_grid_row_returns_the_new_row_index() {
    let conn = demo_db();
    let reg = registry();

    let mut values = crate::types::GridRow::new();
    values.insert("A".to_string(), "Zeta Inc".to_string());
    let index = reg
        .append_grid_row(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, &values)
        .expect("append");
    assert_eq!(index, 3, "new 0-based data row index");

    let grid = reg
        .read_grid(&conn, DEMO_SOURCE_ID, DEMO_TABLE_ID, None, None)
        .expect("grid");
    assert_eq!(grid.total_rows, 4);
    assert_eq!(grid.rows[3].get("A").map(String::as_str), Some("Zeta Inc"));
    assert_eq!(
        grid.rows[3].get("B").map(String::as_str),
        Some(""),
        "unset columns are empty cells"
    );
}
