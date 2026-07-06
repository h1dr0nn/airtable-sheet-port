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

#[derive(Default)]
pub struct GoogleSheetsConnector;

impl GoogleSheetsConnector {
    pub fn new() -> Self {
        Self
    }

    /// Header cells of the first visible sheet as field names (row 1).
    fn fetch_header(&self, token: &str, spreadsheet_id: &str) -> Result<Vec<String>, CoreError> {
        let rows = fetch_values(
            token,
            spreadsheet_id,
            &format!("A{HEADER_ROW}:{LAST_COLUMN}{HEADER_ROW}"),
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
        spreadsheet_id: &str,
        header: &[String],
    ) -> Result<Vec<TableRecord>, CoreError> {
        let rows = fetch_values(
            token,
            spreadsheet_id,
            &format!("A{FIRST_DATA_ROW}:{LAST_COLUMN}"),
        )?;
        Ok(records_from_rows(header, &rows, FIRST_DATA_ROW))
    }
}

impl TableConnector for GoogleSheetsConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::GoogleSheets
    }

    /// The `google-sheets` source row, only while a token is present in the
    /// OS keychain. No network call: presence is a local check.
    fn list_sources(&self, conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        if !google::has_token() {
            return Ok(Vec::new());
        }
        Ok(sources::list(conn)?
            .into_iter()
            .filter(|source| source.kind == SourceKind::GoogleSheets)
            .collect())
    }

    /// Spreadsheets visible to the account via Drive `files.list`; each file
    /// is exposed as one table (its first visible sheet).
    fn list_tables(&self, conn: &Connection, source_id: &str) -> Result<Vec<TableRef>, CoreError> {
        let token = google::access_token(conn)?;
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
        let token = google::access_token(conn)?;
        let title = fetch_spreadsheet_title(&token, table_id)?;
        let rows = fetch_values(
            &token,
            table_id,
            &format!("A{HEADER_ROW}:{LAST_COLUMN}{FIRST_DATA_ROW}"),
        )?;
        let header = rows.first().cloned().unwrap_or_default();
        let first_data_row = rows.get(1).map(Vec::as_slice);
        Ok(TableSchema {
            source_id: source_id.to_string(),
            table_id: table_id.to_string(),
            name: title,
            fields: schema_from_rows(&header, first_data_row),
        })
    }

    fn read_table(
        &self,
        conn: &Connection,
        _source_id: &str,
        table_id: &str,
        options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        let token = google::access_token(conn)?;
        let header = self.fetch_header(&token, table_id)?;
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
        let rows = fetch_values(&token, table_id, &range)?;
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
        _source_id: &str,
        table_id: &str,
        records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let token = google::access_token(conn)?;
        let header = self.fetch_header(&token, table_id)?;
        if header.is_empty() {
            return Err(CoreError::InvalidInput(format!(
                "Spreadsheet {table_id} has no header row to map fields onto"
            )));
        }

        let values: Vec<Vec<Value>> = records
            .iter()
            .map(|fields| row_values(&header, fields))
            .collect();
        let url = format!(
            "{SHEETS_ENDPOINT}/{table_id}/values/A{HEADER_ROW}:append?valueInputOption={VALUE_INPUT_RAW}&insertDataOption=INSERT_ROWS"
        );
        let body = google::post_json(&token, &url, &json!({ "values": values }))?;

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
        _source_id: &str,
        table_id: &str,
        patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError> {
        if patches.is_empty() {
            return Ok(Vec::new());
        }
        let token = google::access_token(conn)?;
        let header = self.fetch_header(&token, table_id)?;
        if header.is_empty() {
            return Err(CoreError::InvalidInput(format!(
                "Spreadsheet {table_id} has no header row to map fields onto"
            )));
        }
        let current = self.read_all(&token, table_id, &header)?;

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
                "range": format!("A{row_number}"),
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

        let url = format!("{SHEETS_ENDPOINT}/{table_id}/values:batchUpdate");
        google::post_json(
            &token,
            &url,
            &json!({ "valueInputOption": VALUE_INPUT_RAW, "data": data_entries }),
        )?;
        Ok(updated)
    }
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested without network access)
// ---------------------------------------------------------------------------

/// `values.get` for a range on the first visible sheet (no sheet prefix).
fn fetch_values(
    token: &str,
    spreadsheet_id: &str,
    range: &str,
) -> Result<Vec<Vec<Value>>, CoreError> {
    let url = format!("{SHEETS_ENDPOINT}/{spreadsheet_id}/values/{range}");
    let body = google::get_json(token, &url)?;
    let rows = body["values"].as_array().cloned().unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(|row| row.as_array().cloned().unwrap_or_default())
        .collect())
}

fn fetch_spreadsheet_title(token: &str, spreadsheet_id: &str) -> Result<String, CoreError> {
    let url = format!("{SHEETS_ENDPOINT}/{spreadsheet_id}?fields=properties.title");
    let body = google::get_json(token, &url)?;
    Ok(body["properties"]["title"]
        .as_str()
        .unwrap_or(spreadsheet_id)
        .to_string())
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
}
