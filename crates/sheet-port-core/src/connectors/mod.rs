//! Connector abstraction and the registry that routes calls by source kind.
//! second connector for the same kind replaces the first.

mod google_sheets;
mod mock;
mod provider;

pub use google_sheets::{parse_spreadsheet_id, spreadsheet_title, GoogleSheetsConnector};
pub use mock::MockConnector;
pub use provider::ProviderConnector;

use rusqlite::Connection;

use crate::constants::{READ_LIMIT_DEFAULT, READ_LIMIT_MAX, READ_LIMIT_MIN};
use crate::error::CoreError;
use crate::sources;
use crate::types::{
    DataSource, GridData, GridRow, JsonMap, ReadOptions, RecordPatch, SheetTab, SourceKind,
    TableRecord, TableRef, TableSchema,
};

pub trait TableConnector: Send + Sync {
    fn kind(&self) -> SourceKind;
    fn list_sources(&self, conn: &Connection) -> Result<Vec<DataSource>, CoreError>;
    fn list_tables(&self, conn: &Connection, source_id: &str) -> Result<Vec<TableRef>, CoreError>;
    fn describe_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableSchema, CoreError>;
    fn read_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError>;
    fn find_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        query: &str,
    ) -> Result<Vec<TableRecord>, CoreError>;
    fn append_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError>;
    fn update_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError>;

    // -----------------------------------------------------------------------
    // Workbench grid access (docs/ipc.md "Workbench"). These are DIRECT reads
    // and writes with no pending-change/approval flow: the desktop user is the
    // approver. Connectors that cannot back a grid inherit the Unsupported
    // defaults below.
    // -----------------------------------------------------------------------

    /// The tabs of one spreadsheet, left to right.
    fn list_sheet_tabs(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _spreadsheet_id: &str,
    ) -> Result<Vec<SheetTab>, CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support sheet tabs".to_string(),
        ))
    }

    /// A page of one sheet tab as a RAW mirror of string cells: columns are the
    /// A1 column letters and rows start at sheet row 1. `table_id` names the tab
    /// (`{spreadsheetId}:{gid}`).
    fn read_grid(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _limit: Option<i64>,
        _offset: Option<i64>,
    ) -> Result<GridData, CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support grid reads".to_string(),
        ))
    }

    /// Writes one cell. `row_index` is 0-based over ALL sheet rows (row 1 =
    /// index 0); `column_id` is the A1 column letter.
    fn write_cell(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _row_index: i64,
        _column_id: &str,
        _value: &str,
    ) -> Result<(), CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support cell writes".to_string(),
        ))
    }

    /// Appends a row (values keyed by column id), returning its new 0-based
    /// data row index.
    fn append_grid_row(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _values: &GridRow,
    ) -> Result<i64, CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support row appends".to_string(),
        ))
    }
}

#[derive(Default)]
pub struct ConnectorRegistry {
    connectors: Vec<Box<dyn TableConnector>>,
}

impl ConnectorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry with the connectors the broker actually serves today; the
    /// provider connector joins once its auth lands. MockConnector stays
    /// registered for tests and the e2e smoke.
    pub fn with_default_connectors() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(MockConnector));
        registry.register(Box::new(GoogleSheetsConnector::new()));
        registry
    }

    pub fn register(&mut self, connector: Box<dyn TableConnector>) {
        // Same-kind registration replaces in place (Map.set semantics).
        match self
            .connectors
            .iter_mut()
            .find(|existing| existing.kind() == connector.kind())
        {
            Some(slot) => *slot = connector,
            None => self.connectors.push(connector),
        }
    }

    pub fn list_sources(&self, conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        let mut all = Vec::new();
        for connector in &self.connectors {
            all.extend(connector.list_sources(conn)?);
        }
        Ok(all)
    }

    pub fn list_tables(
        &self,
        conn: &Connection,
        source_id: &str,
    ) -> Result<Vec<TableRef>, CoreError> {
        self.for_source(conn, source_id)?
            .list_tables(conn, source_id)
    }

    pub fn describe_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        self.for_source(conn, source_id)?
            .describe_table(conn, source_id, table_id)
    }

    pub fn read_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.for_source(conn, source_id)?
            .read_table(conn, source_id, table_id, options)
    }

    pub fn find_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        query: &str,
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.for_source(conn, source_id)?
            .find_records(conn, source_id, table_id, query)
    }

    pub fn append_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.for_source(conn, source_id)?
            .append_records(conn, source_id, table_id, records)
    }

    pub fn update_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.for_source(conn, source_id)?
            .update_records(conn, source_id, table_id, patches)
    }

    pub fn list_sheet_tabs(
        &self,
        conn: &Connection,
        source_id: &str,
        spreadsheet_id: &str,
    ) -> Result<Vec<SheetTab>, CoreError> {
        self.for_source(conn, source_id)?
            .list_sheet_tabs(conn, source_id, spreadsheet_id)
    }

    pub fn read_grid(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<GridData, CoreError> {
        self.for_source(conn, source_id)?
            .read_grid(conn, source_id, table_id, limit, offset)
    }

    pub fn write_cell(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        row_index: i64,
        column_id: &str,
        value: &str,
    ) -> Result<(), CoreError> {
        self.for_source(conn, source_id)?
            .write_cell(conn, source_id, table_id, row_index, column_id, value)
    }

    pub fn append_grid_row(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        values: &GridRow,
    ) -> Result<i64, CoreError> {
        self.for_source(conn, source_id)?
            .append_grid_row(conn, source_id, table_id, values)
    }

    fn for_source(
        &self,
        conn: &Connection,
        source_id: &str,
    ) -> Result<&dyn TableConnector, CoreError> {
        let kind = sources::get_kind(conn, source_id)?
            .ok_or_else(|| CoreError::NotFound(format!("Unknown source {source_id}")))?;
        self.connectors
            .iter()
            .find(|connector| connector.kind() == kind)
            .map(Box::as_ref)
            .ok_or_else(|| {
                CoreError::Unsupported(format!(
                    "No connector registered for source kind {} (source {source_id})",
                    kind.as_str()
                ))
            })
    }
}

/// Bijective base-26 A1 column label for a zero-based column index
/// (0 -> "A", 25 -> "Z", 26 -> "AA"). Used as the stable id of a GridData
/// column so a cell write can map the id back to its column.
pub(crate) fn column_id_for_index(index: usize) -> String {
    let mut n = index + 1; // 1-based for the bijective base-26 scheme.
    let mut label = Vec::new();
    while n > 0 {
        let remainder = (n - 1) % 26;
        label.push(b'A' + remainder as u8);
        n = (n - 1) / 26;
    }
    label.reverse();
    String::from_utf8(label).expect("column label is always ASCII A-Z")
}

/// Inverse of [`column_id_for_index`]. `None` for an empty id or any character
/// outside `A-Z`, so a bad column id is rejected rather than silently mapped.
pub(crate) fn column_index_for_id(id: &str) -> Option<usize> {
    if id.is_empty() {
        return None;
    }
    let mut n: usize = 0;
    for character in id.chars() {
        if !character.is_ascii_uppercase() {
            return None;
        }
        n = n
            .checked_mul(26)?
            .checked_add((character as u8 - b'A') as usize + 1)?;
    }
    Some(n - 1)
}

/// Clamps a grid read window to the same bounds as `read_table` (default 100,
/// limit 1..=500, offset floors at 0) so every connector's `read_grid` slices
/// consistently.
pub(crate) fn clamp_read_window(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    let limit = limit
        .unwrap_or(READ_LIMIT_DEFAULT)
        .clamp(READ_LIMIT_MIN, READ_LIMIT_MAX);
    let offset = offset.unwrap_or(0).max(0);
    (limit, offset)
}

/// JavaScript `String(value)` parity so find_records substring matching
/// behaves identically across connectors (and matches the TypeScript
/// reference): numbers/booleans stringify plainly, null is "null", arrays
/// join with commas, and plain objects become "[object Object]".
pub(crate) fn js_string(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

#[cfg(test)]
#[path = "connectors_tests.rs"]
mod tests;
