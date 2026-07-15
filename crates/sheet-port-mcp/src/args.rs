//! Tool input models and bounds validation (docs/mcp-tools.md). Deserializing
//! covers types; every numeric/length bound is enforced here manually so an
//! out-of-range value surfaces as a tool error with a clear message instead
//! of a raw schema failure.

use schemars::JsonSchema;
use serde::Deserialize;
use sheet_port_core::connectors::validate_a1_range;
use sheet_port_core::constants::{
    AUDIT_LIMIT_DEFAULT, AUDIT_LIMIT_MAX, COLUMN_WIDTH_MAX, COLUMN_WIDTH_MIN, FIND_QUERY_MAX_LEN,
    FONT_SIZE_MAX, FONT_SIZE_MIN, FORMAT_OPS_MAX, FREEZE_MAX, READ_LIMIT_DEFAULT, READ_LIMIT_MAX,
    READ_LIMIT_MIN, WRITE_BATCH_MAX,
};
use sheet_port_core::types::{
    BorderStyle, CellFormat, ColumnWidth, FormatPlan, HorizontalAlignment, JsonMap,
    NumberFormatType,
};
use sheet_port_core::CoreError;

/// Matches the audit module's own lower bound (kept private there).
const AUDIT_LIMIT_MIN: i64 = 1;
const FIND_QUERY_MIN_LEN: usize = 1;
const WRITE_BATCH_MIN: usize = 1;
/// Upper bound on a number-format pattern so a plan cannot smuggle a huge blob.
const NUMBER_FORMAT_MAX_LEN: usize = 60;

fn invalid(message: String) -> CoreError {
    CoreError::InvalidInput(message)
}

fn require_non_empty(value: &str, field: &str) -> Result<(), CoreError> {
    if value.is_empty() {
        return Err(invalid(format!("{field} must be a non-empty string")));
    }
    Ok(())
}

fn require_batch_size(len: usize, field: &str) -> Result<(), CoreError> {
    if !(WRITE_BATCH_MIN..=WRITE_BATCH_MAX).contains(&len) {
        return Err(invalid(format!(
            "{field} must contain between {WRITE_BATCH_MIN} and {WRITE_BATCH_MAX} items"
        )));
    }
    Ok(())
}

fn bounded_limit(limit: Option<i64>, default: i64, min: i64, max: i64) -> Result<i64, CoreError> {
    let limit = limit.unwrap_or(default);
    if !(min..=max).contains(&limit) {
        return Err(invalid(format!(
            "limit must be an integer between {min} and {max}"
        )));
    }
    Ok(limit)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListTablesArgs {
    pub source_id: String,
}

impl ListTablesArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceTableArgs {
    pub source_id: String,
    pub table_id: String,
}

impl SourceTableArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadTableArgs {
    pub source_id: String,
    pub table_id: String,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

impl ReadTableArgs {
    /// Returns the effective `(limit, offset)` after defaults and bounds.
    pub fn validate(&self) -> Result<(i64, i64), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        let limit = bounded_limit(
            self.limit,
            READ_LIMIT_DEFAULT,
            READ_LIMIT_MIN,
            READ_LIMIT_MAX,
        )?;
        let offset = self.offset.unwrap_or(0);
        if offset < 0 {
            return Err(invalid("offset must be an integer >= 0".to_string()));
        }
        Ok((limit, offset))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindRecordsArgs {
    pub source_id: String,
    pub table_id: String,
    pub query: String,
}

impl FindRecordsArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        let length = self.query.chars().count();
        if !(FIND_QUERY_MIN_LEN..=FIND_QUERY_MAX_LEN).contains(&length) {
            return Err(invalid(format!(
                "query must be between {FIND_QUERY_MIN_LEN} and {FIND_QUERY_MAX_LEN} characters"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchArg {
    pub record_id: String,
    pub fields: JsonMap,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewUpdateArgs {
    pub source_id: String,
    pub table_id: String,
    pub patches: Vec<PatchArg>,
}

impl PreviewUpdateArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        require_batch_size(self.patches.len(), "patches")?;
        for (index, patch) in self.patches.iter().enumerate() {
            require_non_empty(&patch.record_id, &format!("patches[{index}].recordId"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppendRecordsArgs {
    pub source_id: String,
    pub table_id: String,
    pub records: Vec<JsonMap>,
    /// Optional formatting (same fields as preview_format_table) applied in the
    /// same commit as the append, so a fresh table is written and styled at once.
    #[serde(flatten)]
    pub format: FormatSpec,
}

impl AppendRecordsArgs {
    /// Validates the append and returns the bundled format plan when the caller
    /// supplied any formatting, or `None` for a plain append.
    pub fn validate(&self) -> Result<Option<FormatPlan>, CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        require_batch_size(self.records.len(), "records")?;
        if self.format.is_present() {
            Ok(Some(self.format.to_plan()?))
        } else {
            Ok(None)
        }
    }
}

/// One cell-format operation as received from an agent; strings are validated
/// and mapped onto the typed [`CellFormat`] in [`FormatTableArgs::validate`].
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CellFormatArg {
    pub range: String,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    #[serde(default)]
    pub font_size: Option<i64>,
    #[serde(default)]
    pub font_color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub horizontal_alignment: Option<String>,
    #[serde(default)]
    pub number_format: Option<String>,
    #[serde(default)]
    pub number_format_type: Option<String>,
    #[serde(default)]
    pub wrap: Option<bool>,
    #[serde(default)]
    pub border: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ColumnWidthArg {
    pub column: String,
    pub pixels: i64,
}

/// The formatting fields shared by `preview_format_table` and the optional
/// formatting bundled into `append_records`. Flattened into both arg structs so
/// the wire shape stays identical in either place.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatSpec {
    #[serde(default)]
    pub formats: Vec<CellFormatArg>,
    #[serde(default)]
    pub freeze_rows: Option<i64>,
    #[serde(default)]
    pub freeze_columns: Option<i64>,
    #[serde(default)]
    pub column_widths: Vec<ColumnWidthArg>,
}

impl FormatSpec {
    /// Whether the caller supplied any formatting at all (used to decide if an
    /// append carries a bundled plan).
    pub fn is_present(&self) -> bool {
        !self.formats.is_empty()
            || self.freeze_rows.is_some()
            || self.freeze_columns.is_some()
            || !self.column_widths.is_empty()
    }

    /// Validates every bound and enum, then returns the typed [`FormatPlan`] the
    /// staged-change layer stores. Rejects an empty plan (nothing to format).
    pub fn to_plan(&self) -> Result<FormatPlan, CoreError> {
        if self.formats.len() > FORMAT_OPS_MAX {
            return Err(invalid(format!(
                "formats must contain at most {FORMAT_OPS_MAX} items"
            )));
        }
        if self.column_widths.len() > FORMAT_OPS_MAX {
            return Err(invalid(format!(
                "columnWidths must contain at most {FORMAT_OPS_MAX} items"
            )));
        }
        validate_freeze(self.freeze_rows, "freezeRows")?;
        validate_freeze(self.freeze_columns, "freezeColumns")?;

        let formats = self
            .formats
            .iter()
            .enumerate()
            .map(|(index, format)| convert_cell_format(index, format))
            .collect::<Result<Vec<_>, _>>()?;
        let column_widths = self
            .column_widths
            .iter()
            .enumerate()
            .map(|(index, width)| convert_column_width(index, width))
            .collect::<Result<Vec<_>, _>>()?;

        let plan = FormatPlan {
            formats,
            freeze_rows: self.freeze_rows,
            freeze_columns: self.freeze_columns,
            column_widths,
        };
        if plan.is_empty() {
            return Err(invalid(
                "a formatting change must set at least one of formats, freezeRows, \
                 freezeColumns, or columnWidths"
                    .to_string(),
            ));
        }
        Ok(plan)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatTableArgs {
    pub source_id: String,
    pub table_id: String,
    #[serde(flatten)]
    pub format: FormatSpec,
}

impl FormatTableArgs {
    /// Validates ids and returns the typed [`FormatPlan`]; rejects an empty plan.
    pub fn validate(&self) -> Result<FormatPlan, CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        self.format.to_plan()
    }
}

fn validate_freeze(value: Option<i64>, field: &str) -> Result<(), CoreError> {
    if let Some(value) = value {
        if !(0..=FREEZE_MAX).contains(&value) {
            return Err(invalid(format!(
                "{field} must be between 0 and {FREEZE_MAX}"
            )));
        }
    }
    Ok(())
}

fn convert_cell_format(index: usize, arg: &CellFormatArg) -> Result<CellFormat, CoreError> {
    let field = |name: &str| format!("formats[{index}].{name}");
    require_non_empty(&arg.range, &field("range"))?;
    validate_a1_range(&arg.range)?;
    if let Some(size) = arg.font_size {
        if !(FONT_SIZE_MIN..=FONT_SIZE_MAX).contains(&size) {
            return Err(invalid(format!(
                "{} must be between {FONT_SIZE_MIN} and {FONT_SIZE_MAX}",
                field("fontSize")
            )));
        }
    }
    if let Some(pattern) = &arg.number_format {
        let length = pattern.chars().count();
        if !(1..=NUMBER_FORMAT_MAX_LEN).contains(&length) {
            return Err(invalid(format!(
                "{} must be 1 to {NUMBER_FORMAT_MAX_LEN} characters",
                field("numberFormat")
            )));
        }
    }
    let font_color = arg
        .font_color
        .as_deref()
        .map(|color| require_hex_color(color, &field("fontColor")))
        .transpose()?;
    let background_color = arg
        .background_color
        .as_deref()
        .map(|color| require_hex_color(color, &field("backgroundColor")))
        .transpose()?;
    let horizontal_alignment = arg
        .horizontal_alignment
        .as_deref()
        .map(|value| parse_alignment(value, &field("horizontalAlignment")))
        .transpose()?;
    let number_format_type = arg
        .number_format_type
        .as_deref()
        .map(|value| parse_number_format_type(value, &field("numberFormatType")))
        .transpose()?;
    let border = arg
        .border
        .as_deref()
        .map(|value| parse_border(value, &field("border")))
        .transpose()?;

    Ok(CellFormat {
        range: arg.range.clone(),
        bold: arg.bold,
        italic: arg.italic,
        font_size: arg.font_size,
        font_color,
        background_color,
        horizontal_alignment,
        number_format: arg.number_format.clone(),
        number_format_type,
        wrap: arg.wrap,
        border,
    })
}

fn convert_column_width(index: usize, arg: &ColumnWidthArg) -> Result<ColumnWidth, CoreError> {
    let field = |name: &str| format!("columnWidths[{index}].{name}");
    require_non_empty(&arg.column, &field("column"))?;
    if !(COLUMN_WIDTH_MIN..=COLUMN_WIDTH_MAX).contains(&arg.pixels) {
        return Err(invalid(format!(
            "{} must be between {COLUMN_WIDTH_MIN} and {COLUMN_WIDTH_MAX}",
            field("pixels")
        )));
    }
    Ok(ColumnWidth {
        column: arg.column.clone(),
        pixels: arg.pixels,
    })
}

/// Accepts a `#rrggbb` color (case-insensitive), echoing back the lowercase
/// form so stored plans are normalized.
fn require_hex_color(value: &str, field: &str) -> Result<String, CoreError> {
    let valid = value
        .strip_prefix('#')
        .is_some_and(|digits| digits.len() == 6 && digits.chars().all(|ch| ch.is_ascii_hexdigit()));
    if !valid {
        return Err(invalid(format!("{field} must be a #rrggbb hex color")));
    }
    Ok(value.to_ascii_lowercase())
}

fn parse_alignment(value: &str, field: &str) -> Result<HorizontalAlignment, CoreError> {
    HorizontalAlignment::from_wire(&value.to_ascii_uppercase())
        .ok_or_else(|| invalid(format!("{field} must be one of LEFT, CENTER, RIGHT")))
}

fn parse_number_format_type(value: &str, field: &str) -> Result<NumberFormatType, CoreError> {
    NumberFormatType::from_wire(&value.to_ascii_uppercase()).ok_or_else(|| {
        invalid(format!(
            "{field} must be one of TEXT, NUMBER, PERCENT, CURRENCY, DATE, TIME, DATE_TIME, SCIENTIFIC"
        ))
    })
}

fn parse_border(value: &str, field: &str) -> Result<BorderStyle, CoreError> {
    BorderStyle::from_wire(&value.to_ascii_lowercase())
        .ok_or_else(|| invalid(format!("{field} must be one of none, all, outer, bottom")))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitChangeArgs {
    /// Single change to commit (back-compat form; returns one outcome).
    #[serde(default)]
    pub change_id: Option<String>,
    /// Several changes to commit in one call (returns an array of outcomes).
    #[serde(default)]
    pub change_ids: Option<Vec<String>>,
}

impl CommitChangeArgs {
    /// The ordered list of change ids to commit, merging the singular and plural
    /// forms. Rejects when neither is given, an id is empty, or the batch is
    /// larger than [`WRITE_BATCH_MAX`].
    pub fn ids(&self) -> Result<Vec<String>, CoreError> {
        let mut ids = Vec::new();
        if let Some(change_id) = &self.change_id {
            require_non_empty(change_id, "changeId")?;
            ids.push(change_id.clone());
        }
        if let Some(change_ids) = &self.change_ids {
            for (index, change_id) in change_ids.iter().enumerate() {
                require_non_empty(change_id, &format!("changeIds[{index}]"))?;
                ids.push(change_id.clone());
            }
        }
        if ids.is_empty() {
            return Err(invalid(
                "provide changeId (single) or changeIds (batch)".to_string(),
            ));
        }
        if ids.len() > WRITE_BATCH_MAX {
            return Err(invalid(format!(
                "changeIds must contain at most {WRITE_BATCH_MAX} items"
            )));
        }
        Ok(ids)
    }

    /// True when the caller used the plural `changeIds` form, which returns an
    /// array of outcomes; the singular form returns a single outcome object.
    pub fn is_batch(&self) -> bool {
        self.change_ids.as_ref().is_some_and(|ids| !ids.is_empty())
    }
}

/// Max length of a spreadsheet or sheet-tab title an agent may request.
const TITLE_MAX_LEN: usize = 200;

fn require_title(title: &str) -> Result<(), CoreError> {
    let length = title.chars().count();
    if !(1..=TITLE_MAX_LEN).contains(&length) {
        return Err(invalid(format!(
            "title must be between 1 and {TITLE_MAX_LEN} characters"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpreadsheetArgs {
    pub source_id: String,
    pub title: String,
}

impl CreateSpreadsheetArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_title(&self.title)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSheetArgs {
    pub source_id: String,
    pub table_id: String,
    pub title: String,
}

impl CreateSheetArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")?;
        require_title(&self.title)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSheetArgs {
    pub source_id: String,
    pub table_id: String,
}

impl DeleteSheetArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty(&self.source_id, "sourceId")?;
        require_non_empty(&self.table_id, "tableId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetAuditLogArgs {
    #[serde(default)]
    pub limit: Option<i64>,
}

impl GetAuditLogArgs {
    /// Returns the effective limit after the default and bounds.
    pub fn validate(&self) -> Result<i64, CoreError> {
        bounded_limit(
            self.limit,
            AUDIT_LIMIT_DEFAULT,
            AUDIT_LIMIT_MIN,
            AUDIT_LIMIT_MAX,
        )
    }
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
