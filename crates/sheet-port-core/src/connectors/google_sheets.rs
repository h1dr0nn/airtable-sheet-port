//! Real Google Sheets connector: Drive `files.list` for spreadsheet
//! discovery and the Sheets values API for reads and writes. The first row of
//! the first visible sheet is the header; records map 1:1 onto sheet rows
//! with ids "row_{n}" (n = 1-based sheet row, so data starts at row_2).
//! Tokens are obtained through the crate-private google module and never
//! leave this crate.

use rusqlite::Connection;
use serde_json::{json, Value};

use super::{
    clamp_read_window, column_id_for_index, column_index_for_id, js_string, parse_a1_range,
    A1Range, TableConnector,
};
use crate::constants::FIND_RECORDS_LIMIT;
use crate::error::CoreError;
use crate::google;
use crate::sources;
use crate::types::{
    BorderStyle, CellFormat, CellStyle, ColumnWidth, DataSource, FieldSchema, FormatPlan,
    GridColumn, GridData, GridRow, JsonMap, NumberFormatType, ReadOptions, RecordPatch, SheetTab,
    SourceKind, TableRecord, TableRef, TableSchema, TableStyle,
};

const DRIVE_FILES_ENDPOINT: &str = "https://www.googleapis.com/drive/v3/files";
const SHEETS_ENDPOINT: &str = "https://sheets.googleapis.com/v4/spreadsheets";
const SPREADSHEET_MIME_TYPE: &str = "application/vnd.google-apps.spreadsheet";
const DRIVE_PAGE_SIZE: &str = "100";

/// Column window for value ranges; ZZ = 702 columns, far beyond broker use.
const LAST_COLUMN: &str = "ZZ";
const HEADER_ROW: i64 = 1;
const FIRST_DATA_ROW: i64 = 2;
/// First row of the RAW Workbench grid mirror: row 1 is real data, not a
/// header (the record/table view above still treats row 1 as the header).
const FIRST_SHEET_ROW: i64 = 1;
/// Smallest column count a raw grid reports, so a fully empty sheet still shows
/// a column A to type into.
const MIN_GRID_COLUMNS: usize = 1;
const RECORD_ID_PREFIX: &str = "row_";

/// Values API `valueInputOption`: write cell values exactly as sent.
const VALUE_INPUT_RAW: &str = "RAW";

/// Separator between a spreadsheet id and a sheet selector in a tableId
/// (`{spreadsheetId}:{gid}` or `{spreadsheetId}:{SheetName}`).
const TABLE_ID_SELECTOR_SEPARATOR: char = ':';
/// Path segment that precedes the spreadsheet id in a Google Sheets URL
/// (`https://docs.google.com/spreadsheets/d/{ID}/edit`).
const SHEETS_URL_ID_MARKER: &str = "/d/";
/// Query/fragment key carrying the tab id in a Google Sheets URL (`gid=0`).
const SHEETS_URL_GID_KEY: &str = "gid";
/// Google document ids are URL-safe base64-ish tokens; this bounds what the
/// parser will treat as a plausible id so junk / hostnames are rejected before
/// any request is built. Real ids are ~44 chars, so 20 is a comfortable floor.
const MIN_SPREADSHEET_ID_LEN: usize = 20;

#[derive(Default)]
pub struct GoogleSheetsConnector;

impl GoogleSheetsConnector {
    pub fn new() -> Self {
        Self
    }

    /// Header cells of the resolved sheet as field names (row 1).
    fn fetch_header(&self, token: &str, sheet: &ResolvedSheet) -> Result<Vec<String>, CoreError> {
        let rows = fetch_values(
            token,
            &sheet.spreadsheet_id,
            &sheet.range(&format!("A{HEADER_ROW}:{LAST_COLUMN}{HEADER_ROW}")),
        )?;
        Ok(rows
            .first()
            .map(|cells| cells.iter().map(js_string).collect())
            .unwrap_or_default())
    }

    /// All current data rows as records (used by find/update flows).
    fn read_all(
        &self,
        token: &str,
        sheet: &ResolvedSheet,
        header: &[String],
    ) -> Result<Vec<TableRecord>, CoreError> {
        let rows = fetch_values(
            token,
            &sheet.spreadsheet_id,
            &sheet.range(&format!("A{FIRST_DATA_ROW}:{LAST_COLUMN}")),
        )?;
        Ok(records_from_rows(header, &rows, FIRST_DATA_ROW))
    }
}

impl TableConnector for GoogleSheetsConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::GoogleSheets
    }

    /// One row per connected Google account. No network call: the connected
    /// accounts are the keyed "google-sheets:{accountKey}" source rows, kept in
    /// lockstep with the keychain by the connect/disconnect flow.
    fn list_sources(&self, conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        let connected: std::collections::HashSet<String> = google::list_accounts(conn)?
            .into_iter()
            .map(|account| account.source_id)
            .collect();
        Ok(sources::list(conn)?
            .into_iter()
            .filter(|source| {
                source.kind == SourceKind::GoogleSheets && connected.contains(&source.id)
            })
            .collect())
    }

    /// Spreadsheets visible to the account via Drive `files.list`; each file
    /// is exposed as one table (its first visible sheet).
    fn list_tables(&self, conn: &Connection, source_id: &str) -> Result<Vec<TableRef>, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let query = format!("mimeType='{SPREADSHEET_MIME_TYPE}' and trashed=false");
        let url = url::Url::parse_with_params(
            DRIVE_FILES_ENDPOINT,
            &[
                ("q", query.as_str()),
                ("pageSize", DRIVE_PAGE_SIZE),
                ("fields", "files(id,name)"),
                ("orderBy", "name"),
            ],
        )
        .map_err(|error| {
            CoreError::Storage(format!("Could not build the Drive listing URL: {error}"))
        })?;
        let body = google::get_json(&token, url.as_str())?;

        let files = body["files"].as_array().cloned().unwrap_or_default();
        Ok(files
            .iter()
            .filter_map(|file| {
                let id = file["id"].as_str()?;
                let name = file["name"].as_str().unwrap_or(id);
                Some(TableRef {
                    source_id: source_id.to_string(),
                    table_id: id.to_string(),
                    name: name.to_string(),
                })
            })
            .collect())
    }

    /// Header row 1 becomes the field list; types are inferred from the first
    /// data row (number/boolean/string, defaulting to string).
    fn describe_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let rows = fetch_values(
            &token,
            &sheet.spreadsheet_id,
            &sheet.range(&format!("A{HEADER_ROW}:{LAST_COLUMN}{FIRST_DATA_ROW}")),
        )?;
        let header = rows.first().cloned().unwrap_or_default();
        let first_data_row = rows.get(1).map(Vec::as_slice);
        Ok(TableSchema {
            source_id: source_id.to_string(),
            table_id: table_id.to_string(),
            name: sheet.display_name(),
            fields: schema_from_rows(&header, first_data_row),
        })
    }

    fn read_table(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let header = self.fetch_header(&token, &sheet)?;
        if header.is_empty() {
            return Ok(Vec::new());
        }

        let offset = options.offset.unwrap_or(0).max(0);
        let start_row = FIRST_DATA_ROW + offset;
        let range = match options.limit {
            Some(limit) if limit <= 0 => return Ok(Vec::new()),
            Some(limit) => format!("A{start_row}:{LAST_COLUMN}{}", start_row + limit - 1),
            None => format!("A{start_row}:{LAST_COLUMN}"),
        };
        let rows = fetch_values(&token, &sheet.spreadsheet_id, &sheet.range(&range))?;
        Ok(records_from_rows(&header, &rows, start_row))
    }

    /// Case-insensitive substring match over every field's string form,
    /// capped at [`FIND_RECORDS_LIMIT`] (same semantics as the mock
    /// connector).
    fn find_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        query: &str,
    ) -> Result<Vec<TableRecord>, CoreError> {
        let records = self.read_table(conn, source_id, table_id, ReadOptions::default())?;
        let normalized = query.to_lowercase();
        Ok(records
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

    /// `values.append` with RAW input; each record becomes one row with cells
    /// ordered by the header. Row ids come from the API's updatedRange.
    ///
    /// On an empty sheet (no header row yet) the field names of the incoming
    /// records seed row 1 as the header before the records are appended below
    /// it, so a fresh tab can be populated in one call - matching the Airtable
    /// model where the record fields define the columns.
    fn append_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let existing_header = self.fetch_header(&token, &sheet)?;

        // Empty sheet: derive the header from the record fields and write it as
        // row 1. Otherwise map onto the header already in the sheet.
        let seed_header = existing_header.is_empty();
        let header = if seed_header {
            header_from_records(records)
        } else {
            existing_header
        };
        if header.is_empty() {
            return Err(CoreError::InvalidInput(format!(
                "Cannot append to spreadsheet {}: the sheet is empty and the records carry no fields to build a header from",
                sheet.spreadsheet_id
            )));
        }

        let values = append_values(&header, records, seed_header);
        let url = values_append_url(
            &sheet.spreadsheet_id,
            &sheet.range(&format!("A{HEADER_ROW}")),
        )?;
        let body = google::post_json(&token, url.as_str(), &json!({ "values": values }))?;

        let start_row = body["updates"]["updatedRange"]
            .as_str()
            .and_then(parse_start_row_from_range)
            .ok_or_else(|| {
                CoreError::Storage(
                    "Google Sheets append response had no usable updatedRange".to_string(),
                )
            })?;
        // When we seeded the header it occupies `start_row`, so the records land
        // on the row after it.
        let first_record_row = if seed_header {
            start_row + 1
        } else {
            start_row
        };
        Ok(records
            .iter()
            .enumerate()
            .map(|(index, fields)| TableRecord {
                id: record_id_for(first_record_row + index as i64),
                fields: fields.clone(),
            })
            .collect())
    }

    /// Parses sheet row numbers back out of the record ids and rewrites each
    /// patched row through `values.batchUpdate` (RAW). Unknown or
    /// out-of-range ids are skipped, mirroring the mock connector.
    fn update_records(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError> {
        if patches.is_empty() {
            return Ok(Vec::new());
        }
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let header = self.fetch_header(&token, &sheet)?;
        if header.is_empty() {
            return Err(CoreError::InvalidInput(format!(
                "Spreadsheet {} is empty, so there are no records to update; use append_records to create the table first",
                sheet.spreadsheet_id
            )));
        }
        let current = self.read_all(&token, &sheet, &header)?;

        let mut updated = Vec::new();
        let mut data_entries = Vec::new();
        for patch in patches {
            let Some(row_number) = parse_row_number(&patch.record_id) else {
                continue;
            };
            let Some(existing) = current.iter().find(|record| record.id == patch.record_id) else {
                continue;
            };
            let mut merged = existing.fields.clone();
            for (key, value) in &patch.fields {
                merged.insert(key.clone(), value.clone());
            }
            data_entries.push(json!({
                "range": sheet.range(&format!("A{row_number}")),
                "values": [row_values(&header, &merged)],
            }));
            updated.push(TableRecord {
                id: patch.record_id.clone(),
                fields: merged,
            });
        }
        if data_entries.is_empty() {
            return Ok(updated);
        }

        let url = values_batch_update_url(&sheet.spreadsheet_id)?;
        google::post_json(
            &token,
            url.as_str(),
            &json!({ "valueInputOption": VALUE_INPUT_RAW, "data": data_entries }),
        )?;
        Ok(updated)
    }

    /// Every tab of the spreadsheet, ordered left to right by its `index`.
    fn list_sheet_tabs(
        &self,
        conn: &Connection,
        source_id: &str,
        spreadsheet_id: &str,
    ) -> Result<Vec<SheetTab>, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let meta = fetch_spreadsheet_meta(&token, spreadsheet_id)?;
        let mut tabs: Vec<SheetTab> = meta
            .sheets
            .iter()
            .map(|sheet| SheetTab {
                gid: sheet.sheet_id.to_string(),
                title: sheet.title.clone(),
                index: sheet.index,
            })
            .collect();
        tabs.sort_by_key(|tab| tab.index);
        Ok(tabs)
    }

    /// RAW mirror of the sheet (Workbench). The whole used range is read from
    /// row 1, columns are the A1 column letters (id AND title), and EVERY sheet
    /// row - starting at row 1 - becomes a string-cell row keyed by column
    /// letter. Empty cells are empty strings. One unbounded fetch keeps
    /// `total_rows` exact, then the page window is sliced locally with the
    /// standard read bounds.
    fn read_grid(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<GridData, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let all_rows = fetch_values(
            &token,
            &sheet.spreadsheet_id,
            &sheet.range(&format!("A{FIRST_SHEET_ROW}:{LAST_COLUMN}")),
        )?;
        let column_count = raw_column_count(&all_rows);
        let columns: Vec<GridColumn> = (0..column_count).map(grid_column_for_index).collect();
        let total_rows = all_rows.len() as i64;
        let (limit, offset) = clamp_read_window(limit, offset);
        let rows = all_rows
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|row| raw_grid_row(column_count, row))
            .collect();
        Ok(GridData {
            columns,
            rows,
            total_rows,
        })
    }

    /// Writes one cell via `values.batchUpdate` (RAW). `row_index` is 0-based
    /// over ALL sheet rows (row 1 = index 0), so the sheet row is
    /// `row_index + FIRST_SHEET_ROW`; `column_id` is the A1 column letter.
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
        let column = column_index_for_id(column_id)
            .filter(|index| *index < last_column_count())
            .ok_or_else(|| CoreError::InvalidInput(format!("Unknown column {column_id}")))?;
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;

        let sheet_row = row_index + FIRST_SHEET_ROW;
        let cell = format!("{}{sheet_row}", column_id_for_index(column));
        let url = values_batch_update_url(&sheet.spreadsheet_id)?;
        google::post_json(
            &token,
            url.as_str(),
            &json!({
                "valueInputOption": VALUE_INPUT_RAW,
                "data": [{ "range": sheet.range(&cell), "values": [[value]] }],
            }),
        )?;
        Ok(())
    }

    /// Appends one row via `values.append` (RAW). Cells are ordered by column
    /// letter (A, B, C ...); a column absent from `values` writes an empty cell.
    /// Returns the new row's 0-based index (row 1 = index 0), which equals the
    /// previous `total_rows`.
    fn append_grid_row(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        values: &GridRow,
    ) -> Result<i64, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let row = row_from_column_values(values);
        let url = values_append_url(
            &sheet.spreadsheet_id,
            &sheet.range(&format!("A{FIRST_SHEET_ROW}")),
        )?;
        let body = google::post_json(&token, url.as_str(), &json!({ "values": [row] }))?;
        let start_row = body["updates"]["updatedRange"]
            .as_str()
            .and_then(parse_start_row_from_range)
            .ok_or_else(|| {
                CoreError::Storage(
                    "Google Sheets append response had no usable updatedRange".to_string(),
                )
            })?;
        Ok(start_row - FIRST_SHEET_ROW)
    }

    /// The effective style of the tab: header row, first data row, sheet freeze
    /// counts, and column widths. One `spreadsheets.get` with `includeGridData`
    /// over the first two rows (docs/mcp-tools.md "get_table_style").
    fn read_table_style(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<TableStyle, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        read_table_style_for(&token, &sheet)
    }

    /// Applies a formatting plan through `spreadsheets.batchUpdate` (cell
    /// formats, header freeze, and column widths). Called only from the
    /// staged-change commit path.
    fn format_cells(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        plan: &FormatPlan,
    ) -> Result<(), CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        apply_format_plan(&token, &sheet, plan)
    }
}

/// Parses a Google Sheets URL, bare spreadsheet id, or `id:selector` down to
/// just the spreadsheet id (SSRF-guarded by the same parser the connector
/// uses). Exposed for the Workbench add-spreadsheet flow.
pub fn parse_spreadsheet_id(input: &str) -> Result<String, CoreError> {
    Ok(ParsedTableId::parse(input)?.spreadsheet_id)
}

/// The spreadsheet's own title (`properties.title`), for the Workbench display
/// name. Uses the connected account's token against the fixed Sheets endpoint.
pub fn spreadsheet_title(
    conn: &Connection,
    source_id: &str,
    spreadsheet_id: &str,
) -> Result<String, CoreError> {
    let token = google::access_token(conn, source_id)?;
    Ok(fetch_spreadsheet_meta(&token, spreadsheet_id)?.title)
}

/// The GridColumn for a zero-based column index: the A1 letter is both id and
/// title, so the Workbench grid reads like Google Sheets (columns A, B, C ...).
fn grid_column_for_index(index: usize) -> GridColumn {
    let letter = column_id_for_index(index);
    GridColumn {
        id: letter.clone(),
        title: letter,
    }
}

/// Raw grid width: the widest returned row, floored at [`MIN_GRID_COLUMNS`] so
/// an empty sheet still shows a column, and capped at the [`LAST_COLUMN`]
/// window.
fn raw_column_count(rows: &[Vec<Value>]) -> usize {
    let widest = rows.iter().map(Vec::len).max().unwrap_or(0);
    widest.clamp(MIN_GRID_COLUMNS, last_column_count())
}

/// Column count of the A1 window bounded by [`LAST_COLUMN`] (ZZ -> 702).
fn last_column_count() -> usize {
    column_index_for_id(LAST_COLUMN)
        .map(|index| index + 1)
        .unwrap_or(MIN_GRID_COLUMNS)
}

/// One raw grid row: every column index gets a string cell keyed by its A1
/// letter, empty when the sheet row is short.
fn raw_grid_row(column_count: usize, row: &[Value]) -> GridRow {
    (0..column_count)
        .map(|index| {
            let cell = row.get(index).map(js_string).unwrap_or_default();
            (column_id_for_index(index), cell)
        })
        .collect()
}

/// Orders column-letter-keyed cell values into a positional row (A, B, C ...),
/// filling gaps with empty strings, for a RAW append.
fn row_from_column_values(values: &GridRow) -> Vec<Value> {
    let column_count = values
        .keys()
        .filter_map(|id| column_index_for_id(id))
        .map(|index| index + 1)
        .max()
        .unwrap_or(0);
    (0..column_count)
        .map(|index| {
            Value::String(
                values
                    .get(&column_id_for_index(index))
                    .cloned()
                    .unwrap_or_default(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// tableId parsing and sheet resolution
// ---------------------------------------------------------------------------

/// How a tableId names a tab within a spreadsheet. Parsing only extracts this
/// from the input; it never carries a host or endpoint (SSRF defense).
#[derive(Debug, Clone, PartialEq, Eq)]
enum SheetSelector {
    /// No tab named: use the spreadsheet's first sheet (current behavior).
    First,
    /// Numeric tab id (`gid`) from a URL fragment/query or `id:gid` form.
    Gid(i64),
    /// A sheet tab title from the `id:SheetName` form.
    Title(String),
}

/// A tableId decomposed into a spreadsheet id and a tab selector. The id is
/// validated to look like a Google document id so a URL/host can never leak
/// into it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTableId {
    spreadsheet_id: String,
    selector: SheetSelector,
}

impl ParsedTableId {
    /// Accepts a full Google Sheets URL, a bare spreadsheet id,
    /// `{spreadsheetId}:{gid}` (numeric), or `{spreadsheetId}:{SheetName}`.
    /// Only the spreadsheet id and the tab selector are extracted; the id is
    /// checked against [`is_plausible_spreadsheet_id`] so non-Google hosts,
    /// paths, or junk are rejected before any request is built.
    fn parse(table_id: &str) -> Result<Self, CoreError> {
        let trimmed = table_id.trim();
        if trimmed.is_empty() {
            return Err(invalid_table_id(table_id));
        }

        // A URL is anything with a scheme marker; parse it structurally rather
        // than string-splitting so only the id + gid are ever pulled out.
        if trimmed.contains("://") {
            return Self::from_url(trimmed).ok_or_else(|| invalid_table_id(table_id));
        }

        // `id:selector` - split on the FIRST separator so sheet titles that
        // themselves contain ':' stay intact.
        if let Some((raw_id, raw_selector)) = trimmed.split_once(TABLE_ID_SELECTOR_SEPARATOR) {
            let spreadsheet_id = raw_id.trim();
            if !is_plausible_spreadsheet_id(spreadsheet_id) {
                return Err(invalid_table_id(table_id));
            }
            let selector = selector_from_str(raw_selector.trim());
            return Ok(Self {
                spreadsheet_id: spreadsheet_id.to_string(),
                selector,
            });
        }

        // Bare id.
        if !is_plausible_spreadsheet_id(trimmed) {
            return Err(invalid_table_id(table_id));
        }
        Ok(Self {
            spreadsheet_id: trimmed.to_string(),
            selector: SheetSelector::First,
        })
    }

    /// Extracts the id from the `/d/{ID}/` segment and the gid from the `gid`
    /// query or fragment of a Google Sheets URL. The host is not trusted for
    /// routing (all requests go to the fixed endpoint), but a wrong-shaped or
    /// non-Google URL still fails here so junk cannot masquerade as an id.
    fn from_url(raw: &str) -> Option<Self> {
        let url = url::Url::parse(raw).ok()?;
        if !is_google_docs_host(url.host_str()?) {
            return None;
        }
        let path = url.path();
        let after_marker = path.split_once(SHEETS_URL_ID_MARKER)?.1;
        let spreadsheet_id = after_marker.split('/').next()?.trim();
        if !is_plausible_spreadsheet_id(spreadsheet_id) {
            return None;
        }
        let gid = gid_from_query(url.query()).or_else(|| gid_from_fragment(url.fragment()));
        Some(Self {
            spreadsheet_id: spreadsheet_id.to_string(),
            selector: gid.map(SheetSelector::Gid).unwrap_or(SheetSelector::First),
        })
    }
}

/// A spreadsheet id plus the concrete sheet-tab title to qualify every range
/// with. `None` title means the first sheet (no explicit prefix needed).
struct ResolvedSheet {
    spreadsheet_id: String,
    /// The spreadsheet's own title (for `describe_table.name`).
    spreadsheet_title: String,
    /// Resolved tab title, or `None` when reading the first sheet.
    sheet_title: Option<String>,
    /// Numeric tab id (the Google `gid`) of the resolved sheet. Needed to build
    /// a `GridRange` for cell-formatting requests; A1 value ranges use the
    /// title instead.
    sheet_id: i64,
}

impl ResolvedSheet {
    /// Parses the tableId, then fetches spreadsheet metadata to turn a gid or
    /// title selector into a concrete tab title (validating that it exists).
    /// The metadata call always targets the fixed Sheets endpoint for the
    /// connected account's token.
    fn resolve(token: &str, table_id: &str) -> Result<Self, CoreError> {
        let parsed = ParsedTableId::parse(table_id)?;
        let meta = fetch_spreadsheet_meta(token, &parsed.spreadsheet_id)?;
        let sheet_title = resolve_sheet_title(&parsed, &meta)?;
        let sheet_id = resolve_sheet_id(&parsed, &meta)?;
        Ok(Self {
            spreadsheet_id: parsed.spreadsheet_id,
            spreadsheet_title: meta.title,
            sheet_title,
            sheet_id,
        })
    }

    /// Qualifies an A1 range with the resolved sheet title when one was chosen,
    /// e.g. `A1:ZZ1` -> `'Sheet Name'!A1:ZZ1`. The first-sheet case keeps the
    /// bare range (matches the prior behavior and needs no title).
    fn range(&self, a1: &str) -> String {
        match &self.sheet_title {
            Some(title) => format!("{}!{a1}", quote_sheet_title(title)),
            None => a1.to_string(),
        }
    }

    /// Human-facing name for `describe_table`: the tab title when a specific
    /// tab was selected, otherwise the spreadsheet title.
    fn display_name(&self) -> String {
        self.sheet_title
            .clone()
            .unwrap_or_else(|| self.spreadsheet_title.clone())
    }
}

/// Minimal spreadsheet metadata: the spreadsheet title and its tabs.
struct SpreadsheetMeta {
    title: String,
    sheets: Vec<SheetProperties>,
}

struct SheetProperties {
    sheet_id: i64,
    title: String,
    /// Tab order, left to right (`sheets[].properties.index`).
    index: i64,
}

/// `GET {SHEETS_ENDPOINT}/{id}?fields=properties.title,sheets.properties(sheetId,title)`.
/// Fixed endpoint + token; the id only selects the resource.
fn fetch_spreadsheet_meta(token: &str, spreadsheet_id: &str) -> Result<SpreadsheetMeta, CoreError> {
    let mut url = sheets_base_url(spreadsheet_id)?;
    url.query_pairs_mut().append_pair(
        "fields",
        "properties.title,sheets.properties(sheetId,title,index)",
    );
    let body = google::get_json(token, url.as_str())?;
    let title = body["properties"]["title"]
        .as_str()
        .unwrap_or(spreadsheet_id)
        .to_string();
    let sheets = body["sheets"]
        .as_array()
        .map(|sheets| {
            sheets
                .iter()
                .filter_map(|sheet| {
                    let properties = &sheet["properties"];
                    Some(SheetProperties {
                        sheet_id: properties["sheetId"].as_i64()?,
                        title: properties["title"].as_str()?.to_string(),
                        // The first tab may omit index in the API response.
                        index: properties["index"].as_i64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SpreadsheetMeta { title, sheets })
}

/// Maps a parsed selector onto a concrete tab title using the metadata:
/// - `First` -> `None` (bare range, first sheet).
/// - `Gid` -> the tab whose sheetId equals the gid, else NotFound.
/// - `Title` -> the exact tab title if it exists, else NotFound.
fn resolve_sheet_title(
    parsed: &ParsedTableId,
    meta: &SpreadsheetMeta,
) -> Result<Option<String>, CoreError> {
    match &parsed.selector {
        SheetSelector::First => Ok(None),
        SheetSelector::Gid(gid) => meta
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == *gid)
            .map(|sheet| Some(sheet.title.clone()))
            .ok_or_else(|| {
                CoreError::NotFound(format!(
                    "Spreadsheet {} has no tab with gid {gid}",
                    parsed.spreadsheet_id
                ))
            }),
        SheetSelector::Title(title) => meta
            .sheets
            .iter()
            .find(|sheet| sheet.title == *title)
            .map(|sheet| Some(sheet.title.clone()))
            .ok_or_else(|| {
                CoreError::NotFound(format!(
                    "Spreadsheet {} has no tab named '{title}'",
                    parsed.spreadsheet_id
                ))
            }),
    }
}

/// Resolves the numeric tab id (gid) the selector points at, using the same
/// rules as [`resolve_sheet_title`]: `First` is the lowest-index tab, `Gid` is
/// the tab with that id, and `Title` is the tab with that exact title.
fn resolve_sheet_id(parsed: &ParsedTableId, meta: &SpreadsheetMeta) -> Result<i64, CoreError> {
    match &parsed.selector {
        SheetSelector::First => meta
            .sheets
            .iter()
            .min_by_key(|sheet| sheet.index)
            .map(|sheet| sheet.sheet_id)
            .ok_or_else(|| {
                CoreError::NotFound(format!(
                    "Spreadsheet {} has no sheets",
                    parsed.spreadsheet_id
                ))
            }),
        SheetSelector::Gid(gid) => meta
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == *gid)
            .map(|sheet| sheet.sheet_id)
            .ok_or_else(|| {
                CoreError::NotFound(format!(
                    "Spreadsheet {} has no tab with gid {gid}",
                    parsed.spreadsheet_id
                ))
            }),
        SheetSelector::Title(title) => meta
            .sheets
            .iter()
            .find(|sheet| sheet.title == *title)
            .map(|sheet| sheet.sheet_id)
            .ok_or_else(|| {
                CoreError::NotFound(format!(
                    "Spreadsheet {} has no tab named '{title}'",
                    parsed.spreadsheet_id
                ))
            }),
    }
}

/// A selector string is a gid when it is all digits, otherwise a sheet title.
fn selector_from_str(raw: &str) -> SheetSelector {
    if raw.is_empty() {
        return SheetSelector::First;
    }
    match raw.parse::<i64>() {
        Ok(gid) => SheetSelector::Gid(gid),
        Err(_) => SheetSelector::Title(raw.to_string()),
    }
}

/// Quotes a sheet title for an A1 range. Titles are wrapped in single quotes
/// (required when they contain spaces or punctuation), and any embedded single
/// quote is doubled per the Sheets A1 grammar.
fn quote_sheet_title(title: &str) -> String {
    format!("'{}'", title.replace('\'', "''"))
}

/// Plausible Google document id: URL-safe base64 alphabet (`A-Za-z0-9_-`) and
/// long enough to be a real id. This is the SSRF/junk guard - a hostname, path,
/// or arbitrary string fails it, so a tableId can never smuggle a URL host into
/// the fixed endpoint.
fn is_plausible_spreadsheet_id(candidate: &str) -> bool {
    candidate.len() >= MIN_SPREADSHEET_ID_LEN
        && candidate.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

/// Accepts only Google's document hosts for URL parsing.
fn is_google_docs_host(host: &str) -> bool {
    host == "docs.google.com" || host == "drive.google.com"
}

/// `gid=123` from a query string, if present and numeric.
fn gid_from_query(query: Option<&str>) -> Option<i64> {
    gid_from_pairs(query?)
}

/// `#gid=123` (or `#...&gid=123`) from a URL fragment, if present and numeric.
fn gid_from_fragment(fragment: Option<&str>) -> Option<i64> {
    gid_from_pairs(fragment?)
}

/// Scans `key=value&key=value` pairs for a numeric `gid`.
fn gid_from_pairs(pairs: &str) -> Option<i64> {
    pairs
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == SHEETS_URL_GID_KEY)
        .and_then(|(_, value)| value.parse::<i64>().ok())
}

fn invalid_table_id(table_id: &str) -> CoreError {
    CoreError::InvalidInput(format!(
        "'{table_id}' is not a Google Sheets URL, spreadsheet id, or spreadsheetId:tab selector"
    ))
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested without network access)
// ---------------------------------------------------------------------------

/// `values.get` for an A1 range that may be sheet-qualified (e.g.
/// `'My Sheet'!A1:ZZ1`). The range becomes a single path segment; the `url`
/// crate percent-encodes it so spaces, quotes, and the `!` separator survive.
fn fetch_values(
    token: &str,
    spreadsheet_id: &str,
    range: &str,
) -> Result<Vec<Vec<Value>>, CoreError> {
    let url = values_get_url(spreadsheet_id, range)?;
    let body = google::get_json(token, url.as_str())?;
    let rows = body["values"].as_array().cloned().unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(|row| row.as_array().cloned().unwrap_or_default())
        .collect())
}

/// `{SHEETS_ENDPOINT}/{id}/values/{range}` with the id and range pushed as
/// path segments so both are percent-encoded. The host and base path stay
/// fixed - the id/range only choose the resource, never the endpoint.
fn values_get_url(spreadsheet_id: &str, range: &str) -> Result<url::Url, CoreError> {
    let mut url = sheets_base_url(spreadsheet_id)?;
    push_segments(&mut url, &["values", range])?;
    Ok(url)
}

/// Base spreadsheet URL (`{SHEETS_ENDPOINT}/{id}`) with the id percent-encoded
/// as a path segment. Parsing the constant endpoint (never the caller's input)
/// guarantees the host is always Google's Sheets API.
fn sheets_base_url(spreadsheet_id: &str) -> Result<url::Url, CoreError> {
    let mut url = url::Url::parse(SHEETS_ENDPOINT)
        .map_err(|error| CoreError::Storage(format!("Could not build the Sheets URL: {error}")))?;
    push_segments(&mut url, &[spreadsheet_id])?;
    Ok(url)
}

fn push_segments(url: &mut url::Url, segments: &[&str]) -> Result<(), CoreError> {
    url.path_segments_mut()
        .map_err(|_| CoreError::Storage("Sheets endpoint cannot be a base URL".to_string()))?
        .extend(segments);
    Ok(())
}

/// `values/{range}:append` URL with the RAW input and insert-rows options.
/// The `:append` verb is part of the final path segment, so it is pushed
/// together with the range (both percent-encoded as one segment).
fn values_append_url(spreadsheet_id: &str, range: &str) -> Result<url::Url, CoreError> {
    let mut url = sheets_base_url(spreadsheet_id)?;
    push_segments(&mut url, &["values", &format!("{range}:append")])?;
    url.query_pairs_mut()
        .append_pair("valueInputOption", VALUE_INPUT_RAW)
        .append_pair("insertDataOption", "INSERT_ROWS");
    Ok(url)
}

/// `values:batchUpdate` URL. The `:batchUpdate` verb is the last path segment.
fn values_batch_update_url(spreadsheet_id: &str) -> Result<url::Url, CoreError> {
    let mut url = sheets_base_url(spreadsheet_id)?;
    push_segments(&mut url, &["values:batchUpdate"])?;
    Ok(url)
}

/// Header row -> FieldSchema list; types inferred from the first data row.
fn schema_from_rows(header: &[Value], first_data_row: Option<&[Value]>) -> Vec<FieldSchema> {
    header
        .iter()
        .enumerate()
        .map(|(index, cell)| FieldSchema {
            name: js_string(cell),
            field_type: infer_field_type(first_data_row.and_then(|row| row.get(index))).to_string(),
            required: None,
            readonly: Some(false),
            enum_values: None,
        })
        .collect()
}

/// number / boolean / string from the first data row's cell; formatted
/// values arrive as strings, so numeric- and boolean-looking text counts.
fn infer_field_type(sample: Option<&Value>) -> &'static str {
    match sample {
        Some(Value::Number(_)) => "number",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                "string"
            } else if trimmed.parse::<f64>().is_ok() {
                "number"
            } else if trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false")
            {
                "boolean"
            } else {
                "string"
            }
        }
        _ => "string",
    }
}

/// Rows -> records with ids "row_{sheetRowNumber}". Every fetched row keeps
/// its record slot (even blank ones) so ids always match sheet rows; cells
/// missing from short rows become null.
fn records_from_rows(
    header: &[String],
    rows: &[Vec<Value>],
    first_row_number: i64,
) -> Vec<TableRecord> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let fields: JsonMap = header
                .iter()
                .enumerate()
                .map(|(column, name)| {
                    (
                        name.clone(),
                        row.get(column).cloned().unwrap_or(Value::Null),
                    )
                })
                .collect();
            TableRecord {
                id: record_id_for(first_row_number + index as i64),
                fields,
            }
        })
        .collect()
}

fn record_id_for(row_number: i64) -> String {
    format!("{RECORD_ID_PREFIX}{row_number}")
}

/// "row_7" -> Some(7); rejects the header row and anything non-numeric.
fn parse_row_number(record_id: &str) -> Option<i64> {
    let number = record_id
        .strip_prefix(RECORD_ID_PREFIX)?
        .parse::<i64>()
        .ok()?;
    (number >= FIRST_DATA_ROW).then_some(number)
}

/// "Sheet1!A5:E6" (or "'My Sheet'!B2") -> starting row number 5 (or 2).
fn parse_start_row_from_range(range: &str) -> Option<i64> {
    let cell_part = range.rsplit('!').next().unwrap_or(range);
    let start_cell = cell_part.split(':').next()?;
    let digits: String = start_cell
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Header field names for an empty sheet: the union of the records' field keys
/// in first-seen order (serde_json's `preserve_order` keeps each record's JSON
/// key order, so the columns follow the order the caller sent).
fn header_from_records(records: &[JsonMap]) -> Vec<String> {
    let mut header = Vec::new();
    for record in records {
        for key in record.keys() {
            if !header.iter().any(|seen| seen == key) {
                header.push(key.clone());
            }
        }
    }
    header
}

/// The `values` rows for an append: the record rows ordered by the header, with
/// the header itself prepended as row 1 when we are seeding an empty sheet.
fn append_values(header: &[String], records: &[JsonMap], seed_header: bool) -> Vec<Vec<Value>> {
    let mut values = Vec::with_capacity(records.len() + usize::from(seed_header));
    if seed_header {
        values.push(header.iter().map(|name| json!(name)).collect());
    }
    values.extend(records.iter().map(|fields| row_values(header, fields)));
    values
}

/// One sheet row ordered by the header; absent and null fields write as empty
/// strings so RAW updates clear cells instead of skipping them.
fn row_values(header: &[String], fields: &JsonMap) -> Vec<Value> {
    header
        .iter()
        .map(|name| match fields.get(name) {
            None | Some(Value::Null) => Value::String(String::new()),
            Some(value) => value.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cell formatting (spreadsheets.batchUpdate) and style reads (spreadsheets.get
// with includeGridData). Request building is pure and unit-tested; only
// apply_format_plan / read_table_style_for touch the network.
// ---------------------------------------------------------------------------

/// Neutral grey used for the border lines a formatting plan draws.
const DEFAULT_BORDER_COLOR: &str = "#bfbfbf";

/// Field mask for a style read: the two sample rows plus sheet freeze counts
/// and per-column pixel widths. Bounds the response to exactly what
/// [`TableStyle`] reports.
const STYLE_FIELDS_MASK: &str = "sheets(properties(sheetId,title,gridProperties(frozenRowCount,frozenColumnCount)),data(rowData(values(formattedValue,effectiveFormat(backgroundColor,horizontalAlignment,wrapStrategy,numberFormat,textFormat(bold,italic,fontSize,foregroundColor)))),columnMetadata(pixelSize)))";

/// POSTs a formatting plan as a single `spreadsheets.batchUpdate`. An empty
/// plan (no requests) is a no-op; plans are validated non-empty upstream.
fn apply_format_plan(
    token: &str,
    sheet: &ResolvedSheet,
    plan: &FormatPlan,
) -> Result<(), CoreError> {
    let requests = build_format_requests(sheet.sheet_id, plan)?;
    if requests.is_empty() {
        return Ok(());
    }
    let url = spreadsheet_batch_update_url(&sheet.spreadsheet_id)?;
    google::post_json(token, url.as_str(), &json!({ "requests": requests }))?;
    Ok(())
}

/// Turns a plan into ordered `spreadsheets.batchUpdate` requests: per-range cell
/// formats and borders, then the header freeze, then column widths. Pure so the
/// request shape can be unit-tested without a network call.
fn build_format_requests(sheet_id: i64, plan: &FormatPlan) -> Result<Vec<Value>, CoreError> {
    let mut requests = Vec::new();
    for format in &plan.formats {
        if let Some(request) = repeat_cell_request(sheet_id, format)? {
            requests.push(request);
        }
        if let Some(request) = border_request(sheet_id, format)? {
            requests.push(request);
        }
    }
    if let Some(request) = freeze_request(sheet_id, plan.freeze_rows, plan.freeze_columns) {
        requests.push(request);
    }
    requests.extend(column_width_requests(sheet_id, &plan.column_widths)?);
    Ok(requests)
}

/// A `repeatCell` request writing only the properties present in `format`; the
/// field mask names exactly those paths so everything else is left untouched.
/// Returns `None` when the op sets no cell-level format (e.g. border only).
fn repeat_cell_request(sheet_id: i64, format: &CellFormat) -> Result<Option<Value>, CoreError> {
    let mut user_format = serde_json::Map::new();
    let mut text_format = serde_json::Map::new();
    let mut fields: Vec<&str> = Vec::new();

    if let Some(bold) = format.bold {
        text_format.insert("bold".to_string(), json!(bold));
        fields.push("userEnteredFormat.textFormat.bold");
    }
    if let Some(italic) = format.italic {
        text_format.insert("italic".to_string(), json!(italic));
        fields.push("userEnteredFormat.textFormat.italic");
    }
    if let Some(size) = format.font_size {
        text_format.insert("fontSize".to_string(), json!(size));
        fields.push("userEnteredFormat.textFormat.fontSize");
    }
    if let Some(color) = &format.font_color {
        text_format.insert("foregroundColor".to_string(), hex_to_color_json(color)?);
        fields.push("userEnteredFormat.textFormat.foregroundColor");
    }
    if !text_format.is_empty() {
        user_format.insert("textFormat".to_string(), Value::Object(text_format));
    }
    if let Some(color) = &format.background_color {
        user_format.insert("backgroundColor".to_string(), hex_to_color_json(color)?);
        fields.push("userEnteredFormat.backgroundColor");
    }
    if let Some(align) = format.horizontal_alignment {
        user_format.insert("horizontalAlignment".to_string(), json!(align.as_str()));
        fields.push("userEnteredFormat.horizontalAlignment");
    }
    if let Some(pattern) = &format.number_format {
        let format_type = number_format_type_str(pattern, format.number_format_type);
        user_format.insert(
            "numberFormat".to_string(),
            json!({ "type": format_type, "pattern": pattern }),
        );
        fields.push("userEnteredFormat.numberFormat");
    }
    if let Some(wrap) = format.wrap {
        let strategy = if wrap { "WRAP" } else { "OVERFLOW_CELL" };
        user_format.insert("wrapStrategy".to_string(), json!(strategy));
        fields.push("userEnteredFormat.wrapStrategy");
    }

    if fields.is_empty() {
        return Ok(None);
    }
    let range = parse_a1_range(&format.range)?;
    Ok(Some(json!({
        "repeatCell": {
            "range": grid_range_json(sheet_id, &range),
            "cell": { "userEnteredFormat": Value::Object(user_format) },
            "fields": fields.join(","),
        }
    })))
}

/// An `updateBorders` request for the op's [`BorderStyle`], or `None` when the
/// op sets no border. `Bottom` draws only a bottom rule (header underline);
/// `None` clears every side.
fn border_request(sheet_id: i64, format: &CellFormat) -> Result<Option<Value>, CoreError> {
    let Some(border) = format.border else {
        return Ok(None);
    };
    let range = parse_a1_range(&format.range)?;
    let (sides, style): (&[&str], &str) = match border {
        BorderStyle::None => (
            &[
                "top",
                "bottom",
                "left",
                "right",
                "innerHorizontal",
                "innerVertical",
            ],
            "NONE",
        ),
        BorderStyle::All => (
            &[
                "top",
                "bottom",
                "left",
                "right",
                "innerHorizontal",
                "innerVertical",
            ],
            "SOLID",
        ),
        BorderStyle::Outer => (&["top", "bottom", "left", "right"], "SOLID"),
        BorderStyle::Bottom => (&["bottom"], "SOLID"),
    };
    let border_obj = if style == "NONE" {
        json!({ "style": "NONE" })
    } else {
        json!({ "style": style, "color": hex_to_color_json(DEFAULT_BORDER_COLOR)? })
    };
    let mut request = serde_json::Map::new();
    request.insert("range".to_string(), grid_range_json(sheet_id, &range));
    for side in sides {
        request.insert((*side).to_string(), border_obj.clone());
    }
    Ok(Some(json!({ "updateBorders": Value::Object(request) })))
}

/// An `updateSheetProperties` request setting the header freeze; `None` when the
/// plan freezes nothing.
fn freeze_request(
    sheet_id: i64,
    freeze_rows: Option<i64>,
    freeze_columns: Option<i64>,
) -> Option<Value> {
    if freeze_rows.is_none() && freeze_columns.is_none() {
        return None;
    }
    let mut grid_properties = serde_json::Map::new();
    let mut fields: Vec<&str> = Vec::new();
    if let Some(rows) = freeze_rows {
        grid_properties.insert("frozenRowCount".to_string(), json!(rows));
        fields.push("gridProperties.frozenRowCount");
    }
    if let Some(columns) = freeze_columns {
        grid_properties.insert("frozenColumnCount".to_string(), json!(columns));
        fields.push("gridProperties.frozenColumnCount");
    }
    Some(json!({
        "updateSheetProperties": {
            "properties": {
                "sheetId": sheet_id,
                "gridProperties": Value::Object(grid_properties),
            },
            "fields": fields.join(","),
        }
    }))
}

/// One `updateDimensionProperties` request per column-width override.
fn column_width_requests(sheet_id: i64, widths: &[ColumnWidth]) -> Result<Vec<Value>, CoreError> {
    widths
        .iter()
        .map(|width| {
            let index = column_index_for_id(&width.column.to_ascii_uppercase())
                .filter(|index| *index < last_column_count())
                .ok_or_else(|| {
                    CoreError::InvalidInput(format!("Unknown column {}", width.column))
                })?;
            Ok(json!({
                "updateDimensionProperties": {
                    "range": {
                        "sheetId": sheet_id,
                        "dimension": "COLUMNS",
                        "startIndex": index,
                        "endIndex": index + 1,
                    },
                    "properties": { "pixelSize": width.pixels },
                    "fields": "pixelSize",
                }
            }))
        })
        .collect()
}

/// A `GridRange` JSON object; unbounded dimensions omit their start/end keys so
/// a whole-column or whole-row range is expressed correctly.
fn grid_range_json(sheet_id: i64, range: &A1Range) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("sheetId".to_string(), json!(sheet_id));
    if let Some(value) = range.start_row {
        object.insert("startRowIndex".to_string(), json!(value));
    }
    if let Some(value) = range.end_row {
        object.insert("endRowIndex".to_string(), json!(value));
    }
    if let Some(value) = range.start_col {
        object.insert("startColumnIndex".to_string(), json!(value));
    }
    if let Some(value) = range.end_col {
        object.insert("endColumnIndex".to_string(), json!(value));
    }
    Value::Object(object)
}

/// The `numberFormat.type` for a pattern: the explicit override when given,
/// otherwise inferred (a year or day token implies a date, else NUMBER).
fn number_format_type_str(pattern: &str, explicit: Option<NumberFormatType>) -> &'static str {
    if let Some(kind) = explicit {
        return kind.as_str();
    }
    let lower = pattern.to_ascii_lowercase();
    if lower.contains('y') || lower.contains('d') {
        NumberFormatType::Date.as_str()
    } else {
        NumberFormatType::Number.as_str()
    }
}

/// `#rrggbb` -> a Google API color object with 0..1 float components. Errors on
/// a malformed hex string (the tool boundary validates first, so this is a
/// defensive check).
fn hex_to_color_json(hex: &str) -> Result<Value, CoreError> {
    let (red, green, blue) = parse_hex_rgb(hex)
        .ok_or_else(|| CoreError::InvalidInput(format!("'{hex}' is not a #rrggbb color")))?;
    Ok(json!({
        "red": f64::from(red) / 255.0,
        "green": f64::from(green) / 255.0,
        "blue": f64::from(blue) / 255.0,
    }))
}

/// Parses `#rrggbb` (case-insensitive) into RGB bytes; `None` for any other
/// shape.
fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let red = u8::from_str_radix(&digits[0..2], 16).ok()?;
    let green = u8::from_str_radix(&digits[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&digits[4..6], 16).ok()?;
    Some((red, green, blue))
}

/// `{SHEETS_ENDPOINT}/{id}:batchUpdate`; the `:batchUpdate` verb rides the id
/// path segment, matching how the values URLs attach their verbs.
fn spreadsheet_batch_update_url(spreadsheet_id: &str) -> Result<url::Url, CoreError> {
    let mut url = url::Url::parse(SHEETS_ENDPOINT)
        .map_err(|error| CoreError::Storage(format!("Could not build the Sheets URL: {error}")))?;
    push_segments(&mut url, &[&format!("{spreadsheet_id}:batchUpdate")])?;
    Ok(url)
}

/// Reads the effective style of the tab via one `spreadsheets.get` with
/// `includeGridData` over the header and first data row.
fn read_table_style_for(token: &str, sheet: &ResolvedSheet) -> Result<TableStyle, CoreError> {
    let mut url = sheets_base_url(&sheet.spreadsheet_id)?;
    url.query_pairs_mut()
        .append_pair(
            "ranges",
            &sheet.range(&format!("A{HEADER_ROW}:{LAST_COLUMN}{FIRST_DATA_ROW}")),
        )
        .append_pair("includeGridData", "true")
        .append_pair("fields", STYLE_FIELDS_MASK);
    let body = google::get_json(token, url.as_str())?;

    let properties = &body["sheets"][0]["properties"];
    let grid = &properties["gridProperties"];
    let frozen_row_count = grid["frozenRowCount"].as_i64().unwrap_or(0);
    let frozen_column_count = grid["frozenColumnCount"].as_i64().unwrap_or(0);

    let data = &body["sheets"][0]["data"][0];
    let row_data = data["rowData"].as_array().cloned().unwrap_or_default();
    let header_values = row_style_values(&row_data, 0);
    let sample_values = row_style_values(&row_data, 1);
    let used = used_style_columns(&header_values);
    Ok(TableStyle {
        spreadsheet_id: sheet.spreadsheet_id.clone(),
        sheet_title: sheet.sheet_title.clone(),
        frozen_row_count,
        frozen_column_count,
        column_count: used as i64,
        header: cell_styles(&header_values, used),
        sample: cell_styles(&sample_values, used),
        column_widths: style_column_widths(&data["columnMetadata"], used),
    })
}

/// The `values` array of one style row (empty when the row is absent).
fn row_style_values(row_data: &[Value], index: usize) -> Vec<Value> {
    row_data
        .get(index)
        .and_then(|row| row["values"].as_array().cloned())
        .unwrap_or_default()
}

/// Used width = the last header cell carrying a non-empty value, plus one,
/// bounded to the supported column window. Zero for an empty header.
fn used_style_columns(header_values: &[Value]) -> usize {
    header_values
        .iter()
        .rposition(|cell| {
            cell["formattedValue"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        })
        .map_or(0, |last| (last + 1).min(last_column_count()))
}

/// The effective style of each used cell in a row, keyed by column letter.
fn cell_styles(values: &[Value], used: usize) -> Vec<CellStyle> {
    let null = Value::Null;
    (0..used)
        .map(|index| {
            let effective = values
                .get(index)
                .map_or(&null, |cell| &cell["effectiveFormat"]);
            parse_cell_style(&column_id_for_index(index), effective)
        })
        .collect()
}

/// Maps a Google `effectiveFormat` onto a [`CellStyle`]; only properties that
/// are actually set (bold/italic true, a present color, a pattern, an explicit
/// alignment) are reported, so the output stays compact.
fn parse_cell_style(column: &str, effective_format: &Value) -> CellStyle {
    let text = &effective_format["textFormat"];
    CellStyle {
        column: column.to_string(),
        bold: text["bold"].as_bool().filter(|bold| *bold),
        italic: text["italic"].as_bool().filter(|italic| *italic),
        font_size: text["fontSize"].as_i64(),
        font_color: google_color_to_hex(&text["foregroundColor"]),
        background_color: google_color_to_hex(&effective_format["backgroundColor"]),
        horizontal_alignment: effective_format["horizontalAlignment"]
            .as_str()
            .map(str::to_string),
        number_format: effective_format["numberFormat"]["pattern"]
            .as_str()
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_string),
        wrap: effective_format["wrapStrategy"]
            .as_str()
            .map(|strategy| strategy == "WRAP"),
    }
}

/// A Google color object -> `#rrggbb`; `None` when no color object is present
/// (absent components default to 0).
fn google_color_to_hex(color: &Value) -> Option<String> {
    if !color.is_object() {
        return None;
    }
    let component = |key: &str| -> u8 {
        (color[key].as_f64().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        component("red"),
        component("green"),
        component("blue")
    ))
}

/// Per-column pixel widths for the used columns, from the style read's
/// `columnMetadata` (skipping columns with no reported width).
fn style_column_widths(column_metadata: &Value, used: usize) -> Vec<ColumnWidth> {
    let metadata = column_metadata.as_array().cloned().unwrap_or_default();
    (0..used)
        .filter_map(|index| {
            let pixels = metadata.get(index)?["pixelSize"].as_i64()?;
            Some(ColumnWidth {
                column: column_id_for_index(index),
                pixels,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HorizontalAlignment;
    use serde_json::json;

    fn header(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn schema_infers_types_from_the_first_data_row() {
        let header_row = vec![
            json!("Name"),
            json!("Seats"),
            json!("Active"),
            json!("Ratio"),
            json!("Notes"),
            json!("Empty"),
        ];
        let data_row = vec![
            json!("Aurora Labs"),
            json!(24),
            json!("TRUE"),
            json!("12.5"),
            json!("hello"),
        ];

        let fields = schema_from_rows(&header_row, Some(&data_row));

        let describe: Vec<(&str, &str)> = fields
            .iter()
            .map(|field| (field.name.as_str(), field.field_type.as_str()))
            .collect();
        assert_eq!(
            describe,
            [
                ("Name", "string"),
                ("Seats", "number"),
                ("Active", "boolean"),
                ("Ratio", "number"),
                ("Notes", "string"),
                ("Empty", "string"), // missing sample cell defaults to string
            ]
        );
        assert!(fields.iter().all(|field| field.readonly == Some(false)));
        assert!(fields.iter().all(|field| field.required.is_none()));
    }

    #[test]
    fn schema_without_a_data_row_defaults_everything_to_string() {
        let fields = schema_from_rows(&[json!("A"), json!("B")], None);
        assert!(fields.iter().all(|field| field.field_type == "string"));
    }

    #[test]
    fn records_map_sheet_rows_to_row_ids_with_offsets() {
        let names = header(&["Name", "Seats"]);
        let rows = vec![
            vec![json!("Aurora"), json!(24)],
            vec![json!("Basalt")], // short row: missing cell becomes null
        ];

        // Offset 3 -> data starts at sheet row 5 (2 + 3).
        let records = records_from_rows(&names, &rows, 5);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, "row_5");
        assert_eq!(records[1].id, "row_6");
        assert_eq!(records[0].fields.get("Name"), Some(&json!("Aurora")));
        assert_eq!(records[0].fields.get("Seats"), Some(&json!(24)));
        assert_eq!(records[1].fields.get("Seats"), Some(&Value::Null));
    }

    #[test]
    fn row_number_parsing_rejects_header_and_foreign_ids() {
        assert_eq!(parse_row_number("row_2"), Some(2));
        assert_eq!(parse_row_number("row_120"), Some(120));
        assert_eq!(
            parse_row_number("row_1"),
            None,
            "header row is not a record"
        );
        assert_eq!(parse_row_number("row_0"), None);
        assert_eq!(parse_row_number("rec_abc"), None);
        assert_eq!(parse_row_number("row_x"), None);
        assert_eq!(parse_row_number("row_"), None);
    }

    #[test]
    fn start_row_parses_from_updated_range_shapes() {
        assert_eq!(parse_start_row_from_range("Sheet1!A5:E6"), Some(5));
        assert_eq!(parse_start_row_from_range("'My Sheet'!B2:B2"), Some(2));
        assert_eq!(parse_start_row_from_range("A10:C12"), Some(10));
        assert_eq!(parse_start_row_from_range("Sheet1!AA103"), Some(103));
        assert_eq!(parse_start_row_from_range("garbage"), None);
    }

    #[test]
    fn row_values_follow_header_order_and_clear_missing_cells() {
        let names = header(&["Name", "Seats", "Active"]);
        let mut fields = JsonMap::new();
        fields.insert("Active".to_string(), json!(true));
        fields.insert("Name".to_string(), json!("Aurora"));
        fields.insert("Ignored".to_string(), json!("not in header"));
        fields.insert("Seats".to_string(), Value::Null);

        let values = row_values(&names, &fields);

        assert_eq!(values, vec![json!("Aurora"), json!(""), json!(true)]);
    }

    #[test]
    fn header_from_records_unions_keys_in_first_seen_order() {
        let mut first = JsonMap::new();
        first.insert("Name".to_string(), json!("Aurora"));
        first.insert("Seats".to_string(), json!(4));
        let mut second = JsonMap::new();
        second.insert("Name".to_string(), json!("Borealis"));
        second.insert("Active".to_string(), json!(true));

        let header = header_from_records(&[first, second]);

        assert_eq!(
            header,
            vec![
                "Name".to_string(),
                "Seats".to_string(),
                "Active".to_string()
            ],
            "keys union across records, each in first-seen order"
        );
    }

    #[test]
    fn header_from_records_is_empty_when_no_record_has_fields() {
        assert!(header_from_records(&[JsonMap::new(), JsonMap::new()]).is_empty());
    }

    #[test]
    fn append_values_prepends_the_header_row_when_seeding() {
        let names = header(&["Name", "Seats"]);
        let mut record = JsonMap::new();
        record.insert("Name".to_string(), json!("Aurora"));
        record.insert("Seats".to_string(), json!(4));

        let seeded = append_values(&names, std::slice::from_ref(&record), true);
        assert_eq!(
            seeded,
            vec![
                vec![json!("Name"), json!("Seats")],
                vec![json!("Aurora"), json!(4)],
            ],
            "row 1 is the header, the record follows"
        );

        let existing = append_values(&names, std::slice::from_ref(&record), false);
        assert_eq!(
            existing,
            vec![vec![json!("Aurora"), json!(4)]],
            "an existing header is not rewritten"
        );
    }

    #[test]
    fn raw_column_count_uses_widest_row_within_bounds() {
        assert_eq!(
            raw_column_count(&[]),
            MIN_GRID_COLUMNS,
            "empty sheet floors at the minimum"
        );
        let rows = vec![
            vec![json!("a")],
            vec![json!("a"), json!("b"), json!("c")],
            vec![json!("a"), json!("b")],
        ];
        assert_eq!(raw_column_count(&rows), 3, "the widest row sets the width");
        assert_eq!(last_column_count(), 702, "ZZ window is 702 columns");
    }

    #[test]
    fn raw_grid_row_keys_cells_by_column_letter_and_pads() {
        let row = vec![json!("Name"), json!(24), json!(true)];
        let grid = raw_grid_row(4, &row);
        assert_eq!(grid.get("A").map(String::as_str), Some("Name"));
        assert_eq!(
            grid.get("B").map(String::as_str),
            Some("24"),
            "numbers stringify"
        );
        assert_eq!(
            grid.get("C").map(String::as_str),
            Some("true"),
            "booleans stringify"
        );
        assert_eq!(
            grid.get("D").map(String::as_str),
            Some(""),
            "a short row pads with empty cells"
        );
    }

    #[test]
    fn grid_column_uses_the_letter_as_both_id_and_title() {
        let column = grid_column_for_index(0);
        assert_eq!(column.id, "A");
        assert_eq!(column.title, "A");
        assert_eq!(grid_column_for_index(26).id, "AA");
    }

    #[test]
    fn row_from_column_values_orders_by_letter_and_fills_gaps() {
        let mut values = GridRow::new();
        values.insert("C".to_string(), "third".to_string());
        values.insert("A".to_string(), "first".to_string());

        let row = row_from_column_values(&values);

        assert_eq!(row, vec![json!("first"), json!(""), json!("third")]);
        assert!(
            row_from_column_values(&GridRow::new()).is_empty(),
            "no values -> no cells"
        );
    }

    // A realistic 44-char Google document id used across the parser tests.
    const SAMPLE_ID: &str = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";

    #[test]
    fn parse_full_url_with_gid_in_fragment_and_query() {
        let parsed = ParsedTableId::parse(&format!(
            "https://docs.google.com/spreadsheets/d/{SAMPLE_ID}/edit?gid=1234567#gid=1234567"
        ))
        .expect("valid url");
        assert_eq!(parsed.spreadsheet_id, SAMPLE_ID);
        assert_eq!(parsed.selector, SheetSelector::Gid(1234567));
    }

    #[test]
    fn parse_url_gid_zero_from_fragment_only() {
        let parsed = ParsedTableId::parse(&format!(
            "https://docs.google.com/spreadsheets/d/{SAMPLE_ID}/edit#gid=0"
        ))
        .expect("valid url");
        assert_eq!(parsed.spreadsheet_id, SAMPLE_ID);
        assert_eq!(parsed.selector, SheetSelector::Gid(0));
    }

    #[test]
    fn parse_url_without_gid_selects_first_sheet() {
        let parsed = ParsedTableId::parse(&format!(
            "https://docs.google.com/spreadsheets/d/{SAMPLE_ID}/edit"
        ))
        .expect("valid url");
        assert_eq!(parsed.spreadsheet_id, SAMPLE_ID);
        assert_eq!(parsed.selector, SheetSelector::First);
    }

    #[test]
    fn parse_bare_id_selects_first_sheet() {
        let parsed = ParsedTableId::parse(SAMPLE_ID).expect("valid id");
        assert_eq!(parsed.spreadsheet_id, SAMPLE_ID);
        assert_eq!(parsed.selector, SheetSelector::First);
    }

    #[test]
    fn parse_id_colon_gid() {
        let parsed = ParsedTableId::parse(&format!("{SAMPLE_ID}:42")).expect("valid id:gid");
        assert_eq!(parsed.spreadsheet_id, SAMPLE_ID);
        assert_eq!(parsed.selector, SheetSelector::Gid(42));
    }

    #[test]
    fn parse_id_colon_sheet_name() {
        let parsed =
            ParsedTableId::parse(&format!("{SAMPLE_ID}:Q3 Summary")).expect("valid id:name");
        assert_eq!(parsed.spreadsheet_id, SAMPLE_ID);
        assert_eq!(
            parsed.selector,
            SheetSelector::Title("Q3 Summary".to_string())
        );
    }

    #[test]
    fn parse_id_colon_name_keeps_colons_in_title() {
        let parsed =
            ParsedTableId::parse(&format!("{SAMPLE_ID}:10:30 report")).expect("valid id:name");
        assert_eq!(
            parsed.selector,
            SheetSelector::Title("10:30 report".to_string())
        );
    }

    #[test]
    fn parse_rejects_junk_and_non_google_hosts() {
        // Non-Google host: never treated as a spreadsheet URL (SSRF guard).
        assert!(ParsedTableId::parse("https://evil.example.com/spreadsheets/d/xxx/edit").is_err());
        // A Google host but the id is too short to be plausible.
        assert!(ParsedTableId::parse("https://docs.google.com/spreadsheets/d/short/edit").is_err());
        // Arbitrary strings and hostnames.
        assert!(ParsedTableId::parse("not-a-real-id").is_err());
        assert!(ParsedTableId::parse("sheets.googleapis.com").is_err());
        assert!(ParsedTableId::parse("").is_err());
        assert!(ParsedTableId::parse("   ").is_err());
        // Short id with a selector is still rejected on the id.
        assert!(ParsedTableId::parse("short:0").is_err());
    }

    #[test]
    fn plausible_id_guard_rejects_url_characters() {
        assert!(is_plausible_spreadsheet_id(SAMPLE_ID));
        assert!(!is_plausible_spreadsheet_id("has/slash/aaaaaaaaaaaaaaaaa"));
        assert!(!is_plausible_spreadsheet_id("has spaces aaaaaaaaaaaaaaa"));
        assert!(!is_plausible_spreadsheet_id("docs.google.com"));
    }

    #[test]
    fn range_qualifies_with_quoted_sheet_title_only_when_selected() {
        let first = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: None,
            sheet_id: 0,
        };
        assert_eq!(first.range("A1:ZZ1"), "A1:ZZ1");

        let named = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("My Sheet".to_string()),
            sheet_id: 42,
        };
        assert_eq!(named.range("A2:ZZ"), "'My Sheet'!A2:ZZ");

        // Embedded single quotes are doubled per the A1 grammar.
        let quoted = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("Bob's Tab".to_string()),
            sheet_id: 7,
        };
        assert_eq!(quoted.range("A1"), "'Bob''s Tab'!A1");
    }

    #[test]
    fn resolve_sheet_title_maps_gid_and_title_or_errors() {
        let meta = SpreadsheetMeta {
            title: "Workbook".to_string(),
            sheets: vec![
                SheetProperties {
                    sheet_id: 0,
                    title: "Sheet1".to_string(),
                    index: 0,
                },
                SheetProperties {
                    sheet_id: 987,
                    title: "Data".to_string(),
                    index: 1,
                },
            ],
        };
        let gid = ParsedTableId {
            spreadsheet_id: SAMPLE_ID.to_string(),
            selector: SheetSelector::Gid(987),
        };
        assert_eq!(
            resolve_sheet_title(&gid, &meta).unwrap(),
            Some("Data".to_string())
        );

        let title = ParsedTableId {
            spreadsheet_id: SAMPLE_ID.to_string(),
            selector: SheetSelector::Title("Sheet1".to_string()),
        };
        assert_eq!(
            resolve_sheet_title(&title, &meta).unwrap(),
            Some("Sheet1".to_string())
        );

        let first = ParsedTableId {
            spreadsheet_id: SAMPLE_ID.to_string(),
            selector: SheetSelector::First,
        };
        assert_eq!(resolve_sheet_title(&first, &meta).unwrap(), None);

        let missing_gid = ParsedTableId {
            spreadsheet_id: SAMPLE_ID.to_string(),
            selector: SheetSelector::Gid(555),
        };
        assert!(matches!(
            resolve_sheet_title(&missing_gid, &meta),
            Err(CoreError::NotFound(_))
        ));

        let missing_title = ParsedTableId {
            spreadsheet_id: SAMPLE_ID.to_string(),
            selector: SheetSelector::Title("Nope".to_string()),
        };
        assert!(matches!(
            resolve_sheet_title(&missing_title, &meta),
            Err(CoreError::NotFound(_))
        ));
    }

    #[test]
    fn url_builders_target_the_fixed_sheets_endpoint_with_encoded_range() {
        let get = values_get_url(SAMPLE_ID, "'My Sheet'!A1:ZZ1").expect("get url");
        assert_eq!(get.host_str(), Some("sheets.googleapis.com"));
        assert!(get.as_str().starts_with(SHEETS_ENDPOINT));
        assert!(get.as_str().contains(SAMPLE_ID));
        // Spaces in the range are percent-encoded so they cannot break the URL
        // path (`!`, `'`, and `:` are valid path characters the Sheets API
        // accepts unencoded, so they may remain literal).
        assert!(!get.path().contains(' '));
        assert!(get.path().contains("%20"));

        let append = values_append_url(SAMPLE_ID, "'My Sheet'!A1").expect("append url");
        assert_eq!(append.host_str(), Some("sheets.googleapis.com"));
        assert!(append.as_str().contains("valueInputOption=RAW"));
        assert!(append.as_str().contains("insertDataOption=INSERT_ROWS"));

        let batch = values_batch_update_url(SAMPLE_ID).expect("batch url");
        assert_eq!(batch.host_str(), Some("sheets.googleapis.com"));
        assert!(batch.as_str().ends_with("values:batchUpdate"));
    }

    fn cell_format(range: &str) -> CellFormat {
        CellFormat {
            range: range.to_string(),
            bold: None,
            italic: None,
            font_size: None,
            font_color: None,
            background_color: None,
            horizontal_alignment: None,
            number_format: None,
            number_format_type: None,
            wrap: None,
            border: None,
        }
    }

    #[test]
    fn hex_to_color_json_maps_channels_to_unit_floats() {
        let color = hex_to_color_json("#ff8000").expect("valid hex");
        assert_eq!(color["red"].as_f64(), Some(1.0));
        assert!((color["green"].as_f64().unwrap() - 128.0 / 255.0).abs() < 1e-9);
        assert_eq!(color["blue"].as_f64(), Some(0.0));
        assert!(
            hex_to_color_json("ff8000").is_err(),
            "missing # is rejected"
        );
        assert!(hex_to_color_json("#fff").is_err(), "shorthand is rejected");
        assert!(hex_to_color_json("#gggggg").is_err(), "non-hex is rejected");
    }

    #[test]
    fn number_format_type_infers_date_from_pattern_or_uses_override() {
        assert_eq!(number_format_type_str("#,##0", None), "NUMBER");
        assert_eq!(number_format_type_str("0.00%", None), "NUMBER");
        assert_eq!(number_format_type_str("yyyy-mm-dd", None), "DATE");
        assert_eq!(
            number_format_type_str("#,##0", Some(NumberFormatType::Currency)),
            "CURRENCY"
        );
    }

    #[test]
    fn grid_range_json_omits_unbounded_dimensions() {
        let cell = grid_range_json(7, &parse_a1_range("B2:C3").expect("range"));
        assert_eq!(cell["sheetId"].as_i64(), Some(7));
        assert_eq!(cell["startRowIndex"].as_i64(), Some(1));
        assert_eq!(cell["endRowIndex"].as_i64(), Some(3));
        assert_eq!(cell["startColumnIndex"].as_i64(), Some(1));
        assert_eq!(cell["endColumnIndex"].as_i64(), Some(3));

        let whole_columns = grid_range_json(7, &parse_a1_range("A:B").expect("range"));
        assert_eq!(whole_columns["startColumnIndex"].as_i64(), Some(0));
        assert_eq!(whole_columns["endColumnIndex"].as_i64(), Some(2));
        assert!(
            whole_columns.get("startRowIndex").is_none(),
            "whole-column range leaves rows unbounded"
        );
    }

    #[test]
    fn repeat_cell_request_masks_only_the_set_properties() {
        let format = CellFormat {
            bold: Some(true),
            background_color: Some("#f3f4f6".to_string()),
            horizontal_alignment: Some(HorizontalAlignment::Center),
            ..cell_format("A1:D1")
        };
        let request = repeat_cell_request(3, &format)
            .expect("request")
            .expect("some");
        let repeat = &request["repeatCell"];
        let fields = repeat["fields"].as_str().expect("fields");
        assert!(fields.contains("userEnteredFormat.textFormat.bold"));
        assert!(fields.contains("userEnteredFormat.backgroundColor"));
        assert!(fields.contains("userEnteredFormat.horizontalAlignment"));
        assert!(
            !fields.contains("italic"),
            "unset properties stay out of the mask"
        );
        let user_format = &repeat["cell"]["userEnteredFormat"];
        assert_eq!(user_format["textFormat"]["bold"].as_bool(), Some(true));
        assert_eq!(user_format["horizontalAlignment"].as_str(), Some("CENTER"));
    }

    #[test]
    fn repeat_cell_request_is_none_when_only_a_border_is_set() {
        let format = CellFormat {
            border: Some(BorderStyle::Bottom),
            ..cell_format("A1:D1")
        };
        assert!(repeat_cell_request(1, &format).expect("ok").is_none());
    }

    #[test]
    fn border_request_bottom_draws_only_a_bottom_rule() {
        let format = CellFormat {
            border: Some(BorderStyle::Bottom),
            ..cell_format("A1:D1")
        };
        let request = border_request(1, &format).expect("ok").expect("some");
        let borders = &request["updateBorders"];
        assert_eq!(borders["bottom"]["style"].as_str(), Some("SOLID"));
        assert!(borders.get("top").is_none(), "only the bottom side is set");
        assert!(border_request(1, &cell_format("A1")).expect("ok").is_none());
    }

    #[test]
    fn freeze_request_combines_row_and_column_fields() {
        let request = freeze_request(5, Some(1), Some(2)).expect("some");
        let update = &request["updateSheetProperties"];
        assert_eq!(update["properties"]["sheetId"].as_i64(), Some(5));
        assert_eq!(
            update["properties"]["gridProperties"]["frozenRowCount"].as_i64(),
            Some(1)
        );
        let fields = update["fields"].as_str().expect("fields");
        assert!(fields.contains("gridProperties.frozenRowCount"));
        assert!(fields.contains("gridProperties.frozenColumnCount"));
        assert!(freeze_request(5, None, None).is_none());
    }

    #[test]
    fn column_width_requests_map_letters_to_dimension_ranges() {
        let widths = vec![ColumnWidth {
            column: "C".to_string(),
            pixels: 160,
        }];
        let requests = column_width_requests(9, &widths).expect("requests");
        let range = &requests[0]["updateDimensionProperties"]["range"];
        assert_eq!(range["dimension"].as_str(), Some("COLUMNS"));
        assert_eq!(range["startIndex"].as_i64(), Some(2));
        assert_eq!(range["endIndex"].as_i64(), Some(3));
        assert_eq!(
            requests[0]["updateDimensionProperties"]["properties"]["pixelSize"].as_i64(),
            Some(160)
        );

        let bad = column_width_requests(
            9,
            &[ColumnWidth {
                column: "not-a-column".to_string(),
                pixels: 100,
            }],
        );
        assert!(bad.is_err(), "an unknown column is rejected");
    }

    #[test]
    fn build_format_requests_orders_cells_then_freeze_then_widths() {
        let plan = FormatPlan {
            formats: vec![CellFormat {
                bold: Some(true),
                border: Some(BorderStyle::Bottom),
                ..cell_format("A1:D1")
            }],
            freeze_rows: Some(1),
            freeze_columns: None,
            column_widths: vec![ColumnWidth {
                column: "A".to_string(),
                pixels: 200,
            }],
        };
        let requests = build_format_requests(0, &plan).expect("requests");
        assert!(requests[0].get("repeatCell").is_some());
        assert!(requests[1].get("updateBorders").is_some());
        assert!(requests[2].get("updateSheetProperties").is_some());
        assert!(requests[3].get("updateDimensionProperties").is_some());

        let empty = build_format_requests(
            0,
            &FormatPlan {
                formats: Vec::new(),
                freeze_rows: None,
                freeze_columns: None,
                column_widths: Vec::new(),
            },
        )
        .expect("requests");
        assert!(empty.is_empty(), "an empty plan produces no requests");
    }

    #[test]
    fn spreadsheet_batch_update_url_targets_the_fixed_endpoint() {
        let url = spreadsheet_batch_update_url(SAMPLE_ID).expect("url");
        assert_eq!(url.host_str(), Some("sheets.googleapis.com"));
        assert!(url.as_str().starts_with(SHEETS_ENDPOINT));
        assert!(url.as_str().ends_with(&format!("{SAMPLE_ID}:batchUpdate")));
    }

    #[test]
    fn parse_cell_style_reports_only_set_properties() {
        let effective = json!({
            "backgroundColor": { "red": 1.0, "green": 1.0, "blue": 1.0 },
            "horizontalAlignment": "RIGHT",
            "numberFormat": { "type": "NUMBER", "pattern": "#,##0" },
            "wrapStrategy": "OVERFLOW_CELL",
            "textFormat": { "bold": true, "italic": false, "fontSize": 11 }
        });
        let style = parse_cell_style("B", &effective);
        assert_eq!(style.column, "B");
        assert_eq!(style.bold, Some(true));
        assert_eq!(style.italic, None, "italic false is omitted as noise");
        assert_eq!(style.font_size, Some(11));
        assert_eq!(style.background_color.as_deref(), Some("#ffffff"));
        assert_eq!(style.horizontal_alignment.as_deref(), Some("RIGHT"));
        assert_eq!(style.number_format.as_deref(), Some("#,##0"));
        assert_eq!(style.wrap, Some(false));

        // A missing effectiveFormat yields an all-empty style for that column.
        let empty = parse_cell_style("A", &Value::Null);
        assert_eq!(empty.bold, None);
        assert_eq!(empty.background_color, None);
    }

    #[test]
    fn used_style_columns_counts_to_the_last_nonempty_header_cell() {
        let header = vec![
            json!({ "formattedValue": "Name" }),
            json!({ "formattedValue": "Seats" }),
            json!({}),
        ];
        assert_eq!(used_style_columns(&header), 2);
        assert_eq!(used_style_columns(&[]), 0);
    }
}
