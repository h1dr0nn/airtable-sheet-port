//! Mock connector data (mock_tables / mock_records). Lives in the shared DB
//! so the desktop UI and the MCP server see the same records and committed
//! changes persist across restarts.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::constants::{READ_LIMIT_DEFAULT, READ_LIMIT_MAX, READ_LIMIT_MIN};
use crate::error::{db_error, parse_json, CoreError};
use crate::types::{
    FieldSchema, JsonMap, ReadOptions, RecordPatch, TablePage, TableRecord, TableRef, TableSchema,
};

/// SQLite: LIMIT -1 means "no limit".
const NO_LIMIT: i64 = -1;

pub fn list_tables(conn: &Connection, source_id: &str) -> Result<Vec<TableRef>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT source_id, table_id, name FROM mock_tables
             WHERE source_id = ?1 ORDER BY table_id",
        )
        .map_err(|error| db_error("Could not list tables", error))?;
    let rows = stmt
        .query_map([source_id], |row| {
            Ok(TableRef {
                source_id: row.get(0)?,
                table_id: row.get(1)?,
                name: row.get(2)?,
            })
        })
        .map_err(|error| db_error("Could not list tables", error))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not list tables", error))
}

pub fn get_table(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
) -> Result<Option<TableSchema>, CoreError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, fields FROM mock_tables WHERE source_id = ?1 AND table_id = ?2",
            params![source_id, table_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| db_error("Could not describe table", error))?;
    row.map(|(name, fields_raw)| {
        let fields: Vec<FieldSchema> = parse_json(&fields_raw, "Table field schema")?;
        Ok(TableSchema {
            source_id: source_id.to_string(),
            table_id: table_id.to_string(),
            name,
            fields,
        })
    })
    .transpose()
}

/// Desktop-facing describe with the docs/ipc.md error wording.
pub fn describe_table(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
) -> Result<TableSchema, CoreError> {
    get_table(conn, source_id, table_id)?
        .ok_or_else(|| CoreError::NotFound(format!("Unknown table {source_id}/{table_id}")))
}

/// Raw page read ordered by position. `None` limit returns every record.
pub fn list_records(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    options: ReadOptions,
) -> Result<TablePage, CoreError> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mock_records WHERE source_id = ?1 AND table_id = ?2",
            params![source_id, table_id],
            |row| row.get(0),
        )
        .map_err(|error| db_error("Could not count records", error))?;

    let mut stmt = conn
        .prepare(
            "SELECT record_id, fields FROM mock_records
             WHERE source_id = ?1 AND table_id = ?2
             ORDER BY position LIMIT ?3 OFFSET ?4",
        )
        .map_err(|error| db_error("Could not read records", error))?;
    let raw_rows = stmt
        .query_map(
            params![
                source_id,
                table_id,
                options.limit.unwrap_or(NO_LIMIT),
                options.offset.unwrap_or(0)
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|error| db_error("Could not read records", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| db_error("Could not read records", error))?;

    let records = raw_rows
        .into_iter()
        .map(|(id, fields_raw)| {
            let fields: JsonMap = parse_json(&fields_raw, &format!("Record {id} fields"))?;
            Ok(TableRecord { id, fields })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    Ok(TablePage { records, total })
}

/// Desktop read_table (docs/ipc.md): limit defaults to 100 and clamps to
/// 1..=500, offset floors at 0.
pub fn read_table_page(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<TablePage, CoreError> {
    let options = ReadOptions {
        limit: Some(
            limit
                .unwrap_or(READ_LIMIT_DEFAULT)
                .clamp(READ_LIMIT_MIN, READ_LIMIT_MAX),
        ),
        offset: Some(offset.unwrap_or(0).max(0)),
    };
    list_records(conn, source_id, table_id, options)
}

/// Inserts new records after the existing ones (position = max + n) with
/// generated `rec_` ids, atomically.
pub fn append_records(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    records: &[JsonMap],
) -> Result<Vec<TableRecord>, CoreError> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| db_error("Could not start append transaction", error))?;

    let max_position: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM mock_records
             WHERE source_id = ?1 AND table_id = ?2",
            params![source_id, table_id],
            |row| row.get(0),
        )
        .map_err(|error| db_error("Could not read record positions", error))?;

    let mut inserted = Vec::with_capacity(records.len());
    for (index, fields) in records.iter().enumerate() {
        let record = TableRecord {
            id: format!("rec_{}", uuid::Uuid::new_v4()),
            fields: fields.clone(),
        };
        let fields_json = serde_json::to_string(&record.fields).map_err(|error| {
            CoreError::Storage(format!("Could not encode record fields: {error}"))
        })?;
        tx.execute(
            "INSERT INTO mock_records (source_id, table_id, record_id, fields, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                source_id,
                table_id,
                record.id,
                fields_json,
                max_position + index as i64 + 1
            ],
        )
        .map_err(|error| db_error("Could not insert record", error))?;
        inserted.push(record);
    }

    tx.commit()
        .map_err(|error| db_error("Could not commit append transaction", error))?;
    Ok(inserted)
}

/// Shallow-merges patch fields into stored fields, atomically. Unknown record
/// ids are skipped (mirrors the in-memory connector semantics); only the
/// records that were actually updated are returned.
pub fn update_records(
    conn: &Connection,
    source_id: &str,
    table_id: &str,
    patches: &[RecordPatch],
) -> Result<Vec<TableRecord>, CoreError> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| db_error("Could not start update transaction", error))?;

    let mut updated = Vec::new();
    for patch in patches {
        let existing: Option<String> = tx
            .query_row(
                "SELECT fields FROM mock_records
                 WHERE source_id = ?1 AND table_id = ?2 AND record_id = ?3",
                params![source_id, table_id, patch.record_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| db_error("Could not read record", error))?;
        let Some(raw_fields) = existing else {
            continue;
        };

        let mut merged: JsonMap =
            parse_json(&raw_fields, &format!("Record {} fields", patch.record_id))?;
        for (key, value) in &patch.fields {
            merged.insert(key.clone(), value.clone());
        }
        let merged_json = serde_json::to_string(&merged).map_err(|error| {
            CoreError::Storage(format!("Could not encode record fields: {error}"))
        })?;
        tx.execute(
            "UPDATE mock_records SET fields = ?1
             WHERE source_id = ?2 AND table_id = ?3 AND record_id = ?4",
            params![merged_json, source_id, table_id, patch.record_id],
        )
        .map_err(|error| db_error("Could not update record", error))?;
        updated.push(TableRecord {
            id: patch.record_id.clone(),
            fields: merged,
        });
    }

    tx.commit()
        .map_err(|error| db_error("Could not commit update transaction", error))?;
    Ok(updated)
}

#[cfg(test)]
#[path = "mock_data_tests.rs"]
mod tests;
