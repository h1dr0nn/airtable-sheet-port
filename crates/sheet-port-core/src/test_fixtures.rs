//! Shared test fixtures. The demo workspace used to ship in seed.sql; since
//! schema v2 fresh databases start empty, so tests that need data install
//! this fixture themselves (MockConnector stays in the crate exactly for
//! tests and the e2e smoke).

use rusqlite::{params, Connection};

use crate::db::now_iso;
use crate::db::test_support::open_temp_db;
use crate::sources;
use crate::types::SourceKind;

pub(crate) const DEMO_SOURCE_ID: &str = "mock-source";
pub(crate) const DEMO_TABLE_ID: &str = "customers";

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

/// Opens an isolated temp DB with the demo workspace installed.
pub(crate) fn demo_db() -> Connection {
    let conn = open_temp_db();
    install_demo_workspace(&conn);
    conn
}

/// Installs the demo workspace the v1 seed used to ship: the mock source,
/// the Customers table with rec_seed_1..3, and a permissive rule requiring
/// confirmation for every write action.
pub(crate) fn install_demo_workspace(conn: &Connection) {
    sources::upsert(
        conn,
        DEMO_SOURCE_ID,
        SourceKind::Mock,
        "Demo Workspace",
        sources::SOURCE_STATUS_CONNECTED,
    )
    .expect("insert demo source");

    conn.execute(
        "INSERT INTO mock_tables (source_id, table_id, name, fields)
         VALUES (?1, ?2, 'Customers', ?3)",
        params![DEMO_SOURCE_ID, DEMO_TABLE_ID, DEMO_TABLE_FIELDS],
    )
    .expect("insert demo table");

    for (position, (record_id, fields)) in DEMO_RECORDS.iter().enumerate() {
        conn.execute(
            "INSERT INTO mock_records (source_id, table_id, record_id, fields, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                DEMO_SOURCE_ID,
                DEMO_TABLE_ID,
                record_id,
                fields,
                position as i64 + 1
            ],
        )
        .expect("insert demo record");
    }

    conn.execute(
        "INSERT INTO permission_rules
             (source_id, table_id, can_read, can_write, can_delete,
              require_confirmation, updated_at)
         VALUES (?1, ?2, 1, 1, 0, '[\"append\",\"update\",\"delete\",\"bulk_update\"]', ?3)",
        params![DEMO_SOURCE_ID, DEMO_TABLE_ID, now_iso()],
    )
    .expect("insert demo rule");
}
