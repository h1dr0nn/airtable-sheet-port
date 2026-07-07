//! Serde models mirroring the TypeScript types in `@sheet-port/shared` and
//! the IPC contract in docs/ipc.md. All structs serialize with camelCase
//! field names so agents and the desktop frontend receive exactly the
//! documented JSON shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON object of field name -> value, the universal record-fields shape.
pub type JsonMap = serde_json::Map<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    GoogleSheets,
    Provider,
    Mock,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GoogleSheets => "google_sheets",
            Self::Provider => "provider",
            Self::Mock => "mock",
        }
    }

    /// Parses the `sources.kind` column (guarded by a CHECK constraint).
    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "google_sheets" => Some(Self::GoogleSheets),
            "provider" => Some(Self::Provider),
            "mock" => Some(Self::Mock),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSource {
    pub id: String,
    pub kind: SourceKind,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef {
    pub source_id: String,
    pub table_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub source_id: String,
    pub table_id: String,
    pub name: String,
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRecord {
    pub id: String,
    pub fields: JsonMap,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePage {
    pub records: Vec<TableRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPatch {
    pub record_id: String,
    pub fields: JsonMap,
}

/// Pagination options for connector reads; `None` limit means "no limit".
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadOptions {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// One tab (sheet) inside a spreadsheet, like a Google Sheets bottom tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetTab {
    /// Provider sheet id (the Google `gid`), stringified for the frontend.
    pub gid: String,
    pub title: String,
    /// Tab order, left to right.
    pub index: i64,
}

/// A grid column: a stable id (A1 column letter) plus the header title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GridColumn {
    pub id: String,
    pub title: String,
}

/// One grid row keyed by column id; every value is a string cell (v1). A
/// `BTreeMap` keeps the JSON object deterministic for tests.
pub type GridRow = std::collections::BTreeMap<String, String>;

/// A rectangular block of string cells for one sheet tab (v1: string cells).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridData {
    pub columns: Vec<GridColumn>,
    pub rows: Vec<GridRow>,
    /// Total data rows ignoring limit/offset.
    pub total_rows: i64,
}

/// Write action evaluated against permission rules; wider than [`ChangeType`]
/// because large updates escalate to `bulk_update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAction {
    Append,
    Update,
    Delete,
    BulkUpdate,
}

impl WriteAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::BulkUpdate => "bulk_update",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Append,
    Update,
    Delete,
}

impl ChangeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "append" => Some(Self::Append),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    Pending,
    Approved,
    Committed,
    Rejected,
}

impl ChangeStatus {
    pub const ALL: [Self; 4] = [
        Self::Pending,
        Self::Approved,
        Self::Committed,
        Self::Rejected,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Committed => "committed",
            Self::Rejected => "rejected",
        }
    }

    pub fn from_db(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == raw)
    }
}

/// 'user' when approved/rejected in the desktop app, 'policy' when the broker
/// auto-approved a change that needs no confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeDecider {
    User,
    Policy,
}

impl ChangeDecider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Policy => "policy",
        }
    }

    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "user" => Some(Self::User),
            "policy" => Some(Self::Policy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    pub id: String,
    pub source_id: String,
    pub table_id: String,
    #[serde(rename = "type")]
    pub change_type: ChangeType,
    pub created_at: String,
    pub status: ChangeStatus,
    pub requires_confirmation: bool,
    /// Agent-visible diff; the internal `payload` column is NEVER exposed.
    pub diff: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<ChangeDecider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditActor {
    User,
    Agent,
    System,
}

impl AuditActor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
            Self::System => "system",
        }
    }

    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "user" => Some(Self::User),
            "agent" => Some(Self::Agent),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub actor: AuditActor,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRuleRow {
    pub id: i64,
    pub source_id: String,
    /// None means the rule applies to the whole source; a table-specific rule
    /// wins over it. Serialized as null on purpose (existing IPC shape).
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
pub struct HeartbeatStatus {
    pub running: bool,
    pub pid: Option<i64>,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStatus {
    pub google_sheets: bool,
    pub provider: bool,
}
