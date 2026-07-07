//! Real Google Sheets connector: Drive `files.list` for spreadsheet
//! discovery and the Sheets values API for reads and writes. The first row of
//! the first visible sheet is the header; records map 1:1 onto sheet rows
//! with ids "row_{n}" (n = 1-based sheet row, so data starts at row_2).
//! Tokens are obtained through the crate-private google module and never
//! leave this crate.

use rusqlite::Connection;
use serde_json::{json, Value};

use super::{js_string, TableConnector};
use crate::constants::FIND_RECORDS_LIMIT;
use crate::error::CoreError;
use crate::google;
use crate::sources;
use crate::types::{
    DataSource, FieldSchema, JsonMap, ReadOptions, RecordPatch, SourceKind, TableRecord, TableRef,
    TableSchema,
};

const DRIVE_FILES_ENDPOINT: &str = "https://www.googleapis.com/drive/v3/files";
const SHEETS_ENDPOINT: &str = "https://sheets.googleapis.com/v4/spreadsheets";
const SPREADSHEET_MIME_TYPE: &str = "application/vnd.google-apps.spreadsheet";
const DRIVE_PAGE_SIZE: &str = "100";

/// Column window for value ranges; ZZ = 702 columns, far beyond broker use.
const LAST_COLUMN: &str = "ZZ";
const HEADER_ROW: i64 = 1;
const FIRST_DATA_ROW: i64 = 2;
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
        let header = self.fetch_header(&token, &sheet)?;
        if header.is_empty() {
            return Err(CoreError::InvalidInput(format!(
                "Spreadsheet {} has no header row to map fields onto",
                sheet.spreadsheet_id
            )));
        }

        let values: Vec<Vec<Value>> = records
            .iter()
            .map(|fields| row_values(&header, fields))
            .collect();
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
        Ok(records
            .iter()
            .enumerate()
            .map(|(index, fields)| TableRecord {
                id: record_id_for(start_row + index as i64),
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
                "Spreadsheet {} has no header row to map fields onto",
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
        Ok(Self {
            spreadsheet_id: parsed.spreadsheet_id,
            spreadsheet_title: meta.title,
            sheet_title,
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
}

/// `GET {SHEETS_ENDPOINT}/{id}?fields=properties.title,sheets.properties(sheetId,title)`.
/// Fixed endpoint + token; the id only selects the resource.
fn fetch_spreadsheet_meta(token: &str, spreadsheet_id: &str) -> Result<SpreadsheetMeta, CoreError> {
    let mut url = sheets_base_url(spreadsheet_id)?;
    url.query_pairs_mut().append_pair(
        "fields",
        "properties.title,sheets.properties(sheetId,title)",
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

#[cfg(test)]
mod tests {
    use super::*;
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
        };
        assert_eq!(first.range("A1:ZZ1"), "A1:ZZ1");

        let named = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("My Sheet".to_string()),
        };
        assert_eq!(named.range("A2:ZZ"), "'My Sheet'!A2:ZZ");

        // Embedded single quotes are doubled per the A1 grammar.
        let quoted = ResolvedSheet {
            spreadsheet_id: SAMPLE_ID.to_string(),
            spreadsheet_title: "Book".to_string(),
            sheet_title: Some("Bob's Tab".to_string()),
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
                },
                SheetProperties {
                    sheet_id: 987,
                    title: "Data".to_string(),
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
}
