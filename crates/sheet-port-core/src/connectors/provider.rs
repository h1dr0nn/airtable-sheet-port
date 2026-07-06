//! Additional-provider connector stub. Error wording matches the TypeScript
//! stub verbatim.

use rusqlite::Connection;

use super::TableConnector;
use crate::error::CoreError;
use crate::types::{
    DataSource, JsonMap, ReadOptions, RecordPatch, SourceKind, TableRecord, TableRef, TableSchema,
};

#[derive(Default)]
pub struct ProviderConnector {
    // TODO: Load API key or OAuth token from secure desktop-owned storage.
    source_ids: Vec<String>,
}

impl ProviderConnector {
    pub fn new(source_ids: Vec<String>) -> Self {
        Self { source_ids }
    }
}

impl TableConnector for ProviderConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::Provider
    }

    fn list_sources(&self, _conn: &Connection) -> Result<Vec<DataSource>, CoreError> {
        Ok(self
            .source_ids
            .iter()
            .map(|id| DataSource {
                id: format!("provider:{id}"),
                kind: SourceKind::Provider,
                name: format!("Provider Source {id}"),
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
            "Provider connector TODO: discover tables after auth is implemented".to_string(),
        ))
    }

    fn describe_table(
        &self,
        _conn: &Connection,
        _source_id: &str,
        _table_id: &str,
    ) -> Result<TableSchema, CoreError> {
        Err(CoreError::Unsupported(
            "Provider connector TODO: map provider field metadata to TableSchema".to_string(),
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
            "Provider connector TODO: read records with pagination and rate limiting".to_string(),
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
            "Provider connector TODO: search/filter records safely".to_string(),
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
            "Provider connector TODO: create records after preview and policy approval".to_string(),
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
            "Provider connector TODO: update records after preview and policy approval".to_string(),
        ))
    }
}
