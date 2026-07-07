//! Mock connector backed by the shared SQLite database, so the desktop UI
//! and the MCP server observe the same tables, records, and committed
//! changes.

use rusqlite::Connection;
use serde_json::Value;

use super::{
    clamp_read_window, column_id_for_index, column_index_for_id, js_string, TableConnector,
};
use crate::constants::FIND_RECORDS_LIMIT;
use crate::error::CoreError;
use crate::types::{
    DataSource, GridColumn, GridData, GridRow, JsonMap, ReadOptions, RecordPatch, SheetTab,
    SourceKind, TableRecord, TableRef, TableSchema,
};
use crate::{mock_data, sources};

/// The single synthetic tab every mock table exposes (the mock has no real
/// tabs). Mirrors a spreadsheet's default first sheet.
const MOCK_TAB_GID: &str = "0";
const MOCK_TAB_TITLE: &str = "Sheet1";

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

    /// The mock has no real tabs, so every table exposes exactly one synthetic
    /// tab (matching a spreadsheet's default first sheet).
    fn list_sheet_tabs(
        &self,
        conn: &Connection,
        source_id: &str,
        _spreadsheet_id: &str,
    ) -> Result<Vec<SheetTab>, CoreError> {
        self.assert_source(conn, source_id)?;
        Ok(vec![SheetTab {
            gid: MOCK_TAB_GID.to_string(),
            title: MOCK_TAB_TITLE.to_string(),
            index: 0,
        }])
    }

    /// The table's field schema becomes the columns (id = A1 column letter,
    /// title = field name); each record becomes a string-cell row keyed by
    /// column id.
    fn read_grid(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<GridData, CoreError> {
        let schema = self.require_table(conn, source_id, table_id)?;
        let columns: Vec<GridColumn> = schema
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| GridColumn {
                id: column_id_for_index(index),
                title: field.name.clone(),
            })
            .collect();
        let (limit, offset) = clamp_read_window(limit, offset);
        let page = mock_data::list_records(
            conn,
            source_id,
            table_id,
            ReadOptions {
                limit: Some(limit),
                offset: Some(offset),
            },
        )?;
        let rows = page
            .records
            .iter()
            .map(|record| {
                schema
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let cell = record
                            .fields
                            .get(&field.name)
                            .map(js_string)
                            .unwrap_or_default();
                        (column_id_for_index(index), cell)
                    })
                    .collect::<GridRow>()
            })
            .collect();
        Ok(GridData {
            columns,
            rows,
            total_rows: page.total,
        })
    }

    /// Maps the column id back to its field and the 0-based `row_index` back to
    /// the record at that position, then writes the value as a string cell.
    fn write_cell(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        row_index: i64,
        column_id: &str,
        value: &str,
    ) -> Result<(), CoreError> {
        if row_index < 0 {
            return Err(CoreError::InvalidInput(format!(
                "Row index must not be negative, got {row_index}"
            )));
        }
        let schema = self.require_table(conn, source_id, table_id)?;
        let column = column_index_for_id(column_id)
            .filter(|index| *index < schema.fields.len())
            .ok_or_else(|| CoreError::InvalidInput(format!("Unknown column {column_id}")))?;
        let field_name = schema.fields[column].name.clone();

        let page = mock_data::list_records(conn, source_id, table_id, ReadOptions::default())?;
        let record = page.records.get(row_index as usize).ok_or_else(|| {
            CoreError::NotFound(format!(
                "Row {row_index} does not exist in {source_id}/{table_id}"
            ))
        })?;
        let mut fields = JsonMap::new();
        fields.insert(field_name, Value::String(value.to_string()));
        mock_data::update_records(
            conn,
            source_id,
            table_id,
            &[RecordPatch {
                record_id: record.id.clone(),
                fields,
            }],
        )?;
        Ok(())
    }

    /// Appends a record built from the column-id-keyed values, returning its
    /// new 0-based row index (the record count before the append).
    fn append_grid_row(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        values: &GridRow,
    ) -> Result<i64, CoreError> {
        let schema = self.require_table(conn, source_id, table_id)?;
        let mut fields = JsonMap::new();
        for (index, field) in schema.fields.iter().enumerate() {
            if let Some(cell) = values.get(&column_id_for_index(index)) {
                fields.insert(field.name.clone(), Value::String(cell.clone()));
            }
        }
        let before =
            mock_data::list_records(conn, source_id, table_id, ReadOptions::default())?.total;
        mock_data::append_records(conn, source_id, table_id, &[fields])?;
        Ok(before)
    }
}
