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
    CellWrite, DataSource, FieldSchema, GridColumn, GridData, GridRow, JsonMap, ReadOptions,
    RecordPatch, SheetTab, SourceKind, TableRecord, TableRef, TableSchema,
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

    /// RAW mirror of the table: columns are the A1 column letters (id AND
    /// title); row 0 is the field-name header and each following row is one
    /// record, every cell a string. `total_rows` counts the header plus all
    /// records, and the page window is sliced locally over all rows.
    fn read_grid(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<GridData, CoreError> {
        let schema = self.require_table(conn, source_id, table_id)?;
        let columns: Vec<GridColumn> = (0..schema.fields.len()).map(lettered_column).collect();
        let records =
            mock_data::list_records(conn, source_id, table_id, ReadOptions::default())?.records;

        let mut all_rows: Vec<GridRow> = Vec::with_capacity(records.len() + 1);
        all_rows.push(header_row(&schema.fields));
        all_rows.extend(
            records
                .iter()
                .map(|record| record_row(&schema.fields, record)),
        );

        let total_rows = all_rows.len() as i64;
        let (limit, offset) = clamp_read_window(limit, offset);
        let rows = all_rows
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(GridData {
            columns,
            rows,
            total_rows,
        })
    }

    /// Maps the column letter and the 0-based `row_index` (over all rows, row 0
    /// = the field-name header) back to a record cell, then writes it. The
    /// header row is metadata the mock does not rewrite.
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
        if row_index == 0 {
            return Err(CoreError::InvalidInput(
                "The header row (row 1) is not editable on a mock sheet".to_string(),
            ));
        }
        let field_name = schema.fields[column].name.clone();

        // Row 0 is the header, so data records start at row index 1.
        let record_index = (row_index - 1) as usize;
        let page = mock_data::list_records(conn, source_id, table_id, ReadOptions::default())?;
        let record = page.records.get(record_index).ok_or_else(|| {
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

    /// Coordinate-level writes mapped onto the grid semantics: each A1 cell is
    /// one [`write_cell`](TableConnector::write_cell) (row 1 = the header row,
    /// which stays read-only on a mock sheet).
    fn update_cells(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        cells: &[CellWrite],
    ) -> Result<(), CoreError> {
        for cell in cells {
            // CellWrite rows are 1-based sheet rows; write_cell indexes from 0.
            self.write_cell(
                conn,
                source_id,
                table_id,
                cell.row - 1,
                &cell.column,
                &cell.value,
            )?;
        }
        Ok(())
    }

    /// Appends a record built from the column-letter-keyed values, returning its
    /// new 0-based row index over all rows (= the header plus the previous
    /// record count).
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
        let record_count =
            mock_data::list_records(conn, source_id, table_id, ReadOptions::default())?.total;
        mock_data::append_records(conn, source_id, table_id, &[fields])?;
        // 0-based index over all rows = the header row (1) + previous records.
        Ok(record_count + 1)
    }
}

/// The GridColumn for a field position: the A1 letter as both id and title, so
/// the mock grid reads like Google Sheets (columns A, B, C ...).
fn lettered_column(index: usize) -> GridColumn {
    let letter = column_id_for_index(index);
    GridColumn {
        id: letter.clone(),
        title: letter,
    }
}

/// The header row: each field name as a string cell keyed by its A1 letter.
fn header_row(fields: &[FieldSchema]) -> GridRow {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| (column_id_for_index(index), field.name.clone()))
        .collect()
}

/// One record as a string-cell row keyed by A1 letter; missing fields are empty.
fn record_row(fields: &[FieldSchema], record: &TableRecord) -> GridRow {
    fields
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
        .collect()
}
