//! Serde models mirroring the TypeScript types in `@sheet-port/shared` and
//! the IPC contract in `docs/ipc.md`. All structs serialize with camelCase
//! field names so the frontend receives exactly the documented JSON shape.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub app_version: String,
    pub db_path: String,
    pub mcp_running: bool,
    pub mcp_pid: Option<i64>,
    pub mcp_last_seen: Option<String>,
    pub pending_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSource {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef {
    pub source_id: String,
    pub table_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub source_id: String,
    pub table_id: String,
    pub name: String,
    /// FieldSchema[] stored as JSON in `mock_tables.fields`; passed through verbatim.
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRecord {
    pub id: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePage {
    pub records: Vec<TableRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRuleRow {
    pub id: i64,
    pub source_id: String,
    pub table_id: Option<String>,
    pub read: bool,
    pub write: bool,
    pub delete_records: bool,
    pub require_confirmation_for: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePermissionRule {
    pub id: Option<i64>,
    pub source_id: String,
    pub table_id: Option<String>,
    pub read: bool,
    pub write: bool,
    pub delete_records: bool,
    pub require_confirmation_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    pub id: String,
    pub source_id: String,
    pub table_id: String,
    #[serde(rename = "type")]
    pub change_type: String,
    pub created_at: String,
    pub status: String,
    pub requires_confirmation: bool,
    /// Agent-visible diff; the internal `payload` column is NEVER read or returned.
    pub diff: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStatus {
    pub google_sheets: bool,
    pub provider: bool,
}
