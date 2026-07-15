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
    DataSource, FormatPlan, GridData, GridRow, JsonMap, ReadOptions, RecordPatch, SheetTab,
    SourceKind, TableRecord, TableRef, TableSchema, TableStyle,
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

    /// Read cells with their formulas preserved (raw `=...` strings) instead of
    /// the computed values, record-shaped like [`read_table`](Self::read_table),
    /// so an agent can see and keep formula logic before overwriting a cell.
    /// Connectors that cannot read formulas inherit the Unsupported default.
    fn read_formulas(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support reading formulas".to_string(),
        ))
    }

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

    // -----------------------------------------------------------------------
    // Cell formatting (docs/mcp-tools.md "Formatting"). `read_table_style` is a
    // DIRECT read; `format_cells` runs from the staged-change commit path only,
    // never directly from an agent. Connectors that cannot format cells inherit
    // the Unsupported defaults below.
    // -----------------------------------------------------------------------

    /// The existing style of a tab (header + first data row + sheet freeze and
    /// column widths) so an agent can match it instead of imposing a new look.
    fn read_table_style(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
    ) -> Result<TableStyle, CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support reading cell formatting".to_string(),
        ))
    }

    /// Applies a formatting plan to the resolved tab.
    fn format_cells(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _plan: &FormatPlan,
    ) -> Result<(), CoreError> {
        Err(CoreError::Unsupported(
            "This source does not support cell formatting".to_string(),
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

    pub fn read_formulas(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        self.for_source(conn, source_id)?
            .read_formulas(conn, source_id, table_id, options)
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

    pub fn read_table_style(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableStyle, CoreError> {
        self.for_source(conn, source_id)?
            .read_table_style(conn, source_id, table_id)
    }

    pub fn format_cells(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        plan: &FormatPlan,
    ) -> Result<(), CoreError> {
        self.for_source(conn, source_id)?
            .format_cells(conn, source_id, table_id, plan)
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

/// Highest column count a formatting range may span (A..ZZ, matching the
/// connector's value window).
const GRID_MAX_COLUMNS: usize = 702;
/// Highest 1-based row a formatting range may reference; a sane upper bound so
/// a typo cannot build an absurd grid range.
const GRID_MAX_ROWS: usize = 10_000_000;

/// A parsed A1 range as half-open, zero-based grid bounds. `None` on a
/// dimension means it is unbounded (a whole-column range like `A:B` leaves the
/// rows unbounded; a whole-row range like `1:3` leaves the columns unbounded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A1Range {
    pub start_col: Option<usize>,
    /// Exclusive.
    pub end_col: Option<usize>,
    pub start_row: Option<usize>,
    /// Exclusive.
    pub end_row: Option<usize>,
}

/// One endpoint of an A1 range: a column, a row, or a cell (both). At least one
/// of the two is always present.
struct A1Endpoint {
    col: Option<usize>,
    row: Option<usize>,
}

fn bad_a1_range(range: &str) -> CoreError {
    CoreError::InvalidInput(format!(
        "'{range}' is not a valid A1 range (examples: A1, A1:D10, B:B, 2:2)"
    ))
}

/// Splits an endpoint like `A`, `12`, or `C7` into its column and row parts.
/// Absolute `$` markers are tolerated; letters must precede digits.
fn parse_a1_endpoint(token: &str) -> Option<A1Endpoint> {
    let cleaned: String = token.trim().chars().filter(|ch| *ch != '$').collect();
    if cleaned.is_empty() {
        return None;
    }
    let mut letters = String::new();
    let mut digits = String::new();
    for ch in cleaned.chars() {
        if ch.is_ascii_alphabetic() {
            if !digits.is_empty() {
                return None; // letters after digits is not a valid A1 cell
            }
            letters.push(ch.to_ascii_uppercase());
        } else if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            return None;
        }
    }
    let col = match letters.is_empty() {
        true => None,
        false => Some(column_index_for_id(&letters)?),
    };
    let row = if digits.is_empty() {
        None
    } else {
        let number: usize = digits.parse().ok()?;
        // A1 rows are 1-based, so row 0 (e.g. "A0") is not a valid reference.
        if number == 0 {
            return None;
        }
        Some(number - 1)
    };
    if col.is_none() && row.is_none() {
        return None;
    }
    Some(A1Endpoint { col, row })
}

/// Combines two matching endpoint bounds into a half-open `(start, end)` pair,
/// normalizing reversed inputs. `(None, None)` stays unbounded.
fn combine_bounds(a: Option<usize>, b: Option<usize>) -> (Option<usize>, Option<usize>) {
    match (a, b) {
        (Some(a), Some(b)) => (Some(a.min(b)), Some(a.max(b) + 1)),
        _ => (None, None),
    }
}

/// Parses an A1 range within a single tab into zero-based, half-open grid
/// bounds. Both endpoints must describe the same shape (cell:cell, col:col, or
/// row:row); a bare endpoint (no `:`) covers exactly that cell/column/row.
pub(crate) fn parse_a1_range(range: &str) -> Result<A1Range, CoreError> {
    let trimmed = range.trim();
    if trimmed.is_empty() {
        return Err(bad_a1_range(range));
    }
    let (left, right) = match trimmed.split_once(':') {
        Some((left, right)) => (left, right),
        None => (trimmed, trimmed),
    };
    let start = parse_a1_endpoint(left).ok_or_else(|| bad_a1_range(range))?;
    let end = parse_a1_endpoint(right).ok_or_else(|| bad_a1_range(range))?;
    if start.col.is_some() != end.col.is_some() || start.row.is_some() != end.row.is_some() {
        return Err(bad_a1_range(range));
    }
    let (start_col, end_col) = combine_bounds(start.col, end.col);
    let (start_row, end_row) = combine_bounds(start.row, end.row);
    if end_col.is_some_and(|end| end > GRID_MAX_COLUMNS)
        || end_row.is_some_and(|end| end > GRID_MAX_ROWS)
    {
        return Err(bad_a1_range(range));
    }
    Ok(A1Range {
        start_col,
        end_col,
        start_row,
        end_row,
    })
}

/// Validates that a string is a well-formed A1 range without keeping the parse
/// result. Used at the MCP boundary so a bad range fails at preview time.
pub fn validate_a1_range(range: &str) -> Result<(), CoreError> {
    parse_a1_range(range).map(|_| ())
}

#[cfg(test)]
#[path = "connectors_tests.rs"]
mod tests;
