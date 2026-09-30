//! `read_formats`: per-cell formatting of a tab window and its compact wire
//! encoding (docs/mcp-tools.md "read_formats"). Connectors return window-
//! anchored [`CellSample`] rows; [`shape_format_grid`] fixes the grid size and
//! enforces the cell cap the same way for every connector, and
//! [`encode_formats`] turns the grid into one palette plus run-length rows per
//! requested field.

use std::collections::HashMap;
use std::hash::Hash;

use serde::Serialize;

use super::{column_id_for_index, A1Range, GRID_MAX_COLUMNS};
use crate::constants::READ_FORMATS_MAX_CELLS_SAVED;
use crate::error::CoreError;

/// Which per-cell properties a `read_formats` call returns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatFields {
    pub background: bool,
    pub font_color: bool,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub underline: bool,
    pub font_family: bool,
    pub font_size: bool,
    pub vertical_alignment: bool,
    pub value: bool,
}

impl FormatFields {
    /// True when any format property (not just the value) is requested.
    pub fn any_format(&self) -> bool {
        self.background
            || self.font_color
            || self.bold
            || self.italic
            || self.strikethrough
            || self.underline
            || self.font_family
            || self.font_size
            || self.vertical_alignment
    }

    /// True when a color property is requested.
    pub fn any_color(&self) -> bool {
        self.background || self.font_color
    }

    /// The wire names of the requested fields, in the documented order.
    pub fn names(&self) -> Vec<&'static str> {
        [
            (self.background, "background"),
            (self.font_color, "fontColor"),
            (self.bold, "bold"),
            (self.italic, "italic"),
            (self.strikethrough, "strikethrough"),
            (self.underline, "underline"),
            (self.font_family, "fontFamily"),
            (self.font_size, "fontSize"),
            (self.vertical_alignment, "verticalAlignment"),
            (self.value, "value"),
        ]
        .into_iter()
        .filter_map(|(set, name)| set.then_some(name))
        .collect()
    }
}

/// Which format a `read_formats` call reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FormatSource {
    /// What the user sees: the cell format merged with defaults and the
    /// results of conditional formatting (Sheets `effectiveFormat`).
    #[default]
    Effective,
    /// Only the format set on the cell itself (Sheets `userEnteredFormat`).
    UserEntered,
}

impl FormatSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Effective => "effective",
            Self::UserEntered => "userEntered",
        }
    }
}

/// One `read_formats` call as the connector sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatsRequest {
    /// The window within the tab; `None` reads the whole tab.
    pub range: Option<A1Range>,
    pub fields: FormatFields,
    pub source: FormatSource,
    /// Most cells the shaped grid may hold.
    pub max_cells: usize,
}

/// The requested properties of one cell. Unrequested properties stay at
/// their defaults (see [`CellSample::masked`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellSample {
    /// `#rrggbb`, or `None` for no fill.
    pub background: Option<String>,
    /// `#rrggbb`, or `None` for the default text color.
    pub font_color: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub underline: bool,
    /// Font name, or `None` when the format carries none.
    pub font_family: Option<String>,
    /// Font size in points, or `None` when the format carries none.
    pub font_size: Option<i64>,
    /// `TOP`, `MIDDLE`, or `BOTTOM`, or `None` when the format carries none.
    pub vertical_alignment: Option<String>,
    /// The formatted value as shown to the user.
    pub value: String,
}

impl CellSample {
    /// A copy holding only the requested properties.
    pub fn masked(self, fields: &FormatFields) -> Self {
        Self {
            background: self.background.filter(|_| fields.background),
            font_color: self.font_color.filter(|_| fields.font_color),
            bold: self.bold && fields.bold,
            italic: self.italic && fields.italic,
            strikethrough: self.strikethrough && fields.strikethrough,
            underline: self.underline && fields.underline,
            font_family: self.font_family.filter(|_| fields.font_family),
            font_size: self.font_size.filter(|_| fields.font_size),
            vertical_alignment: self
                .vertical_alignment
                .filter(|_| fields.vertical_alignment),
            value: if fields.value {
                self.value
            } else {
                String::new()
            },
        }
    }

    /// True when every requested property holds its default.
    fn is_default_for(&self, fields: &FormatFields) -> bool {
        !((fields.background && self.background.is_some())
            || (fields.font_color && self.font_color.is_some())
            || (fields.bold && self.bold)
            || (fields.italic && self.italic)
            || (fields.strikethrough && self.strikethrough)
            || (fields.underline && self.underline)
            || (fields.font_family && self.font_family.is_some())
            || (fields.font_size && self.font_size.is_some())
            || (fields.vertical_alignment && self.vertical_alignment.is_some())
            || (fields.value && !self.value.is_empty()))
    }
}

/// A shaped `read_formats` window: `rows` x `columns` cells starting at the
/// zero-based `start_row`/`start_col`. `cells` may be ragged or short; a
/// missing cell is a default cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatGrid {
    pub sheet_title: String,
    pub start_row: usize,
    pub start_col: usize,
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<Vec<CellSample>>,
}

impl FormatGrid {
    pub fn cell_count(&self) -> usize {
        self.rows * self.columns
    }
}

/// The InvalidInput a call over the cell cap gets. Below the `saveTo` cap it
/// points at `saveTo` as well as a smaller range.
pub fn too_many_cells(cells: usize, max_cells: usize) -> CoreError {
    let hint = if max_cells < READ_FORMATS_MAX_CELLS_SAVED {
        format!(
            "pass a smaller range, or saveTo to write up to {READ_FORMATS_MAX_CELLS_SAVED} cells to a file"
        )
    } else {
        "pass a smaller range".to_string()
    };
    CoreError::InvalidInput(format!(
        "read_formats covers {cells} cells, over the limit of {max_cells} per call: {hint}"
    ))
}

/// Guards the provider fetch before it happens, using the tab's grid size
/// (`grid_rows` x `grid_cols`). A range bounded on both dimensions returns
/// exactly its window (clamped to the grid), so it is held to `max_cells`
/// here. An open dimension is trimmed to the used cells after the fetch, so
/// it only has to stay under the `saveTo` cap now and is checked against
/// `max_cells` by [`shape_format_grid`].
pub fn check_fetch_size(
    range: Option<&A1Range>,
    grid_rows: usize,
    grid_cols: usize,
    max_cells: usize,
) -> Result<(), CoreError> {
    let start_row = range.and_then(|range| range.start_row).unwrap_or(0);
    let start_col = range.and_then(|range| range.start_col).unwrap_or(0);
    let end_row = range
        .and_then(|range| range.end_row)
        .map_or(grid_rows, |end| end.min(grid_rows));
    let end_col = range
        .and_then(|range| range.end_col)
        .map_or(grid_cols, |end| end.min(grid_cols))
        .min(GRID_MAX_COLUMNS);
    let cells = end_row.saturating_sub(start_row) * end_col.saturating_sub(start_col);
    let bounded = range.is_some_and(|range| range.end_row.is_some() && range.end_col.is_some());
    let limit = if bounded {
        max_cells
    } else {
        max_cells.max(READ_FORMATS_MAX_CELLS_SAVED)
    };
    if cells > limit {
        return Err(too_many_cells(cells, limit));
    }
    Ok(())
}

/// Sizes the window and enforces the cell cap. `cells` is anchored at the
/// window's top-left cell. A dimension the range bounds keeps its requested
/// size, clamped to the tab's grid (`grid_rows`/`grid_cols` when known) and
/// to the A:ZZ column window. An open dimension (no range, or `A:C` / `5:9`)
/// is trimmed to the last row/column holding a non-default requested
/// property. Cells beyond the window are dropped.
pub fn shape_format_grid(
    sheet_title: String,
    request: &FormatsRequest,
    grid_rows: Option<usize>,
    grid_cols: Option<usize>,
    cells: Vec<Vec<CellSample>>,
) -> Result<FormatGrid, CoreError> {
    let range = request.range.as_ref();
    let fields = &request.fields;
    let start_row = range.and_then(|range| range.start_row).unwrap_or(0);
    let start_col = range.and_then(|range| range.start_col).unwrap_or(0);

    let used_rows = cells
        .iter()
        .rposition(|row| row.iter().any(|cell| !cell.is_default_for(fields)))
        .map_or(0, |last| last + 1);
    let used_cols = cells
        .iter()
        .filter_map(|row| row.iter().rposition(|cell| !cell.is_default_for(fields)))
        .max()
        .map_or(0, |last| last + 1);

    let rows = match range.and_then(|range| range.end_row) {
        Some(end) => {
            let requested = end.saturating_sub(start_row);
            grid_rows.map_or(requested, |grid| {
                requested.min(grid.saturating_sub(start_row))
            })
        }
        None => used_rows,
    };
    let max_cols = GRID_MAX_COLUMNS.saturating_sub(start_col);
    let columns = match range.and_then(|range| range.end_col) {
        Some(end) => {
            let requested = end.saturating_sub(start_col);
            grid_cols.map_or(requested, |grid| {
                requested.min(grid.saturating_sub(start_col))
            })
        }
        None => used_cols,
    }
    .min(max_cols);

    let total = rows * columns;
    if total > request.max_cells {
        return Err(too_many_cells(total, request.max_cells));
    }
    let cells = cells
        .into_iter()
        .take(rows)
        .map(|mut row| {
            row.truncate(columns);
            row
        })
        .collect();
    Ok(FormatGrid {
        sheet_title,
        start_row,
        start_col,
        rows,
        columns,
        cells,
    })
}

/// One palette-encoded field: `palette[0]` is the default, `counts[i]` the
/// cells using `palette[i]`, and `grid` one run-length row per window row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaletteLayer<T> {
    pub palette: Vec<T>,
    pub counts: Vec<u64>,
    pub grid: Vec<String>,
}

/// The formatted values: one array per window row, trailing empty cells
/// trimmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValueLayer {
    pub grid: Vec<Vec<String>>,
}

/// The `read_formats` result (docs/mcp-tools.md "read_formats").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatsOutput {
    pub sheet_title: String,
    /// A1 range of the returned window, e.g. `A1:OA95` (just the start cell
    /// when the window is empty).
    pub range: String,
    /// 1-based sheet row of grid row 0.
    pub start_row: i64,
    /// Column letter of grid column 0.
    pub start_column: String,
    pub rows: usize,
    pub columns: usize,
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<PaletteLayer<Option<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<PaletteLayer<Option<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<PaletteLayer<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<PaletteLayer<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<PaletteLayer<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<PaletteLayer<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<PaletteLayer<Option<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<PaletteLayer<Option<i64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_alignment: Option<PaletteLayer<Option<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueLayer>,
}

/// Encodes a shaped grid into the compact output, one layer per requested
/// field.
pub fn encode_formats(
    grid: &FormatGrid,
    fields: &FormatFields,
    source: FormatSource,
) -> FormatsOutput {
    let first_row = grid.start_row + 1;
    let first_col = column_id_for_index(grid.start_col);
    let range = if grid.rows == 0 || grid.columns == 0 {
        format!("{first_col}{first_row}")
    } else {
        format!(
            "{first_col}{first_row}:{}{}",
            column_id_for_index(grid.start_col + grid.columns - 1),
            grid.start_row + grid.rows
        )
    };
    FormatsOutput {
        sheet_title: grid.sheet_title.clone(),
        range,
        start_row: first_row as i64,
        start_column: first_col,
        rows: grid.rows,
        columns: grid.columns,
        source: source.as_str(),
        background: fields
            .background
            .then(|| encode_layer(grid, None, |cell| cell.background.clone())),
        font_color: fields
            .font_color
            .then(|| encode_layer(grid, None, |cell| cell.font_color.clone())),
        bold: fields
            .bold
            .then(|| encode_layer(grid, false, |cell| cell.bold)),
        italic: fields
            .italic
            .then(|| encode_layer(grid, false, |cell| cell.italic)),
        strikethrough: fields
            .strikethrough
            .then(|| encode_layer(grid, false, |cell| cell.strikethrough)),
        underline: fields
            .underline
            .then(|| encode_layer(grid, false, |cell| cell.underline)),
        font_family: fields
            .font_family
            .then(|| encode_layer(grid, None, |cell| cell.font_family.clone())),
        font_size: fields
            .font_size
            .then(|| encode_layer(grid, None, |cell| cell.font_size)),
        vertical_alignment: fields
            .vertical_alignment
            .then(|| encode_layer(grid, None, |cell| cell.vertical_alignment.clone())),
        value: fields.value.then(|| ValueLayer {
            grid: (0..grid.rows)
                .map(|row| {
                    let cells = grid.cells.get(row).map(Vec::as_slice).unwrap_or_default();
                    let mut values: Vec<String> =
                        cells.iter().map(|cell| cell.value.clone()).collect();
                    while values.last().is_some_and(String::is_empty) {
                        values.pop();
                    }
                    values
                })
                .collect(),
        }),
    }
}

/// Builds one palette layer: `default` is palette entry 0, other values are
/// added in row-major order of first appearance, and every window cell
/// (missing ones as the default) is counted.
fn encode_layer<T, F>(grid: &FormatGrid, default: T, get: F) -> PaletteLayer<T>
where
    T: Clone + Eq + Hash,
    F: Fn(&CellSample) -> T,
{
    let mut palette = vec![default.clone()];
    let mut lookup: HashMap<T, usize> = HashMap::from([(default, 0)]);
    let mut counts = vec![0u64];
    let mut rows = Vec::with_capacity(grid.rows);
    for row in 0..grid.rows {
        let cells = grid.cells.get(row).map(Vec::as_slice).unwrap_or_default();
        let mut indices = Vec::with_capacity(grid.columns);
        for column in 0..grid.columns {
            let index = match cells.get(column) {
                Some(cell) => {
                    let value = get(cell);
                    match lookup.get(&value) {
                        Some(index) => *index,
                        None => {
                            let index = palette.len();
                            palette.push(value.clone());
                            lookup.insert(value, index);
                            counts.push(0);
                            index
                        }
                    }
                }
                None => 0,
            };
            counts[index] += 1;
            indices.push(index);
        }
        rows.push(encode_runs(&indices));
    }
    PaletteLayer {
        palette,
        counts,
        grid: rows,
    }
}

/// Run-length encodes one row of palette indices: comma-separated `index*count`
/// runs, a bare `index` for a run of one, with trailing runs of index 0 (the
/// default) dropped. An all-default row is the empty string.
pub fn encode_runs(indices: &[usize]) -> String {
    let end = indices
        .iter()
        .rposition(|index| *index != 0)
        .map_or(0, |last| last + 1);
    let mut runs: Vec<String> = Vec::new();
    let mut position = 0;
    while position < end {
        let index = indices[position];
        let length = indices[position..end]
            .iter()
            .take_while(|other| **other == index)
            .count();
        runs.push(if length == 1 {
            index.to_string()
        } else {
            format!("{index}*{length}")
        });
        position += length;
    }
    runs.join(",")
}

/// Inverse of [`encode_runs`]: expands a row back to exactly `columns`
/// indices, padding the trimmed tail with 0. Errors on a malformed run or a
/// row longer than `columns`.
pub fn decode_runs(row: &str, columns: usize) -> Result<Vec<usize>, String> {
    let mut indices = Vec::with_capacity(columns);
    if !row.is_empty() {
        for run in row.split(',') {
            let (index, count) = match run.split_once('*') {
                Some((index, count)) => (index, count),
                None => (run, "1"),
            };
            let index: usize = index.parse().map_err(|_| format!("bad run '{run}'"))?;
            let count: usize = count.parse().map_err(|_| format!("bad run '{run}'"))?;
            if count == 0 {
                return Err(format!("bad run '{run}'"));
            }
            indices.extend(std::iter::repeat_n(index, count));
        }
    }
    if indices.len() > columns {
        return Err(format!(
            "row has {} cells, more than {columns}",
            indices.len()
        ));
    }
    indices.resize(columns, 0);
    Ok(indices)
}

#[cfg(test)]
#[path = "formats_tests.rs"]
mod tests;
