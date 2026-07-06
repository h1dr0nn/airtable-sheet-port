//! Google Sheets connector stub. Real reads and writes arrive once OAuth is
//! implemented; error wording matches the TypeScript stub verbatim.

use rusqlite::Connection;

use super::TableConnector;
use crate::error::CoreError;
use crate::types::{
    DataSource, JsonMap, ReadOptions, RecordPatch, SourceKind, TableRecord, TableRef, TableSchema,
};

#[derive(Default)]
pub struct GoogleSheetsConnector {
    // TODO: Inject OAuth client and secure token lookup owned by the desktop app.
    spreadsheet_ids: Vec<String>,
}

impl GoogleSheetsConnector {
    pub fn new(spreadsheet_ids: Vec<String>) -> Self {
        Self { spreadsheet_ids }
    }
}

impl TableConnector for GoogleSheetsConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::GoogleSheets
    }

    fn list_sources(&self, _conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        Ok(self
            .spreadsheet_ids
            .iter()
            .map(|id| DataSource {
                id: format!("google_sheets:{id}"),
                kind: SourceKind::GoogleSheets,
                name: format!("Google Sheet {id}"),
                status: None,
            })
            .collect())
    }

    fn list_tables(
        &self,
        _conn: &Connection,
        _source_id: &str,
    ) -> Result<Vec<TableRef>, CoreError> {
        Err(CoreError::Unsupported(
            "Google Sheets connector TODO: discover sheet tabs/ranges after OAuth is implemented"
                .to_string(),
        ))
    }

    fn describe_table(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        Err(CoreError::Unsupported(
            "Google Sheets connector TODO: infer headers and schema from configured range"
                .to_string(),
        ))
    }

    fn read_table(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _options: ReadOptions,
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported(
            "Google Sheets connector TODO: read bounded values through googleapis".to_string(),
        ))
    }

    fn find_records(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _query: &str,
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported(
            "Google Sheets connector TODO: search records in fetched rows".to_string(),
        ))
    }

    fn append_records(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _records: &[JsonMap],
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported(
            "Google Sheets connector TODO: append rows after preview and policy approval"
                .to_string(),
        ))
    }

    fn update_records(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
        _patches: &[RecordPatch],
    ) -> Result<Vec<TableRecord>, CoreError> {
        Err(CoreError::Unsupported(
            "Google Sheets connector TODO: update mapped row ranges after preview and policy approval"
                .to_string(),
        ))
    }
}
