//! Mock connector backed by the shared SQLite database, so the desktop UI
//! and the MCP server observe the same tables, records, and committed
//! changes.

use rusqlite::Connection;
use serde_json::Value;

use super::TableConnector;
use crate::constants::FIND_RECORDS_LIMIT;
use crate::error::CoreError;
use crate::types::{
    DataSource, JsonMap, ReadOptions, RecordPatch, SourceKind, TableRecord, TableRef, TableSchema,
};
use crate::{mock_data, sources};

pub struct MockConnector;

impl MockConnector {
    fn assert_source(&self, conn: &Connection, source_id: &str) -> Result<(), CoreError> {
        if sources::get_kind(conn, source_id)? != Some(SourceKind::Mock) {
            return Err(CoreError::NotFound(format!(
                "Unknown mock source {source_id}"
            )));
        }
        Ok(())
    }

    fn require_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        self.assert_source(conn, source_id)?;
        mock_data::get_table(conn, source_id, table_id)?.ok_or_else(|| {
            CoreError::NotFound(format!("Unknown mock table {source_id}/{table_id}"))
        })
    }
}

impl TableConnector for MockConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::Mock
    }

    fn list_sources(&self, conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        Ok(sources::list(conn)?
            .into_iter()
            .filter(|source| source.kind == SourceKind::Mock)
            .collect())
    }

    fn list_tables(&self, conn: &Connection, source_id: &str) -> Result<Vec<TableRef>, CoreError> {
        self.assert_source(conn, source_id)?;
        mock_data::list_tables(conn, source_id)
    }

    fn describe_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        self.require_table(conn, source_id, table_id)
    }

    fn read_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.require_table(conn, source_id, table_id)?;
        Ok(mock_data::list_records(conn, source_id, table_id, options)?.records)
    }

    fn find_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        query: &str,
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.require_table(conn, source_id, table_id)?;
        let normalized = query.to_lowercase();
        let page = mock_data::list_records(conn, source_id, table_id, ReadOptions::default())?;
        Ok(page
            .records
            .into_iter()
            .filter(|record| {
                record
                    .fields
                    .values()
                    .any(|value| js_string(value).to_lowercase().contains(&normalized))
            })
            .take(FIND_RECORDS_LIMIT)
            .collect())
    }

    fn append_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.require_table(conn, source_id, table_id)?;
        mock_data::append_records(conn, source_id, table_id, records)
    }

    fn update_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.require_table(conn, source_id, table_id)?;
        mock_data::update_records(conn, source_id, table_id, patches)
    }
}

/// JavaScript `String(value)` parity so search results match the TypeScript
/// connector exactly: numbers/booleans stringify plainly, null is "null",
/// arrays join with commas, and plain objects become "[object Object]".
fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}
