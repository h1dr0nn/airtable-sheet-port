//! Connector abstraction and the registry that routes calls by source kind.
//! second connector for the same kind replaces the first.

mod google_sheets;
mod mock;
mod provider;

pub use google_sheets::GoogleSheetsConnector;
pub use mock::MockConnector;
pub use provider::ProviderConnector;

use rusqlite::Connection;

use crate::error::CoreError;
use crate::sources;
use crate::types::{
    DataSource, JsonMap, ReadOptions, RecordPatch, SourceKind, TableRecord, TableRef, TableSchema,
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
    /// Google Sheets and provider connectors join once their auth lands.
    pub fn with_default_connectors() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(MockConnector));
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

#[cfg(test)]
#[path = "connectors_tests.rs"]
mod tests;
