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

/// One coordinate-level cell write: the A1 column letter, the 1-based sheet
/// row, and the value to type into the cell (USER_ENTERED semantics: numbers
/// parse as numbers, a leading `=` becomes a formula). The escape hatch for
/// document-style sheets whose layout does not fit the row-1-header record
/// model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellWrite {
    /// A1 column letters (e.g. "E"), uppercase.
    pub column: String,
    /// 1-based sheet row (e.g. 48 for cell E48).
    pub row: i64,
    pub value: String,
}

impl CellWrite {
    /// The A1 cell reference, e.g. "E48".
    pub fn a1(&self) -> String {
        format!("{}{}", self.column, self.row)
    }
}

/// A resource created by committing a structural change (a new spreadsheet or a
/// new sheet tab). Returned on the [`CommitOutcome`](crate::changes::CommitOutcome)
/// so the agent gets the id/url of what it just created. Only the fields that
/// apply to the created resource are populated.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedResource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spreadsheet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_gid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
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
    Format,
    CreateSpreadsheet,
    CreateSheet,
    DeleteSheet,
}

impl WriteAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::BulkUpdate => "bulk_update",
            Self::Format => "format",
            Self::CreateSpreadsheet => "create_spreadsheet",
            Self::CreateSheet => "create_sheet",
            Self::DeleteSheet => "delete_sheet",
        }
    }

    /// Whether this action needs the `delete_records` permission (the Bypass
    /// preset). Deleting a whole sheet tab is gated like a record delete, so
    /// auto-approve alone never authorizes it.
    pub fn needs_delete_permission(self) -> bool {
        matches!(self, Self::Delete | Self::DeleteSheet)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Append,
    Update,
    Delete,
    Format,
    #[serde(rename = "update_cells")]
    UpdateCells,
    #[serde(rename = "create_spreadsheet")]
    CreateSpreadsheet,
    #[serde(rename = "create_sheet")]
    CreateSheet,
    #[serde(rename = "delete_sheet")]
    DeleteSheet,
}

impl ChangeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Format => "format",
            Self::UpdateCells => "update_cells",
            Self::CreateSpreadsheet => "create_spreadsheet",
            Self::CreateSheet => "create_sheet",
            Self::DeleteSheet => "delete_sheet",
        }
    }

    pub fn from_db(raw: &str) -> Option<Self> {
        match raw {
            "append" => Some(Self::Append),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            "format" => Some(Self::Format),
            "update_cells" => Some(Self::UpdateCells),
            "create_spreadsheet" => Some(Self::CreateSpreadsheet),
            "create_sheet" => Some(Self::CreateSheet),
            "delete_sheet" => Some(Self::DeleteSheet),
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

// ---------------------------------------------------------------------------
// Cell formatting (docs/mcp-tools.md "Formatting"). A FormatPlan is the whole
// staged formatting change: any subset of per-range cell formats, a header
// freeze, and column widths. It is both the internal change payload and the
// agent-visible diff, so every field is a plain, non-sensitive value.
// ---------------------------------------------------------------------------

/// Border treatment applied to a range: no lines, all inner+outer gridlines,
/// the outer frame only, or a single bottom rule (the common header underline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    None,
    All,
    Outer,
    Bottom,
}

impl BorderStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::All => "all",
            Self::Outer => "outer",
            Self::Bottom => "bottom",
        }
    }

    /// Parses a border keyword; the allow-list is enforced at the tool boundary,
    /// so `None` here means an out-of-contract value slipped through.
    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "none" => Some(Self::None),
            "all" => Some(Self::All),
            "outer" => Some(Self::Outer),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// Google Sheets number-format category. Paired with a pattern; when omitted
/// the connector infers it from the pattern (date tokens -> DATE, else NUMBER).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NumberFormatType {
    Text,
    Number,
    Percent,
    Currency,
    Date,
    Time,
    DateTime,
    Scientific,
}

impl NumberFormatType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Number => "NUMBER",
            Self::Percent => "PERCENT",
            Self::Currency => "CURRENCY",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::DateTime => "DATE_TIME",
            Self::Scientific => "SCIENTIFIC",
        }
    }

    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "TEXT" => Some(Self::Text),
            "NUMBER" => Some(Self::Number),
            "PERCENT" => Some(Self::Percent),
            "CURRENCY" => Some(Self::Currency),
            "DATE" => Some(Self::Date),
            "TIME" => Some(Self::Time),
            "DATE_TIME" => Some(Self::DateTime),
            "SCIENTIFIC" => Some(Self::Scientific),
            _ => None,
        }
    }
}

/// Horizontal text alignment for a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}

impl HorizontalAlignment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Center => "CENTER",
            Self::Right => "RIGHT",
        }
    }

    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "LEFT" => Some(Self::Left),
            "CENTER" => Some(Self::Center),
            "RIGHT" => Some(Self::Right),
            _ => None,
        }
    }
}

/// One cell-format operation over an A1 range within the resolved tab. Every
/// optional field left unset is preserved on the sheet (partial formatting):
/// only the properties present here are written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellFormat {
    /// A1 range within the tab, e.g. `A1:D1`, `B:B`, or `A1`.
    pub range: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<i64>,
    /// `#rrggbb` text color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    /// `#rrggbb` cell fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_alignment: Option<HorizontalAlignment>,
    /// Google Sheets number-format pattern, e.g. `#,##0` or `yyyy-mm-dd`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    /// Category for `number_format`; inferred from the pattern when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format_type: Option<NumberFormatType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<BorderStyle>,
}

/// A single column-width override (pixels), keyed by A1 column letter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnWidth {
    pub column: String,
    pub pixels: i64,
}

/// A staged formatting change: any mix of per-range cell formats, a header
/// freeze, and column widths. Serialized verbatim as the agent-visible diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatPlan {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<CellFormat>,
    /// Freeze this many top rows (0 unfreezes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_rows: Option<i64>,
    /// Freeze this many left columns (0 unfreezes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_columns: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_widths: Vec<ColumnWidth>,
}

impl FormatPlan {
    /// A plan is empty when it would produce no batchUpdate request at all.
    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
            && self.freeze_rows.is_none()
            && self.freeze_columns.is_none()
            && self.column_widths.is_empty()
    }
}

/// The effective format of one cell in a style read, keyed by A1 column letter.
/// Only properties actually set on the cell are present.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellStyle {
    pub column: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizontal_alignment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
}

/// The existing style of a tab: sheet-level freeze/width plus the effective
/// format of the header row and the first data row, so an agent can match an
/// existing sheet's look instead of imposing a new one.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStyle {
    pub spreadsheet_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_title: Option<String>,
    pub frozen_row_count: i64,
    pub frozen_column_count: i64,
    /// Number of used columns the style covers (header width).
    pub column_count: i64,
    /// Effective format of each used cell in sheet row 1 (the header row).
    pub header: Vec<CellStyle>,
    /// Effective format of each used cell in sheet row 2 (first data row);
    /// empty when the sheet has no data row.
    pub sample: Vec<CellStyle>,
    pub column_widths: Vec<ColumnWidth>,
}
