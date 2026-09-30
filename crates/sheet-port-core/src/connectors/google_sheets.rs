//! Real Google Sheets connector: Drive `files.list` for spreadsheet
//! discovery and the Sheets values API for reads and writes. The first row of
//! the first visible sheet is the header; records map 1:1 onto sheet rows
//! with ids "row_{n}" (n = 1-based sheet row, so data starts at row_2).
//! Tokens are obtained through the crate-private google module and never
//! leave this crate.

use std::collections::HashMap;

use rusqlite::Connection;
use serde_json::{json, Value};

use super::formats::{
    check_fetch_size, shape_format_grid, CellSample, FormatFields, FormatGrid, FormatSource,
    FormatsRequest,
};
use super::{
    clamp_read_window, column_id_for_index, column_index_for_id, grid_window, js_string,
    parse_a1_range, window_a1, A1Range, GridWindow, TableConnector,
};
use crate::constants::FIND_RECORDS_LIMIT;
use crate::error::CoreError;
use crate::google;
use crate::sources;
use crate::types::{
    BorderStyle, CellFormat, CellStyle, CellWrite, ColumnWidth, ConditionWhen, ConditionalFormat,
    CreatedResource, DataSource, DataValidation, FieldSchema, FormatPlan, GridColumn, GridData,
    GridRow, JsonMap, NumberFormatType, ReadOptions, RecordPatch, RowHeight, SheetTab, SourceKind,
    SpreadsheetInfo, TableRecord, TableRef, TableSchema, TableStyle, ValidationKind,
};

const DRIVE_FILES_ENDPOINT: &str = "https://www.googleapis.com/drive/v3/files";
const SHEETS_ENDPOINT: &str = "https://sheets.googleapis.com/v4/spreadsheets";
const SPREADSHEET_MIME_TYPE: &str = "application/vnd.google-apps.spreadsheet";
const DRIVE_PAGE_SIZE: &str = "100";

/// Column window for value ranges; ZZ = 702 columns, far beyond broker use.
const LAST_COLUMN: &str = "ZZ";
const HEADER_ROW: i64 = 1;
const FIRST_DATA_ROW: i64 = 2;
/// Highest 1-based header row get_table_style accepts (its sample row is the
/// next one); Google Sheets caps a tab at 10 million cells, so rows beyond this
/// are never a real header.
pub const STYLE_HEADER_ROW_MAX: i64 = 1_000_000;
/// First row of the RAW Workbench grid mirror: row 1 is real data, not a
/// header (the record/table view above still treats row 1 as the header).
const FIRST_SHEET_ROW: i64 = 1;
/// Smallest column count a raw grid reports, so a fully empty sheet still shows
/// a column A to type into.
const MIN_GRID_COLUMNS: usize = 1;
const RECORD_ID_PREFIX: &str = "row_";

/// Values API `valueInputOption`: write cell values exactly as sent.
const VALUE_INPUT_RAW: &str = "RAW";

/// Values API `valueRenderOption`: return each cell's raw formula (`=...`)
/// instead of its computed value (used by `read_formulas`).
const VALUE_RENDER_FORMULA: &str = "FORMULA";

/// Values API `valueInputOption` for coordinate-level cell writes: parse the
/// value as if the user typed it (numbers become numbers, `=` a formula) -
/// matching what someone editing the sheet by hand would get.
const VALUE_INPUT_USER_ENTERED: &str = "USER_ENTERED";

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
            locale: sheet.locale.clone(),
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

    /// Record-shaped read like [`read_table`](Self::read_table) but with the
    /// `FORMULA` render option, so formula cells return their raw `=...` text.
    fn read_formulas(
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
        let rows = fetch_formula_values(&token, &sheet.spreadsheet_id, &sheet.range(&range))?;
        Ok(records_from_rows(&header, &rows, start_row))
    }

    /// One `values.batchUpdate` with USER_ENTERED input and one data entry per
    /// cell, so any cell of the tab can be written regardless of the sheet's
    /// header layout.
    fn update_cells(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        cells: &[CellWrite],
    ) -> Result<(), CoreError> {
        if cells.is_empty() {
            return Ok(());
        }
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let url = values_batch_update_url(&sheet.spreadsheet_id)?;
        google::post_json(
            &token,
            url.as_str(),
            &json!({
                "valueInputOption": VALUE_INPUT_USER_ENTERED,
                "data": cell_update_entries(&sheet, cells),
            }),
        )?;
        Ok(())
    }

    /// `spreadsheets.create` with just a title; returns the new id and url.
    fn create_spreadsheet(
        &self,
        conn: &Connection,
        source_id: &str,
        title: &str,
    ) -> Result<CreatedResource, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let url = url::Url::parse(SHEETS_ENDPOINT).map_err(|error| {
            CoreError::Storage(format!("Could not build the Sheets URL: {error}"))
        })?;
        let body = google::post_json(
            &token,
            url.as_str(),
            &json!({ "properties": { "title": title } }),
        )?;
        let spreadsheet_id = body["spreadsheetId"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                CoreError::Storage("Sheets create response had no spreadsheetId".to_string())
            })?;
        Ok(CreatedResource {
            spreadsheet_id: Some(spreadsheet_id),
            sheet_gid: None,
            title: Some(title.to_string()),
            url: body["spreadsheetUrl"].as_str().map(str::to_string),
        })
    }

    /// `spreadsheets.batchUpdate` with an `addSheet` request; returns the new
    /// tab's gid. Only the spreadsheet id of `table_id` is used.
    fn create_sheet(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        title: &str,
    ) -> Result<CreatedResource, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let parsed = ParsedTableId::parse(table_id)?;
        let url = spreadsheet_batch_update_url(&parsed.spreadsheet_id)?;
        let body = google::post_json(
            &token,
            url.as_str(),
            &json!({ "requests": [{ "addSheet": { "properties": { "title": title } } }] }),
        )?;
        let gid = body["replies"][0]["addSheet"]["properties"]["sheetId"]
            .as_i64()
            .ok_or_else(|| {
                CoreError::Storage("Sheets addSheet response had no sheetId".to_string())
            })?;
        Ok(CreatedResource {
            spreadsheet_id: Some(parsed.spreadsheet_id),
            sheet_gid: Some(gid.to_string()),
            title: Some(title.to_string()),
            url: None,
        })
    }

    /// `spreadsheets.batchUpdate` with a `deleteSheet` request for the tab that
    /// `table_id` resolves to.
    fn delete_sheet(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
    ) -> Result<(), CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let url = spreadsheet_batch_update_url(&sheet.spreadsheet_id)?;
        google::post_json(
            &token,
            url.as_str(),
            &json!({ "requests": [{ "deleteSheet": { "sheetId": sheet.sheet_id } }] }),
        )?;
        Ok(())
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
        Ok(self.spreadsheet_info(conn, source_id, spreadsheet_id)?.tabs)
    }

    /// The tabs plus `properties.locale` / `properties.timeZone`, from the
    /// same single metadata read.
    fn spreadsheet_info(
        &self,
        conn: &Connection,
        source_id: &str,
        spreadsheet_id: &str,
    ) -> Result<SpreadsheetInfo, CoreError> {
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
        Ok(SpreadsheetInfo {
            tabs,
            locale: meta.locale,
            time_zone: meta.time_zone,
        })
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

    /// One A1 window of the tab. Only the window is requested from the Values
    /// API (which returns it anchored at the window's top-left cell and trims
    /// trailing empty rows), so a deep slice of a large tab stays cheap.
    fn read_grid_range(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        range: &A1Range,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<GridWindow, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        let rows: Vec<Vec<String>> = fetch_values(
            &token,
            &sheet.spreadsheet_id,
            &sheet.range(&window_a1(range)),
        )?
        .iter()
        .map(|row| row.iter().map(js_string).collect())
        .collect();
        Ok(grid_window(&rows, range, limit, offset))
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

    /// The effective style of the tab: header row, the row below it, sheet
    /// freeze counts, and column widths. One `spreadsheets.get` with
    /// `includeGridData` over those two rows (docs/mcp-tools.md
    /// "get_table_style").
    fn read_table_style(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        header_row: i64,
    ) -> Result<TableStyle, CoreError> {
        let range = style_range_a1(header_row)?;
        let token = google::access_token(conn, source_id)?;
        let sheet = ResolvedSheet::resolve(&token, table_id)?;
        read_table_style_for(&token, &sheet, &range, header_row)
    }

    /// Per-cell formats of one window via a single `spreadsheets.get` with
    /// `includeGridData` and a fields mask trimmed to the requested fields
    /// (docs/mcp-tools.md "read_formats"). The metadata read that resolves
    /// the tab also gives its grid size, so an oversized window is refused
    /// before the grid is fetched.
    fn read_formats(
        &self,
        conn: &Connection,
        source_id: &str,
        table_id: &str,
        request: &FormatsRequest,
    ) -> Result<FormatGrid, CoreError> {
        let token = google::access_token(conn, source_id)?;
        let parsed = ParsedTableId::parse(table_id)?;
        let meta = fetch_spreadsheet_meta(&token, &parsed.spreadsheet_id)?;
        let sheet_id = resolve_sheet_id(&parsed, &meta)?;
        let tab = meta
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .ok_or_else(|| {
                CoreError::NotFound(format!(
                    "Spreadsheet {} has no tab with gid {sheet_id}",
                    parsed.spreadsheet_id
                ))
            })?;
        check_fetch_size(
            request.range.as_ref(),
            tab.grid_rows,
            tab.grid_cols,
            request.max_cells,
        )?;
        let mut url = sheets_base_url(&parsed.spreadsheet_id)?;
        url.query_pairs_mut()
            .append_pair(
                "ranges",
                &formats_range_a1(&tab.title, request.range.as_ref()),
            )
            .append_pair("includeGridData", "true")
            .append_pair(
                "fields",
                &formats_fields_mask(&request.fields, request.source),
            );
        let body = google::get_json(&token, url.as_str())?;
        let cells = format_samples_from_body(&body, request);
        shape_format_grid(
            tab.title.clone(),
            request,
            Some(tab.grid_rows),
            Some(tab.grid_cols),
            cells,
        )
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
    /// Spreadsheet locale (`properties.locale`), e.g. `vi_VN`.
    locale: Option<String>,
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
            locale: meta.locale,
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

/// Minimal spreadsheet metadata: the spreadsheet title, locale settings, and
/// its tabs.
struct SpreadsheetMeta {
    title: String,
    locale: Option<String>,
    time_zone: Option<String>,
    sheets: Vec<SheetProperties>,
}

struct SheetProperties {
    sheet_id: i64,
    title: String,
    /// Tab order, left to right (`sheets[].properties.index`).
    index: i64,
    /// Grid size (`gridProperties.rowCount` / `columnCount`); 0 when absent.
    grid_rows: usize,
    grid_cols: usize,
}

/// `GET {SHEETS_ENDPOINT}/{id}?fields=properties(title,locale,timeZone),sheets.properties(sheetId,title,index,gridProperties(rowCount,columnCount))`.
/// Fixed endpoint + token; the id only selects the resource.
fn fetch_spreadsheet_meta(token: &str, spreadsheet_id: &str) -> Result<SpreadsheetMeta, CoreError> {
    let mut url = sheets_base_url(spreadsheet_id)?;
    url.query_pairs_mut().append_pair(
        "fields",
        "properties(title,locale,timeZone),sheets.properties(sheetId,title,index,gridProperties(rowCount,columnCount))",
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
                        grid_rows: grid_dimension(&properties["gridProperties"]["rowCount"]),
                        grid_cols: grid_dimension(&properties["gridProperties"]["columnCount"]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let text = |key: &str| {
        body["properties"][key]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    Ok(SpreadsheetMeta {
        title,
        locale: text("locale"),
        time_zone: text("timeZone"),
        sheets,
    })
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
    Ok(rows_from_values_body(&google::get_json(
        token,
        url.as_str(),
    )?))
}

/// Like [`fetch_values`] but with `valueRenderOption=FORMULA`, so a cell holding
/// a formula comes back as its raw `=...` string instead of the computed value.
fn fetch_formula_values(
    token: &str,
    spreadsheet_id: &str,
    range: &str,
) -> Result<Vec<Vec<Value>>, CoreError> {
    let mut url = values_get_url(spreadsheet_id, range)?;
    url.query_pairs_mut()
        .append_pair("valueRenderOption", VALUE_RENDER_FORMULA);
    Ok(rows_from_values_body(&google::get_json(
        token,
        url.as_str(),
    )?))
}

/// Extracts the `values` 2D array from a `spreadsheets.values.get` body,
/// defaulting missing rows/cells to empty.
fn rows_from_values_body(body: &Value) -> Vec<Vec<Value>> {
    body["values"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.as_array().cloned().unwrap_or_default())
        .collect()
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

/// The `data` entries of a cell-write `values.batchUpdate`: one single-cell
/// range per write, scoped to the resolved tab. Pure so the request shape can
/// be unit-tested without a network call.
fn cell_update_entries(sheet: &ResolvedSheet, cells: &[CellWrite]) -> Vec<Value> {
    cells
        .iter()
        .map(|write| {
            json!({
                "range": sheet.range(&write.a1()),
                "values": [[write.value]],
            })
        })
        .collect()
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
const STYLE_FIELDS_MASK: &str = "sheets(properties(sheetId,title,gridProperties(frozenRowCount,frozenColumnCount)),data(rowData(values(formattedValue,dataValidation(condition(type)),effectiveFormat(backgroundColor,horizontalAlignment,verticalAlignment,wrapStrategy,numberFormat,textFormat(bold,italic,underline,strikethrough,fontFamily,fontSize,foregroundColor)))),columnMetadata(pixelSize)))";

/// Field mask for the conditional-format read that precedes adding rules: the
/// tab ids and every existing rule (for the replace step).
const CONDITIONAL_FORMATS_FIELDS_MASK: &str = "sheets(properties.sheetId,conditionalFormats)";

/// Locale languages whose decimal mark is a comma (Google Sheets then also
/// separates formula arguments with `;`). Checked on the language prefix of
/// `properties.locale`, with the dot-decimal regional exceptions below.
const DECIMAL_COMMA_LANGUAGES: &[&str] = &[
    "vi", "de", "fr", "es", "it", "pt", "ru", "id", "tr", "nl", "pl", "cs", "sk", "da", "fi", "nb",
    "nn", "no", "sv", "uk", "ro", "hu", "el", "bg", "hr", "sl", "sr", "lt", "lv", "et", "ca", "be",
    "kk", "az", "ka", "hy", "mk", "sq", "is", "gl", "eu",
];
/// Regional locales of the languages above that use a dot decimal mark.
const DECIMAL_DOT_EXCEPTIONS: &[&str] = &["es_MX", "es_US", "es_419", "de_CH", "it_CH"];

/// POSTs a formatting plan as a single `spreadsheets.batchUpdate`. An empty
/// plan (no requests) is a no-op; plans are validated non-empty upstream.
fn apply_format_plan(
    token: &str,
    sheet: &ResolvedSheet,
    plan: &FormatPlan,
) -> Result<(), CoreError> {
    // Replace semantics need the tab's current rules; skip the read when the
    // plan adds none.
    let existing_rules = if plan.conditional_formats.is_empty() {
        Vec::new()
    } else {
        fetch_conditional_rule_ranges(token, sheet)?
    };
    let decimal_comma = locale_uses_decimal_comma(sheet.locale.as_deref());
    let requests = build_format_requests(sheet.sheet_id, plan, &existing_rules, decimal_comma)?;
    if requests.is_empty() {
        return Ok(());
    }
    let url = spreadsheet_batch_update_url(&sheet.spreadsheet_id)?;
    google::post_json(token, url.as_str(), &json!({ "requests": requests }))?;
    Ok(())
}

/// Turns a plan into ordered `spreadsheets.batchUpdate` requests: unmerges,
/// then merges (so a format over a merged range lands on its top-left cell),
/// per-range cell formats and borders, data validations, conditional formats (deletes of the
/// existing rules on exactly the new ranges, or every intersecting rule when
/// the plan sets `replace_intersecting`, highest index first, then the new
/// rules in plan order at the top of the list), then the header freeze,
/// then column widths and row heights. `existing_rules` holds the ranges of each current rule
/// on the tab, by rule index. Pure so the request shape can be unit-tested
/// without a network call.
fn build_format_requests(
    sheet_id: i64,
    plan: &FormatPlan,
    existing_rules: &[Vec<A1Range>],
    decimal_comma: bool,
) -> Result<Vec<Value>, CoreError> {
    let mut requests = Vec::new();
    for range in &plan.unmerges {
        let range = parse_a1_range(range)?;
        requests.push(json!({
            "unmergeCells": { "range": grid_range_json(sheet_id, &range) }
        }));
    }
    for merge in &plan.merges {
        let range = parse_a1_range(&merge.range)?;
        requests.push(json!({
            "mergeCells": {
                "range": grid_range_json(sheet_id, &range),
                "mergeType": merge.kind.api_type(),
            }
        }));
    }
    for format in &plan.formats {
        if let Some(request) = repeat_cell_request(sheet_id, format)? {
            requests.push(request);
        }
        if let Some(request) = border_request(sheet_id, format)? {
            requests.push(request);
        }
    }
    for validation in &plan.validations {
        requests.push(data_validation_request(sheet_id, validation)?);
    }
    if !plan.conditional_formats.is_empty() {
        let targets = plan
            .conditional_formats
            .iter()
            .map(|rule| parse_a1_range(&rule.range))
            .collect::<Result<Vec<_>, _>>()?;
        for index in replaced_rule_indices(existing_rules, &targets, plan.replace_intersecting) {
            requests.push(json!({
                "deleteConditionalFormatRule": { "sheetId": sheet_id, "index": index }
            }));
        }
        for (index, (rule, range)) in plan.conditional_formats.iter().zip(&targets).enumerate() {
            requests.push(add_conditional_format_request(
                sheet_id,
                index,
                rule,
                range,
                decimal_comma,
            )?);
        }
    }
    if let Some(request) = freeze_request(sheet_id, plan.freeze_rows, plan.freeze_columns) {
        requests.push(request);
    }
    requests.extend(column_width_requests(sheet_id, &plan.column_widths)?);
    requests.extend(row_height_requests(sheet_id, &plan.row_heights)?);
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
    if let Some(underline) = format.underline {
        text_format.insert("underline".to_string(), json!(underline));
        fields.push("userEnteredFormat.textFormat.underline");
    }
    if let Some(strikethrough) = format.strikethrough {
        text_format.insert("strikethrough".to_string(), json!(strikethrough));
        fields.push("userEnteredFormat.textFormat.strikethrough");
    }
    if let Some(family) = &format.font_family {
        text_format.insert("fontFamily".to_string(), json!(family));
        fields.push("userEnteredFormat.textFormat.fontFamily");
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
    if let Some(align) = format.vertical_alignment {
        user_format.insert("verticalAlignment".to_string(), json!(align.as_str()));
        fields.push("userEnteredFormat.verticalAlignment");
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

/// One `updateDimensionProperties` request per row-height override, over the
/// 1-based inclusive rows mapped to a zero-based, end-exclusive ROWS range.
fn row_height_requests(sheet_id: i64, heights: &[RowHeight]) -> Result<Vec<Value>, CoreError> {
    heights
        .iter()
        .map(|height| {
            if height.start_row < 1 || height.end_row < height.start_row {
                return Err(CoreError::InvalidInput(format!(
                    "Invalid row span {}:{}",
                    height.start_row, height.end_row
                )));
            }
            Ok(json!({
                "updateDimensionProperties": {
                    "range": {
                        "sheetId": sheet_id,
                        "dimension": "ROWS",
                        "startIndex": height.start_row - 1,
                        "endIndex": height.end_row,
                    },
                    "properties": { "pixelSize": height.pixels },
                    "fields": "pixelSize",
                }
            }))
        })
        .collect()
}

/// A `setDataValidation` request: a dropdown (ONE_OF_LIST over `values`, with
/// `showCustomUi` from `showDropdown`) or a checkbox (BOOLEAN). Setting a rule
/// replaces whatever validation the range had.
fn data_validation_request(sheet_id: i64, validation: &DataValidation) -> Result<Value, CoreError> {
    let range = parse_a1_range(&validation.range)?;
    let rule = match validation.kind {
        ValidationKind::List => {
            if validation.values.is_empty() {
                return Err(CoreError::InvalidInput(format!(
                    "The list validation on {} has no values",
                    validation.range
                )));
            }
            let values: Vec<Value> = validation
                .values
                .iter()
                .map(|value| json!({ "userEnteredValue": value }))
                .collect();
            json!({
                "condition": { "type": "ONE_OF_LIST", "values": values },
                "strict": validation.strict,
                "showCustomUi": validation.show_dropdown.unwrap_or(true),
            })
        }
        ValidationKind::Checkbox => json!({
            "condition": { "type": "BOOLEAN" },
            "strict": validation.strict,
        }),
    };
    Ok(json!({
        "setDataValidation": {
            "range": grid_range_json(sheet_id, &range),
            "rule": rule,
        }
    }))
}

/// An `addConditionalFormatRule` request inserting a BooleanRule at `index`,
/// so the plan's rules keep their order (earlier = higher priority) above any
/// rules left untouched.
fn add_conditional_format_request(
    sheet_id: i64,
    index: usize,
    rule: &ConditionalFormat,
    range: &A1Range,
    decimal_comma: bool,
) -> Result<Value, CoreError> {
    let mut format = serde_json::Map::new();
    let mut text_format = serde_json::Map::new();
    if let Some(color) = &rule.background_color {
        format.insert(
            "backgroundColorStyle".to_string(),
            json!({ "rgbColor": hex_to_color_json(color)? }),
        );
    }
    if let Some(bold) = rule.bold {
        text_format.insert("bold".to_string(), json!(bold));
    }
    if let Some(color) = &rule.font_color {
        text_format.insert(
            "foregroundColorStyle".to_string(),
            json!({ "rgbColor": hex_to_color_json(color)? }),
        );
    }
    if !text_format.is_empty() {
        format.insert("textFormat".to_string(), Value::Object(text_format));
    }
    Ok(json!({
        "addConditionalFormatRule": {
            "rule": {
                "ranges": [grid_range_json(sheet_id, range)],
                "booleanRule": {
                    "condition": boolean_condition_json(&rule.when, decimal_comma)?,
                    "format": Value::Object(format),
                },
            },
            "index": index,
        }
    }))
}

/// Maps a [`ConditionWhen`] (exactly one key set) onto a BooleanCondition.
/// Number values are written with the spreadsheet's decimal mark, since
/// condition values are parsed as if typed into a cell.
fn boolean_condition_json(when: &ConditionWhen, decimal_comma: bool) -> Result<Value, CoreError> {
    if when.set_count() != 1 {
        return Err(CoreError::InvalidInput(
            "A conditional format needs exactly one condition".to_string(),
        ));
    }
    let number = |value: f64| condition_number(value, decimal_comma);
    let (kind, values): (&str, Vec<String>) = if let Some(text) = &when.text_eq {
        ("TEXT_EQ", vec![text.clone()])
    } else if let Some(text) = &when.text_contains {
        ("TEXT_CONTAINS", vec![text.clone()])
    } else if let Some(value) = when.number_gt {
        ("NUMBER_GREATER", vec![number(value)])
    } else if let Some(value) = when.number_lt {
        ("NUMBER_LESS", vec![number(value)])
    } else if let Some([low, high]) = when.number_between {
        ("NUMBER_BETWEEN", vec![number(low), number(high)])
    } else if when.blank == Some(true) {
        ("BLANK", Vec::new())
    } else if when.not_blank == Some(true) {
        ("NOT_BLANK", Vec::new())
    } else if let Some(formula) = &when.formula {
        ("CUSTOM_FORMULA", vec![formula.clone()])
    } else {
        return Err(CoreError::InvalidInput(
            "blank and notBlank conditions must be true".to_string(),
        ));
    };
    if values.is_empty() {
        return Ok(json!({ "type": kind }));
    }
    let values: Vec<Value> = values
        .into_iter()
        .map(|value| json!({ "userEnteredValue": value }))
        .collect();
    Ok(json!({ "type": kind, "values": values }))
}

/// A condition number as the spreadsheet would parse it: shortest decimal
/// form, with a comma decimal mark in comma-decimal locales.
fn condition_number(value: f64, decimal_comma: bool) -> String {
    let text = value.to_string();
    if decimal_comma {
        text.replace('.', ",")
    } else {
        text
    }
}

/// Whether a Sheets locale (e.g. `vi_VN`, `de_DE`, `en_US`) writes decimals
/// with a comma. Unknown or absent locales are treated as dot-decimal.
fn locale_uses_decimal_comma(locale: Option<&str>) -> bool {
    let Some(locale) = locale else {
        return false;
    };
    if DECIMAL_DOT_EXCEPTIONS.contains(&locale) {
        return false;
    }
    let language = locale.split(['_', '-']).next().unwrap_or(locale);
    DECIMAL_COMMA_LANGUAGES.contains(&language)
}

/// Indices of the existing rules to delete, highest first (so each delete
/// leaves the remaining indices valid). By default only a rule whose range set
/// is exactly one target range is replaced, so a new rule never wipes rules on
/// neighbouring or overlapping ranges; with `intersecting` every rule with a
/// range that intersects one of the target ranges is deleted.
fn replaced_rule_indices(
    existing_rules: &[Vec<A1Range>],
    targets: &[A1Range],
    intersecting: bool,
) -> Vec<usize> {
    let mut indices: Vec<usize> = existing_rules
        .iter()
        .enumerate()
        .filter(|(_, ranges)| {
            if intersecting {
                ranges
                    .iter()
                    .any(|range| targets.iter().any(|target| ranges_intersect(range, target)))
            } else {
                !ranges.is_empty()
                    && targets
                        .iter()
                        .any(|target| ranges.iter().all(|range| same_grid_range(range, target)))
            }
        })
        .map(|(index, _)| index)
        .collect();
    indices.reverse();
    indices
}

/// Whether two ranges cover the same grid cells. A missing start is row or
/// column 0 (the API omits zero indices), a missing end is unbounded.
fn same_grid_range(a: &A1Range, b: &A1Range) -> bool {
    a.start_row.unwrap_or(0) == b.start_row.unwrap_or(0)
        && a.start_col.unwrap_or(0) == b.start_col.unwrap_or(0)
        && a.end_row == b.end_row
        && a.end_col == b.end_col
}

/// Whether two half-open grid ranges overlap; an unbounded side spans the
/// whole dimension.
fn ranges_intersect(a: &A1Range, b: &A1Range) -> bool {
    fn overlaps(
        a_start: Option<usize>,
        a_end: Option<usize>,
        b_start: Option<usize>,
        b_end: Option<usize>,
    ) -> bool {
        let start = a_start.unwrap_or(0).max(b_start.unwrap_or(0));
        let end = a_end.unwrap_or(usize::MAX).min(b_end.unwrap_or(usize::MAX));
        start < end
    }
    overlaps(a.start_row, a.end_row, b.start_row, b.end_row)
        && overlaps(a.start_col, a.end_col, b.start_col, b.end_col)
}

/// Reads the tab's conditional-format rules and returns each rule's ranges,
/// by rule index (`spreadsheets.get` with
/// [`CONDITIONAL_FORMATS_FIELDS_MASK`]).
fn fetch_conditional_rule_ranges(
    token: &str,
    sheet: &ResolvedSheet,
) -> Result<Vec<Vec<A1Range>>, CoreError> {
    let mut url = sheets_base_url(&sheet.spreadsheet_id)?;
    url.query_pairs_mut()
        .append_pair("fields", CONDITIONAL_FORMATS_FIELDS_MASK);
    let body = google::get_json(token, url.as_str())?;
    Ok(conditional_rule_ranges(&body, sheet.sheet_id))
}

/// Pulls the rule ranges of one tab out of a conditional-format read. The API
/// omits zero-valued ids and indices, so a missing one means 0 (and a missing
/// end means unbounded).
fn conditional_rule_ranges(body: &Value, sheet_id: i64) -> Vec<Vec<A1Range>> {
    let index = |value: &Value| value.as_u64().map(|number| number as usize);
    let grid_range = |range: &Value| A1Range {
        start_row: index(&range["startRowIndex"]),
        end_row: index(&range["endRowIndex"]),
        start_col: index(&range["startColumnIndex"]),
        end_col: index(&range["endColumnIndex"]),
    };
    let Some(rules) = body["sheets"]
        .as_array()
        .and_then(|sheets| {
            sheets
                .iter()
                .find(|sheet| sheet["properties"]["sheetId"].as_i64().unwrap_or(0) == sheet_id)
        })
        .and_then(|sheet| sheet["conditionalFormats"].as_array())
    else {
        return Vec::new();
    };
    rules
        .iter()
        .map(|rule| {
            rule["ranges"]
                .as_array()
                .map(|ranges| ranges.iter().map(grid_range).collect())
                .unwrap_or_default()
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

/// The A1 window a style read fetches: the 1-based `header_row` and the row
/// below it (the sample row), across the supported column window. Rejects a
/// header row outside `1..=`[`STYLE_HEADER_ROW_MAX`].
fn style_range_a1(header_row: i64) -> Result<String, CoreError> {
    if !(HEADER_ROW..=STYLE_HEADER_ROW_MAX).contains(&header_row) {
        return Err(CoreError::InvalidInput(format!(
            "headerRow must be between {HEADER_ROW} and {STYLE_HEADER_ROW_MAX}"
        )));
    }
    let sample_row = header_row + 1;
    Ok(format!("A{header_row}:{LAST_COLUMN}{sample_row}"))
}

/// Reads the effective style of the tab via one `spreadsheets.get` with
/// `includeGridData` over `range` (the header row and the row below it, from
/// [`style_range_a1`]), plus a second metadata read for the conditional-format
/// count: a `ranges`-scoped get only returns the rules that touch those rows,
/// so it would miss rules further down the tab.
fn read_table_style_for(
    token: &str,
    sheet: &ResolvedSheet,
    range: &str,
    header_row: i64,
) -> Result<TableStyle, CoreError> {
    let mut url = sheets_base_url(&sheet.spreadsheet_id)?;
    url.query_pairs_mut()
        .append_pair("ranges", &sheet.range(range))
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
        header_row,
        column_count: used as i64,
        header: cell_styles(&header_values, used),
        sample: cell_styles(&sample_values, used),
        column_widths: style_column_widths(&data["columnMetadata"], used),
        conditional_format_count: fetch_conditional_rule_ranges(token, sheet)?.len() as i64,
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
            let cell = values.get(index).unwrap_or(&null);
            let mut style = parse_cell_style(&column_id_for_index(index), &cell["effectiveFormat"]);
            style.validation = cell["dataValidation"]["condition"]["type"]
                .as_str()
                .map(validation_label);
            style
        })
        .collect()
}

/// A short label for a cell's data-validation condition type: `list` for a
/// dropdown, `checkbox` for BOOLEAN, otherwise the type in lowercase.
fn validation_label(condition_type: &str) -> String {
    match condition_type {
        "ONE_OF_LIST" => "list".to_string(),
        "BOOLEAN" => "checkbox".to_string(),
        other => other.to_ascii_lowercase(),
    }
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
        underline: text["underline"].as_bool().filter(|underline| *underline),
        strikethrough: text["strikethrough"].as_bool().filter(|strike| *strike),
        font_family: text["fontFamily"]
            .as_str()
            .filter(|family| !family.is_empty())
            .map(str::to_string),
        font_size: text["fontSize"].as_i64(),
        font_color: google_color_to_hex(&text["foregroundColor"]),
        background_color: google_color_to_hex(&effective_format["backgroundColor"]),
        horizontal_alignment: effective_format["horizontalAlignment"]
            .as_str()
            .map(str::to_string),
        vertical_alignment: effective_format["verticalAlignment"]
            .as_str()
            .map(str::to_string),
        number_format: effective_format["numberFormat"]["pattern"]
            .as_str()
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_string),
        wrap: effective_format["wrapStrategy"]
            .as_str()
            .map(|strategy| strategy == "WRAP"),
        validation: None,
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

/// A non-negative grid dimension or offset from an API body (absent or bad
/// = 0, since the API omits zero values).
fn grid_dimension(value: &Value) -> usize {
    value
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0)
}

/// The `ranges` value of a `read_formats` read: the quoted tab title alone
/// (the whole tab) or qualified with the window (see [`window_a1`]).
fn formats_range_a1(title: &str, range: Option<&A1Range>) -> String {
    match range {
        Some(range) => format!("{}!{}", quote_sheet_title(title), window_a1(range)),
        None => quote_sheet_title(title),
    }
}

/// The JSON key of the chosen format source in a `CellData`.
fn format_source_key(source: FormatSource) -> &'static str {
    match source {
        FormatSource::Effective => "effectiveFormat",
        FormatSource::UserEntered => "userEnteredFormat",
    }
}

/// The `fields` mask of a `read_formats` grid read: only the requested
/// properties of the chosen format (`effectiveFormat` or
/// `userEnteredFormat`), `formattedValue` when values are requested, the data
/// block's start offsets, and the spreadsheet theme when a color may be a
/// theme reference.
fn formats_fields_mask(fields: &FormatFields, source: FormatSource) -> String {
    let mut text_parts = Vec::new();
    if fields.font_color {
        text_parts.push("foregroundColor,foregroundColorStyle");
    }
    for (set, name) in [
        (fields.bold, "bold"),
        (fields.italic, "italic"),
        (fields.strikethrough, "strikethrough"),
        (fields.underline, "underline"),
        (fields.font_family, "fontFamily"),
        (fields.font_size, "fontSize"),
    ] {
        if set {
            text_parts.push(name);
        }
    }
    let mut format_parts = Vec::new();
    if fields.background {
        format_parts.push("backgroundColor,backgroundColorStyle".to_string());
    }
    if fields.vertical_alignment {
        format_parts.push("verticalAlignment".to_string());
    }
    if !text_parts.is_empty() {
        format_parts.push(format!("textFormat({})", text_parts.join(",")));
    }
    let mut value_parts = Vec::new();
    if !format_parts.is_empty() {
        value_parts.push(format!(
            "{}({})",
            format_source_key(source),
            format_parts.join(",")
        ));
    }
    if fields.value {
        value_parts.push("formattedValue".to_string());
    }
    let sheets = format!(
        "sheets(properties(sheetId,title),data(startRow,startColumn,rowData(values({}))))",
        value_parts.join(",")
    );
    if fields.any_color() {
        format!("properties.spreadsheetTheme.themeColors,{sheets}")
    } else {
        sheets
    }
}

/// The window-anchored cells of a `read_formats` grid read. The API omits
/// zero values, so a missing `startRow`/`startColumn` is 0 and a missing
/// color component is 0; rows and cells may be missing or short (a missing
/// cell is a default cell). Cells above or left of the window are dropped.
fn format_samples_from_body(body: &Value, request: &FormatsRequest) -> Vec<Vec<CellSample>> {
    let theme = theme_colors(body);
    let window_row = request.range.and_then(|range| range.start_row).unwrap_or(0);
    let window_col = request.range.and_then(|range| range.start_col).unwrap_or(0);
    let format_key = format_source_key(request.source);
    let mut rows: Vec<Vec<CellSample>> = Vec::new();
    let Some(blocks) = body["sheets"][0]["data"].as_array() else {
        return rows;
    };
    for block in blocks {
        let start_row = grid_dimension(&block["startRow"]);
        let start_col = grid_dimension(&block["startColumn"]);
        let Some(row_data) = block["rowData"].as_array() else {
            continue;
        };
        for (offset, row) in row_data.iter().enumerate() {
            let Some(row_index) = (start_row + offset).checked_sub(window_row) else {
                continue;
            };
            let Some(values) = row["values"].as_array() else {
                continue;
            };
            let mut samples: Vec<CellSample> = Vec::new();
            for (position, cell) in values.iter().enumerate() {
                let Some(col_index) = (start_col + position).checked_sub(window_col) else {
                    continue;
                };
                if samples.len() <= col_index {
                    samples.resize(col_index + 1, CellSample::default());
                }
                samples[col_index] = cell_sample(cell, format_key, &theme).masked(&request.fields);
            }
            if rows.len() <= row_index {
                rows.resize(row_index + 1, Vec::new());
            }
            rows[row_index] = samples;
        }
    }
    rows
}

/// One `CellData` as a [`CellSample`] (every property; the caller masks).
fn cell_sample(cell: &Value, format_key: &str, theme: &HashMap<String, String>) -> CellSample {
    let format = &cell[format_key];
    let text = &format["textFormat"];
    let flag = |key: &str| text[key].as_bool().unwrap_or(false);
    CellSample {
        background: styled_color_to_hex(
            &format["backgroundColor"],
            &format["backgroundColorStyle"],
            theme,
        ),
        font_color: styled_color_to_hex(
            &text["foregroundColor"],
            &text["foregroundColorStyle"],
            theme,
        ),
        bold: flag("bold"),
        italic: flag("italic"),
        strikethrough: flag("strikethrough"),
        underline: flag("underline"),
        font_family: text["fontFamily"]
            .as_str()
            .filter(|family| !family.is_empty())
            .map(str::to_string),
        font_size: text["fontSize"].as_i64(),
        vertical_alignment: format["verticalAlignment"]
            .as_str()
            .filter(|align| !align.is_empty())
            .map(str::to_string),
        value: cell["formattedValue"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    }
}

/// A color from a `*ColorStyle` (which takes precedence) or the plain,
/// deprecated color field, as `#rrggbb`. A theme reference is looked up in
/// the spreadsheet theme; an unresolved one falls back to the plain field.
/// `None` when neither is present (the default).
fn styled_color_to_hex(
    plain: &Value,
    style: &Value,
    theme: &HashMap<String, String>,
) -> Option<String> {
    if style["rgbColor"].is_object() {
        return google_color_to_hex(&style["rgbColor"]);
    }
    if let Some(color) = style["themeColor"]
        .as_str()
        .and_then(|name| theme.get(name))
    {
        return Some(color.clone());
    }
    google_color_to_hex(plain)
}

/// The spreadsheet theme's colors by type (`ACCENT1` -> `#rrggbb`).
fn theme_colors(body: &Value) -> HashMap<String, String> {
    body["properties"]["spreadsheetTheme"]["themeColors"]
        .as_array()
        .map(|colors| {
            colors
                .iter()
                .filter_map(|entry| {
                    let name = entry["colorType"].as_str()?;
                    let hex = google_color_to_hex(&entry["color"]["rgbColor"])?;
                    Some((name.to_string(), hex))
                })
                .collect()
        })
        .unwrap_or_default()
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
    use crate::types::{HorizontalAlignment, MergeKind, MergeRange, VerticalAlignment};
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
    fn cell_update_entries_scope_each_write_to_the_resolved_tab() {
        let sheet = ResolvedSheet {
            spreadsheet_id: "spreadsheet".to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("Tab 27".to_string()),
            sheet_id: 851827100,
            locale: None,
        };
        let cells = vec![
            CellWrite {
                column: "E".to_string(),
                row: 48,
                value: "350h".to_string(),
            },
            CellWrite {
                column: "A".to_string(),
                row: 1,
                value: "=SUM(B2:B9)".to_string(),
            },
        ];

        let entries = cell_update_entries(&sheet, &cells);

        assert_eq!(
            entries[0],
            json!({ "range": "'Tab 27'!E48", "values": [["350h"]] })
        );
        assert_eq!(
            entries[1],
            json!({ "range": "'Tab 27'!A1", "values": [["=SUM(B2:B9)"]] }),
            "formula text passes through for USER_ENTERED parsing"
        );
    }

    #[test]
    fn rows_from_values_body_extracts_the_2d_array_and_defaults_empties() {
        let body = json!({ "values": [["=SUM(A1:A2)", "x"], ["1"]] });
        let rows = rows_from_values_body(&body);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0][0],
            json!("=SUM(A1:A2)"),
            "formulas pass through raw"
        );
        assert_eq!(rows[1].len(), 1, "short rows are kept as-is");
        assert!(
            rows_from_values_body(&json!({})).is_empty(),
            "a body with no values is an empty grid"
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
            locale: None,
        };
        assert_eq!(first.range("A1:ZZ1"), "A1:ZZ1");

        let named = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("My Sheet".to_string()),
            sheet_id: 42,
            locale: None,
        };
        assert_eq!(named.range("A2:ZZ"), "'My Sheet'!A2:ZZ");

        // Embedded single quotes are doubled per the A1 grammar.
        let quoted = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("Bob's Tab".to_string()),
            sheet_id: 7,
            locale: None,
        };
        assert_eq!(quoted.range("A1"), "'Bob''s Tab'!A1");
    }

    #[test]
    fn resolve_sheet_title_maps_gid_and_title_or_errors() {
        let meta = SpreadsheetMeta {
            title: "Workbook".to_string(),
            locale: None,
            time_zone: None,
            sheets: vec![
                SheetProperties {
                    sheet_id: 0,
                    title: "Sheet1".to_string(),
                    index: 0,
                    grid_rows: 1000,
                    grid_cols: 26,
                },
                SheetProperties {
                    sheet_id: 987,
                    title: "Data".to_string(),
                    index: 1,
                    grid_rows: 1000,
                    grid_cols: 26,
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
            underline: None,
            strikethrough: None,
            font_family: None,
            font_size: None,
            font_color: None,
            background_color: None,
            horizontal_alignment: None,
            vertical_alignment: None,
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
            column_widths: vec![ColumnWidth {
                column: "A".to_string(),
                pixels: 200,
            }],
            ..FormatPlan::default()
        };
        let requests = build_format_requests(0, &plan, &[], false).expect("requests");
        assert!(requests[0].get("repeatCell").is_some());
        assert!(requests[1].get("updateBorders").is_some());
        assert!(requests[2].get("updateSheetProperties").is_some());
        assert!(requests[3].get("updateDimensionProperties").is_some());

        let empty = build_format_requests(0, &FormatPlan::default(), &[], false).expect("requests");
        assert!(empty.is_empty(), "an empty plan produces no requests");
    }

    #[test]
    fn repeat_cell_request_writes_font_family_decoration_and_vertical_alignment() {
        let format = CellFormat {
            font_family: Some("Lexend".to_string()),
            underline: Some(true),
            strikethrough: Some(false),
            vertical_alignment: Some(VerticalAlignment::Middle),
            ..cell_format("A1:B1")
        };
        let request = repeat_cell_request(0, &format)
            .expect("request")
            .expect("some");
        let repeat = &request["repeatCell"];
        let text = &repeat["cell"]["userEnteredFormat"]["textFormat"];
        assert_eq!(text["fontFamily"], "Lexend");
        assert_eq!(text["underline"], true);
        assert_eq!(text["strikethrough"], false);
        assert_eq!(
            repeat["cell"]["userEnteredFormat"]["verticalAlignment"],
            "MIDDLE"
        );
        assert_eq!(
            repeat["fields"],
            "userEnteredFormat.textFormat.underline,userEnteredFormat.textFormat.strikethrough,\
             userEnteredFormat.textFormat.fontFamily,userEnteredFormat.verticalAlignment"
        );

        let font_only = CellFormat {
            font_family: Some("Roboto Mono".to_string()),
            ..cell_format("A1")
        };
        let request = repeat_cell_request(0, &font_only)
            .expect("request")
            .expect("some");
        assert_eq!(
            request["repeatCell"]["fields"], "userEnteredFormat.textFormat.fontFamily",
            "a font change leaves bold, size and color untouched"
        );
    }

    #[test]
    fn merges_map_their_type_and_run_after_unmerges_before_formats() {
        let plan = FormatPlan {
            formats: vec![CellFormat {
                bold: Some(true),
                ..cell_format("A1:F1")
            }],
            unmerges: vec!["A1:F3".to_string()],
            merges: vec![
                MergeRange {
                    range: "A1:F1".to_string(),
                    kind: MergeKind::All,
                },
                MergeRange {
                    range: "A2:C3".to_string(),
                    kind: MergeKind::Rows,
                },
                MergeRange {
                    range: "E2:F3".to_string(),
                    kind: MergeKind::Columns,
                },
            ],
            row_heights: vec![
                RowHeight {
                    start_row: 1,
                    end_row: 1,
                    pixels: 48,
                },
                RowHeight {
                    start_row: 5,
                    end_row: 9,
                    pixels: 24,
                },
            ],
            column_widths: vec![ColumnWidth {
                column: "A".to_string(),
                pixels: 200,
            }],
            ..FormatPlan::default()
        };
        let requests = build_format_requests(4, &plan, &[], false).expect("requests");
        let kinds: Vec<&str> = requests
            .iter()
            .map(|request| {
                request
                    .as_object()
                    .and_then(|object| object.keys().next())
                    .map(String::as_str)
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "unmergeCells",
                "mergeCells",
                "mergeCells",
                "mergeCells",
                "repeatCell",
                "updateDimensionProperties",
                "updateDimensionProperties",
                "updateDimensionProperties",
            ]
        );
        assert_eq!(
            requests[0]["unmergeCells"]["range"],
            json!({ "sheetId": 4, "startRowIndex": 0, "endRowIndex": 3, "startColumnIndex": 0, "endColumnIndex": 6 })
        );
        assert_eq!(requests[1]["mergeCells"]["mergeType"], "MERGE_ALL");
        assert_eq!(
            requests[1]["mergeCells"]["range"],
            json!({ "sheetId": 4, "startRowIndex": 0, "endRowIndex": 1, "startColumnIndex": 0, "endColumnIndex": 6 })
        );
        assert_eq!(requests[2]["mergeCells"]["mergeType"], "MERGE_ROWS");
        assert_eq!(requests[3]["mergeCells"]["mergeType"], "MERGE_COLUMNS");
        assert_eq!(
            requests[5]["updateDimensionProperties"]["range"]["dimension"],
            "COLUMNS"
        );
        assert_eq!(
            requests[6]["updateDimensionProperties"],
            json!({
                "range": { "sheetId": 4, "dimension": "ROWS", "startIndex": 0, "endIndex": 1 },
                "properties": { "pixelSize": 48 },
                "fields": "pixelSize",
            })
        );
        assert_eq!(
            requests[7]["updateDimensionProperties"]["range"],
            json!({ "sheetId": 4, "dimension": "ROWS", "startIndex": 4, "endIndex": 9 })
        );
    }

    fn list_validation(range: &str, values: &[&str]) -> DataValidation {
        DataValidation {
            range: range.to_string(),
            kind: ValidationKind::List,
            values: values.iter().map(|value| value.to_string()).collect(),
            strict: true,
            show_dropdown: Some(true),
        }
    }

    fn rule(range: &str, when: ConditionWhen) -> ConditionalFormat {
        ConditionalFormat {
            range: range.to_string(),
            when,
            background_color: Some("#d1fae5".to_string()),
            font_color: None,
            bold: None,
        }
    }

    #[test]
    fn list_validation_builds_a_one_of_list_dropdown() {
        let mut validation = list_validation("D2:D21", &["Todo", "Done"]);
        validation.strict = false;
        validation.show_dropdown = Some(false);
        let request = data_validation_request(4, &validation).expect("request");
        let set = &request["setDataValidation"];
        assert_eq!(set["range"]["sheetId"], 4);
        assert_eq!(set["range"]["startRowIndex"], 1);
        assert_eq!(set["range"]["endRowIndex"], 21);
        assert_eq!(set["range"]["startColumnIndex"], 3);
        assert_eq!(
            set["rule"],
            json!({
                "condition": {
                    "type": "ONE_OF_LIST",
                    "values": [{ "userEnteredValue": "Todo" }, { "userEnteredValue": "Done" }],
                },
                "strict": false,
                "showCustomUi": false,
            })
        );

        let empty = list_validation("D2:D21", &[]);
        assert!(data_validation_request(4, &empty).is_err());
    }

    #[test]
    fn checkbox_validation_builds_a_boolean_rule() {
        let validation = DataValidation {
            range: "E:E".to_string(),
            kind: ValidationKind::Checkbox,
            values: Vec::new(),
            strict: true,
            show_dropdown: None,
        };
        let request = data_validation_request(0, &validation).expect("request");
        assert_eq!(
            request["setDataValidation"]["rule"],
            json!({ "condition": { "type": "BOOLEAN" }, "strict": true })
        );
        assert!(request["setDataValidation"]["range"]
            .get("startRowIndex")
            .is_none());
    }

    #[test]
    fn every_condition_maps_to_its_boolean_condition_type() {
        let cases = [
            (
                ConditionWhen {
                    text_eq: Some("Done".to_string()),
                    ..ConditionWhen::default()
                },
                json!({ "type": "TEXT_EQ", "values": [{ "userEnteredValue": "Done" }] }),
            ),
            (
                ConditionWhen {
                    text_contains: Some("late".to_string()),
                    ..ConditionWhen::default()
                },
                json!({ "type": "TEXT_CONTAINS", "values": [{ "userEnteredValue": "late" }] }),
            ),
            (
                ConditionWhen {
                    number_gt: Some(100.0),
                    ..ConditionWhen::default()
                },
                json!({ "type": "NUMBER_GREATER", "values": [{ "userEnteredValue": "100" }] }),
            ),
            (
                ConditionWhen {
                    number_lt: Some(0.5),
                    ..ConditionWhen::default()
                },
                json!({ "type": "NUMBER_LESS", "values": [{ "userEnteredValue": "0.5" }] }),
            ),
            (
                ConditionWhen {
                    number_between: Some([1.0, 2.5]),
                    ..ConditionWhen::default()
                },
                json!({
                    "type": "NUMBER_BETWEEN",
                    "values": [{ "userEnteredValue": "1" }, { "userEnteredValue": "2.5" }],
                }),
            ),
            (
                ConditionWhen {
                    blank: Some(true),
                    ..ConditionWhen::default()
                },
                json!({ "type": "BLANK" }),
            ),
            (
                ConditionWhen {
                    not_blank: Some(true),
                    ..ConditionWhen::default()
                },
                json!({ "type": "NOT_BLANK" }),
            ),
            (
                ConditionWhen {
                    formula: Some("=$D2=\"Done\"".to_string()),
                    ..ConditionWhen::default()
                },
                json!({
                    "type": "CUSTOM_FORMULA",
                    "values": [{ "userEnteredValue": "=$D2=\"Done\"" }],
                }),
            ),
        ];
        for (when, expected) in cases {
            assert_eq!(
                boolean_condition_json(&when, false).expect("condition"),
                expected
            );
        }
        assert!(boolean_condition_json(&ConditionWhen::default(), false).is_err());
        let two = ConditionWhen {
            blank: Some(true),
            number_gt: Some(1.0),
            ..ConditionWhen::default()
        };
        assert!(boolean_condition_json(&two, false).is_err());
    }

    #[test]
    fn condition_numbers_follow_the_locale_decimal_mark() {
        let when = ConditionWhen {
            number_between: Some([0.25, 0.65]),
            ..ConditionWhen::default()
        };
        let condition = boolean_condition_json(&when, true).expect("condition");
        assert_eq!(condition["values"][0]["userEnteredValue"], "0,25");
        assert_eq!(condition["values"][1]["userEnteredValue"], "0,65");

        assert!(locale_uses_decimal_comma(Some("vi_VN")));
        assert!(locale_uses_decimal_comma(Some("de_DE")));
        assert!(locale_uses_decimal_comma(Some("pt_BR")));
        assert!(!locale_uses_decimal_comma(Some("en_US")));
        assert!(!locale_uses_decimal_comma(Some("es_MX")));
        assert!(!locale_uses_decimal_comma(None));
    }

    #[test]
    fn conditional_format_rule_carries_the_format_styles() {
        let conditional = ConditionalFormat {
            font_color: Some("#065f46".to_string()),
            bold: Some(true),
            ..rule(
                "D2:D21",
                ConditionWhen {
                    text_eq: Some("Done".to_string()),
                    ..ConditionWhen::default()
                },
            )
        };
        let range = parse_a1_range("D2:D21").expect("range");
        let request =
            add_conditional_format_request(3, 0, &conditional, &range, false).expect("request");
        let add = &request["addConditionalFormatRule"];
        assert_eq!(add["index"], 0);
        assert_eq!(add["rule"]["ranges"][0]["sheetId"], 3);
        let format = &add["rule"]["booleanRule"]["format"];
        assert!(format["backgroundColorStyle"]["rgbColor"].is_object());
        assert_eq!(format["textFormat"]["bold"], true);
        assert!(format["textFormat"]["foregroundColorStyle"]["rgbColor"].is_object());
        assert_eq!(add["rule"]["booleanRule"]["condition"]["type"], "TEXT_EQ");
    }

    /// Existing rules on tab 5 used by the replace tests, by index:
    /// 0 = D2:D21 (overlaps D10:D21), 1 = H1:H5 (disjoint), 2 = whole column D
    /// (overlaps), 3 = exactly D10:D21 (API form: zero start omitted elsewhere),
    /// 4 = D10:D21 plus E10:E21 (overlaps, but not the same range set).
    fn existing_replace_rules() -> Vec<Vec<A1Range>> {
        let body = json!({
            "sheets": [
                { "properties": { "sheetId": 9 }, "conditionalFormats": [
                    { "ranges": [{ "sheetId": 9 }] }
                ]},
                { "properties": { "sheetId": 5 }, "conditionalFormats": [
                    { "ranges": [{ "sheetId": 5, "startRowIndex": 1, "endRowIndex": 21,
                                   "startColumnIndex": 3, "endColumnIndex": 4 }] },
                    { "ranges": [{ "sheetId": 5, "endRowIndex": 5,
                                   "startColumnIndex": 7, "endColumnIndex": 8 }] },
                    { "ranges": [{ "sheetId": 5, "startColumnIndex": 3, "endColumnIndex": 4 }] },
                    { "ranges": [{ "sheetId": 5, "startRowIndex": 9, "endRowIndex": 21,
                                   "startColumnIndex": 3, "endColumnIndex": 4 }] },
                    { "ranges": [
                        { "sheetId": 5, "startRowIndex": 9, "endRowIndex": 21,
                          "startColumnIndex": 3, "endColumnIndex": 4 },
                        { "sheetId": 5, "startRowIndex": 9, "endRowIndex": 21,
                          "startColumnIndex": 4, "endColumnIndex": 5 }
                    ]}
                ]}
            ]
        });
        let existing = conditional_rule_ranges(&body, 5);
        assert_eq!(existing.len(), 5);
        existing
    }

    /// Two rules on D10:D21 plus a validation and a freeze.
    fn replace_plan(replace_intersecting: bool) -> FormatPlan {
        FormatPlan {
            validations: vec![list_validation("D2:D21", &["Todo", "Done"])],
            conditional_formats: vec![
                rule(
                    "D10:D21",
                    ConditionWhen {
                        text_eq: Some("Done".to_string()),
                        ..ConditionWhen::default()
                    },
                ),
                rule(
                    "D10:D21",
                    ConditionWhen {
                        blank: Some(true),
                        ..ConditionWhen::default()
                    },
                ),
            ],
            freeze_rows: Some(1),
            replace_intersecting,
            ..FormatPlan::default()
        }
    }

    fn request_kinds(requests: &[Value]) -> Vec<&str> {
        requests
            .iter()
            .map(|request| {
                request
                    .as_object()
                    .and_then(|object| object.keys().next())
                    .map(String::as_str)
                    .expect("request kind")
            })
            .collect()
    }

    fn deleted_indices(requests: &[Value]) -> Vec<i64> {
        requests
            .iter()
            .filter_map(|request| request["deleteConditionalFormatRule"]["index"].as_i64())
            .collect()
    }

    #[test]
    fn conditional_formats_replace_only_rules_on_the_exact_same_range() {
        let existing = existing_replace_rules();
        let requests =
            build_format_requests(5, &replace_plan(false), &existing, false).expect("requests");
        assert_eq!(
            request_kinds(&requests),
            [
                "setDataValidation",
                "deleteConditionalFormatRule",
                "addConditionalFormatRule",
                "addConditionalFormatRule",
                "updateSheetProperties",
            ]
        );
        assert_eq!(
            deleted_indices(&requests),
            [3],
            "only the exact D10:D21 rule"
        );
        assert_eq!(requests[1]["deleteConditionalFormatRule"]["sheetId"], 5);
        // New rules keep plan order at the top of the list.
        assert_eq!(requests[2]["addConditionalFormatRule"]["index"], 0);
        assert_eq!(requests[3]["addConditionalFormatRule"]["index"], 1);

        // No existing rules: only adds.
        let fresh = build_format_requests(5, &replace_plan(false), &[], false).expect("requests");
        assert!(deleted_indices(&fresh).is_empty());
    }

    #[test]
    fn conditional_formats_keep_intersecting_rules_by_default() {
        // A full-row rule over B10:I21 must not wipe the per-column rules in D.
        let existing = existing_replace_rules();
        let plan = FormatPlan {
            conditional_formats: vec![rule(
                "B10:I21",
                ConditionWhen {
                    formula: Some("=$E10=\"High\"".to_string()),
                    ..ConditionWhen::default()
                },
            )],
            ..FormatPlan::default()
        };
        let requests = build_format_requests(5, &plan, &existing, false).expect("requests");
        assert!(deleted_indices(&requests).is_empty());
        assert_eq!(request_kinds(&requests), ["addConditionalFormatRule"]);

        // An exact-range match works with the zero-start form too (A1 vs API).
        let whole_d = parse_a1_range("D:D").expect("range");
        assert_eq!(replaced_rule_indices(&existing, &[whole_d], false), [2]);
        let top = parse_a1_range("H1:H5").expect("range");
        assert_eq!(replaced_rule_indices(&existing, &[top], false), [1]);
    }

    #[test]
    fn conditional_formats_delete_intersecting_rules_with_the_flag() {
        let existing = existing_replace_rules();
        let requests =
            build_format_requests(5, &replace_plan(true), &existing, false).expect("requests");
        // Deletes run highest index first so earlier indices stay valid; H1:H5
        // (index 1) is disjoint and survives.
        assert_eq!(deleted_indices(&requests), [4, 3, 2, 0]);
        assert_eq!(
            request_kinds(&requests)
                .iter()
                .filter(|kind| **kind == "addConditionalFormatRule")
                .count(),
            2
        );
    }

    #[test]
    fn missing_sheet_id_in_a_conditional_read_means_the_first_tab() {
        let body = json!({
            "sheets": [{ "properties": {}, "conditionalFormats": [{ "ranges": [{}] }] }]
        });
        let existing = conditional_rule_ranges(&body, 0);
        assert_eq!(existing.len(), 1);
        let target = parse_a1_range("Z100").expect("range");
        assert!(
            ranges_intersect(&existing[0][0], &target),
            "an unbounded rule covers the tab"
        );
        assert!(conditional_rule_ranges(&body, 3).is_empty());
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
        assert_eq!(empty.font_family, None);
        assert_eq!(empty.vertical_alignment, None);
    }

    #[test]
    fn parse_cell_style_reads_font_decoration_and_vertical_alignment() {
        let effective = json!({
            "verticalAlignment": "MIDDLE",
            "textFormat": {
                "fontFamily": "Lexend",
                "underline": true,
                "strikethrough": false
            }
        });
        let style = parse_cell_style("C", &effective);
        assert_eq!(style.font_family.as_deref(), Some("Lexend"));
        assert_eq!(style.underline, Some(true));
        assert_eq!(style.strikethrough, None, "strikethrough false is omitted");
        assert_eq!(style.vertical_alignment.as_deref(), Some("MIDDLE"));
        let json = serde_json::to_value(&style).expect("serialize");
        assert_eq!(json["fontFamily"], "Lexend");
        assert_eq!(json["verticalAlignment"], "MIDDLE");
        assert!(json.get("strikethrough").is_none());
    }

    #[test]
    fn style_fields_mask_reads_the_new_style_properties() {
        for key in [
            "verticalAlignment",
            "underline",
            "strikethrough",
            "fontFamily",
        ] {
            assert!(STYLE_FIELDS_MASK.contains(key), "{key} in the style mask");
        }
    }

    #[test]
    fn style_range_covers_the_header_row_and_the_row_below() {
        assert_eq!(style_range_a1(1).expect("default"), "A1:ZZ2");
        assert_eq!(style_range_a1(9).expect("document-style"), "A9:ZZ10");
        assert_eq!(
            style_range_a1(STYLE_HEADER_ROW_MAX).expect("max"),
            format!("A{STYLE_HEADER_ROW_MAX}:ZZ{}", STYLE_HEADER_ROW_MAX + 1)
        );
        for bad in [0, -3, STYLE_HEADER_ROW_MAX + 1] {
            let error = style_range_a1(bad).expect_err("out of bounds");
            assert!(
                matches!(&error, CoreError::InvalidInput(message) if message.contains("headerRow")),
                "unexpected error for {bad}: {error}"
            );
        }
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
    fn formats_request(range: Option<&str>, fields: FormatFields) -> FormatsRequest {
        FormatsRequest {
            range: range.map(|range| parse_a1_range(range).expect("range")),
            fields,
            source: FormatSource::Effective,
            max_cells: 1_000,
        }
    }

    #[test]
    fn colors_convert_float_rgb_and_color_styles_to_hex() {
        let theme = HashMap::from([("ACCENT1".to_string(), "#4285f4".to_string())]);
        let none = Value::Null;
        // Zero components are omitted by the API: {} is black.
        assert_eq!(
            styled_color_to_hex(&json!({}), &none, &theme),
            Some("#000000".to_string())
        );
        assert_eq!(
            styled_color_to_hex(
                &json!({ "red": 0.8666667, "green": 0.90588236, "blue": 0.9607843 }),
                &none,
                &theme
            ),
            Some("#dde7f5".to_string())
        );
        // Out-of-range components clamp.
        assert_eq!(
            styled_color_to_hex(&json!({ "red": 2.0, "green": -1.0 }), &none, &theme),
            Some("#ff0000".to_string())
        );
        // The style wins over the plain field; a theme reference resolves.
        assert_eq!(
            styled_color_to_hex(
                &json!({ "red": 1.0 }),
                &json!({ "rgbColor": { "blue": 1.0 } }),
                &theme
            ),
            Some("#0000ff".to_string())
        );
        assert_eq!(
            styled_color_to_hex(&none, &json!({ "themeColor": "ACCENT1" }), &theme),
            Some("#4285f4".to_string())
        );
        // An unknown theme color falls back to the plain field, then to none.
        assert_eq!(
            styled_color_to_hex(
                &json!({ "green": 1.0 }),
                &json!({ "themeColor": "ACCENT6" }),
                &theme
            ),
            Some("#00ff00".to_string())
        );
        assert_eq!(styled_color_to_hex(&none, &none, &theme), None);
    }

    #[test]
    fn theme_colors_are_read_from_the_spreadsheet_properties() {
        let body = json!({ "properties": { "spreadsheetTheme": { "themeColors": [
            { "colorType": "TEXT", "color": { "rgbColor": {} } },
            { "colorType": "ACCENT1", "color": { "rgbColor": { "red": 1.0 } } },
            { "colorType": "LINK" }
        ] } } });
        let theme = theme_colors(&body);
        assert_eq!(theme.get("TEXT").map(String::as_str), Some("#000000"));
        assert_eq!(theme.get("ACCENT1").map(String::as_str), Some("#ff0000"));
        assert!(!theme.contains_key("LINK"));
    }

    #[test]
    fn formats_fields_mask_requests_only_the_needed_fields() {
        let background = FormatFields {
            background: true,
            ..FormatFields::default()
        };
        assert_eq!(
            formats_fields_mask(&background, FormatSource::Effective),
            "properties.spreadsheetTheme.themeColors,sheets(properties(sheetId,title),data(startRow,startColumn,rowData(values(effectiveFormat(backgroundColor,backgroundColorStyle)))))"
        );
        let text = FormatFields {
            bold: true,
            strikethrough: true,
            value: true,
            ..FormatFields::default()
        };
        assert_eq!(
            formats_fields_mask(&text, FormatSource::UserEntered),
            "sheets(properties(sheetId,title),data(startRow,startColumn,rowData(values(userEnteredFormat(textFormat(bold,strikethrough)),formattedValue))))"
        );
        let all = FormatFields {
            background: true,
            font_color: true,
            bold: true,
            italic: true,
            strikethrough: true,
            underline: true,
            font_family: true,
            font_size: true,
            vertical_alignment: true,
            value: true,
        };
        assert_eq!(
            formats_fields_mask(&all, FormatSource::Effective),
            "properties.spreadsheetTheme.themeColors,sheets(properties(sheetId,title),data(startRow,startColumn,rowData(values(effectiveFormat(backgroundColor,backgroundColorStyle,verticalAlignment,textFormat(foregroundColor,foregroundColorStyle,bold,italic,strikethrough,underline,fontFamily,fontSize)),formattedValue))))"
        );
        let font = FormatFields {
            font_family: true,
            ..FormatFields::default()
        };
        assert_eq!(
            formats_fields_mask(&font, FormatSource::UserEntered),
            "sheets(properties(sheetId,title),data(startRow,startColumn,rowData(values(userEnteredFormat(textFormat(fontFamily))))))",
            "a font read needs no theme"
        );
        let value_only = FormatFields {
            value: true,
            ..FormatFields::default()
        };
        assert_eq!(
            formats_fields_mask(&value_only, FormatSource::Effective),
            "sheets(properties(sheetId,title),data(startRow,startColumn,rowData(values(formattedValue))))"
        );
    }

    #[test]
    fn formats_range_quotes_the_tab_and_adds_the_window() {
        assert_eq!(formats_range_a1("Level 1", None), "'Level 1'");
        let window = parse_a1_range("A1:OA95").unwrap();
        assert_eq!(formats_range_a1("Bob's", Some(&window)), "'Bob''s'!A1:OA95");
        let rows = parse_a1_range("5:9").unwrap();
        assert_eq!(formats_range_a1("T", Some(&rows)), "'T'!A5:ZZ9");
    }

    #[test]
    fn format_samples_read_font_family_size_underline_and_vertical_alignment() {
        let fields = FormatFields {
            font_family: true,
            font_size: true,
            underline: true,
            vertical_alignment: true,
            ..FormatFields::default()
        };
        let body = json!({ "sheets": [{ "data": [{ "rowData": [
            { "values": [
                { "effectiveFormat": {
                    "verticalAlignment": "TOP",
                    "textFormat": { "fontFamily": "Lexend", "fontSize": 14, "underline": true, "bold": true }
                } },
                { "effectiveFormat": { "textFormat": { "fontFamily": "Arial", "fontSize": 10 } } },
                {}
            ] }
        ] }] }] });
        let cells = format_samples_from_body(&body, &formats_request(None, fields));
        assert_eq!(cells[0][0].font_family.as_deref(), Some("Lexend"));
        assert_eq!(cells[0][0].font_size, Some(14));
        assert!(cells[0][0].underline);
        assert!(!cells[0][0].bold, "unrequested bold is masked");
        assert_eq!(cells[0][0].vertical_alignment.as_deref(), Some("TOP"));
        assert_eq!(cells[0][1].font_family.as_deref(), Some("Arial"));
        assert_eq!(cells[0][2], CellSample::default());

        let grid = shape_format_grid(
            "T".to_string(),
            &formats_request(None, fields),
            None,
            None,
            cells,
        )
        .expect("shape");
        let output = super::super::formats::encode_formats(&grid, &fields, FormatSource::Effective);
        let family = output.font_family.expect("fontFamily layer");
        assert_eq!(
            family.palette,
            vec![None, Some("Lexend".to_string()), Some("Arial".to_string())]
        );
        assert_eq!(family.grid, vec!["1,2".to_string()]);
        let size = output.font_size.expect("fontSize layer");
        assert_eq!(size.palette, vec![None, Some(14), Some(10)]);
        let underline = output.underline.expect("underline layer");
        assert_eq!(underline.palette, vec![false, true]);
        assert_eq!(underline.grid, vec!["1".to_string()]);
        let vertical = output.vertical_alignment.expect("verticalAlignment layer");
        assert_eq!(vertical.palette, vec![None, Some("TOP".to_string())]);
        assert!(output.background.is_none(), "unrequested layers are absent");
    }

    #[test]
    fn format_samples_handle_omitted_offsets_and_ragged_rows() {
        let fields = FormatFields {
            background: true,
            bold: true,
            value: true,
            ..FormatFields::default()
        };
        // No startRow/startColumn (0), an empty row object, a row without
        // values, a short row, and a cell without effectiveFormat.
        let body = json!({ "sheets": [{ "data": [{ "rowData": [
            { "values": [
                { "effectiveFormat": { "backgroundColor": { "red": 1.0 }, "textFormat": { "bold": true } }, "formattedValue": "F" },
                {},
                { "effectiveFormat": { "backgroundColorStyle": { "rgbColor": {} } } }
            ] },
            {},
            { "values": [{ "formattedValue": "x" }] }
        ] }] }] });
        let cells = format_samples_from_body(&body, &formats_request(None, fields));
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0].len(), 3);
        assert_eq!(cells[0][0].background.as_deref(), Some("#ff0000"));
        assert!(cells[0][0].bold);
        assert_eq!(cells[0][0].value, "F");
        assert_eq!(cells[0][1], CellSample::default());
        assert_eq!(cells[0][2].background.as_deref(), Some("#000000"));
        assert!(cells[1].is_empty());
        assert_eq!(cells[2][0].value, "x");
        assert_eq!(cells[2][0].background, None);

        let grid = shape_format_grid(
            "T".to_string(),
            &formats_request(None, fields),
            None,
            None,
            cells,
        )
        .expect("shape");
        assert_eq!((grid.rows, grid.columns), (3, 3));
    }

    #[test]
    fn format_samples_are_anchored_at_the_window() {
        let fields = FormatFields {
            background: true,
            ..FormatFields::default()
        };
        let body = json!({ "sheets": [{ "data": [{ "startRow": 4, "startColumn": 2, "rowData": [
            { "values": [{ "effectiveFormat": { "backgroundColor": { "blue": 1.0 } } }] },
            { "values": [{}, { "userEnteredFormat": { "backgroundColor": {} }, "effectiveFormat": { "backgroundColor": { "green": 1.0 } } }] }
        ] }] }] });
        let request = formats_request(Some("C5:E9"), fields);
        let cells = format_samples_from_body(&body, &request);
        assert_eq!(cells[0][0].background.as_deref(), Some("#0000ff"));
        assert_eq!(cells[1][1].background.as_deref(), Some("#00ff00"));

        let user = FormatsRequest {
            source: FormatSource::UserEntered,
            ..request.clone()
        };
        let cells = format_samples_from_body(&body, &user);
        assert_eq!(cells[0][0].background, None);
        assert_eq!(cells[1][1].background.as_deref(), Some("#000000"));

        // A block starting above/left of the window drops the outside cells.
        let shifted = formats_request(Some("D6:E9"), fields);
        let cells = format_samples_from_body(&body, &shifted);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0][0].background.as_deref(), Some("#00ff00"));

        assert!(format_samples_from_body(&json!({}), &request).is_empty());
    }
}
