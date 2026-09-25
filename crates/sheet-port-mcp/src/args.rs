//! Tool input models and bounds validation (docs/mcp-tools.md). Deserializing
//! covers types; every numeric/length bound is enforced here manually so an
//! out-of-range value surfaces as a tool error with a clear message instead
//! of a raw schema failure.

use schemars::JsonSchema;
use serde::Deserialize;
use sheet_port_core::connectors::{
    parse_cell_ref, parse_cells_range, validate_a1_range, A1Range, STYLE_HEADER_ROW_MAX,
};
use sheet_port_core::constants::{
    AUDIT_LIMIT_DEFAULT, AUDIT_LIMIT_MAX, COLUMN_WIDTH_MAX, COLUMN_WIDTH_MIN,
    CONDITIONAL_FORMATS_MAX, FIND_QUERY_MAX_LEN, FONT_SIZE_MAX, FONT_SIZE_MIN, FORMAT_OPS_MAX,
    FREEZE_MAX, READ_LIMIT_DEFAULT, READ_LIMIT_MAX, READ_LIMIT_MIN, VALIDATIONS_MAX,
    VALIDATION_LIST_VALUES_MAX, WRITE_BATCH_MAX,
};
use sheet_port_core::types::{
    BorderStyle, CellFormat, CellWrite, ColumnWidth, ConditionWhen, ConditionalFormat,
    DataValidation, FormatPlan, HorizontalAlignment, JsonMap, NumberFormatType, ValidationKind,
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

/// `sourceId` is optional everywhere (omitted = auto-routed), but an explicit
/// empty string is a caller bug rather than a request to auto-route.
fn require_source(source_id: Option<&str>) -> Result<(), CoreError> {
    match source_id {
        Some(source_id) => require_non_empty(source_id, "sourceId"),
        None => Ok(()),
    }
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

/// The read-page bounds shared by read_table, read_formulas, and read_cells.
fn read_window(limit: Option<i64>, offset: Option<i64>) -> Result<(i64, i64), CoreError> {
    let limit = bounded_limit(limit, READ_LIMIT_DEFAULT, READ_LIMIT_MIN, READ_LIMIT_MAX)?;
    let offset = offset.unwrap_or(0);
    if offset < 0 {
        return Err(invalid("offset must be an integer >= 0".to_string()));
    }
    Ok((limit, offset))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListTablesArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
}

impl ListTablesArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceTableArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
}

impl SourceTableArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadTableArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Max records to return (1-500, default 100).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Data rows to skip before the first returned record (default 0).
    #[serde(default)]
    pub offset: Option<i64>,
}

impl ReadTableArgs {
    /// Returns the effective `(limit, offset)` after defaults and bounds.
    pub fn validate(&self) -> Result<(i64, i64), CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")?;
        read_window(self.limit, self.offset)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadCellsArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Optional A1 window within the tab, like "B40:F60", "A:C", or "5:9" (no sheet name).
    #[serde(default)]
    pub range: Option<String>,
    /// Max rows to return (1-500, default 100).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Rows to skip within the window before the first returned row (default 0).
    #[serde(default)]
    pub offset: Option<i64>,
}

impl ReadCellsArgs {
    /// Returns the effective `(limit, offset)` plus the parsed window when a
    /// `range` was given.
    pub fn validate(&self) -> Result<(i64, i64, Option<A1Range>), CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")?;
        let range = self
            .range
            .as_deref()
            .map(|range| {
                require_non_empty(range, "range")?;
                parse_cells_range(range)
            })
            .transpose()?;
        let (limit, offset) = read_window(self.limit, self.offset)?;
        Ok((limit, offset, range))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindRecordsArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Text to look for (case-insensitive, matched against every field).
    pub query: String,
}

impl FindRecordsArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())?;
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
pub struct GetTableStyleArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// 1-based row holding the header (default 1); the sample is the next row.
    /// Set it for document-style sheets whose table starts lower, e.g. 9.
    #[serde(default)]
    pub header_row: Option<i64>,
}

impl GetTableStyleArgs {
    /// Returns the effective 1-based header row after the default and bounds.
    pub fn validate(&self) -> Result<i64, CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")?;
        let header_row = self.header_row.unwrap_or(1);
        if !(1..=STYLE_HEADER_ROW_MAX).contains(&header_row) {
            return Err(invalid(format!(
                "headerRow must be between 1 and {STYLE_HEADER_ROW_MAX}"
            )));
        }
        Ok(header_row)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchArg {
    /// recordId of the row to patch, as returned by read_table or find_records.
    pub record_id: String,
    /// Field name to new value; only the fields listed change.
    pub fields: JsonMap,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRecordsArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Record patches to apply (1-100).
    pub patches: Vec<PatchArg>,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl UpdateRecordsArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())?;
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
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Rows to append, each an object of field name to value (1-100). On an
    /// empty tab the field names become the header row.
    pub records: Vec<JsonMap>,
    // Optional formatting (same fields as format_table) applied in the same
    // commit as the append, so a fresh table is written and styled at once.
    #[serde(flatten)]
    pub format: FormatSpec,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl AppendRecordsArgs {
    /// Validates the append and returns the bundled format plan when the caller
    /// supplied any formatting, or `None` for a plain append.
    pub fn validate(&self) -> Result<Option<FormatPlan>, CoreError> {
        require_source(self.source_id.as_deref())?;
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
    /// A1 range within the tab, e.g. "A1:F1", "C2:C50", "B:B" (no sheet name).
    pub range: String,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    /// Font size in points.
    #[serde(default)]
    pub font_size: Option<i64>,
    /// Text color as #rrggbb.
    #[serde(default)]
    pub font_color: Option<String>,
    /// Fill color as #rrggbb.
    #[serde(default)]
    pub background_color: Option<String>,
    /// LEFT, CENTER, or RIGHT.
    #[serde(default)]
    pub horizontal_alignment: Option<String>,
    /// Number-format pattern, e.g. "#,##0.00", "0%", "yyyy-mm-dd".
    #[serde(default)]
    pub number_format: Option<String>,
    /// TEXT, NUMBER, PERCENT, CURRENCY, DATE, TIME, DATE_TIME, or SCIENTIFIC.
    #[serde(default)]
    pub number_format_type: Option<String>,
    /// true wraps long text; false lets it overflow.
    #[serde(default)]
    pub wrap: Option<bool>,
    /// none, all, outer, or bottom (thin grey lines).
    #[serde(default)]
    pub border: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ColumnWidthArg {
    /// Column letter, e.g. "C".
    pub column: String,
    /// Width in pixels.
    pub pixels: i64,
}

/// A native data-validation rule: a dropdown (`list`) or a checkbox.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationArg {
    /// A1 range within the tab, e.g. "D2:D100" or "E:E".
    pub range: String,
    /// "list" (dropdown of `values`) or "checkbox".
    #[serde(rename = "type")]
    pub kind: String,
    /// Dropdown options for type "list" (1 to 100 non-empty strings).
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// Reject input that fails the rule (default true); false only warns.
    #[serde(default)]
    pub strict: Option<bool>,
    /// Show the dropdown chip for type "list" (default true).
    #[serde(default)]
    pub show_dropdown: Option<bool>,
}

/// The condition of a conditional-format rule; set exactly one key.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConditionWhenArg {
    /// Cell text equals this.
    #[serde(default)]
    pub text_eq: Option<String>,
    /// Cell text contains this.
    #[serde(default)]
    pub text_contains: Option<String>,
    /// Number greater than this.
    #[serde(default)]
    pub number_gt: Option<f64>,
    /// Number less than this.
    #[serde(default)]
    pub number_lt: Option<f64>,
    /// Number between [low, high], inclusive.
    #[serde(default)]
    pub number_between: Option<[f64; 2]>,
    /// Cell is empty (must be true).
    #[serde(default)]
    pub blank: Option<bool>,
    /// Cell is not empty (must be true).
    #[serde(default)]
    pub not_blank: Option<bool>,
    /// Custom formula starting with "=", in the spreadsheet's locale syntax,
    /// relative to the range's top-left cell (e.g. =$E2="High").
    #[serde(default)]
    pub formula: Option<String>,
}

/// A conditional-format rule. It replaces the tab's existing rules on exactly
/// the same range (every intersecting rule with replaceIntersecting).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalFormatArg {
    /// A1 range within the tab, e.g. "D2:D100".
    pub range: String,
    /// The condition; set exactly one key.
    pub when: ConditionWhenArg,
    /// Fill color as #rrggbb when the condition holds.
    #[serde(default)]
    pub background_color: Option<String>,
    /// Text color as #rrggbb when the condition holds.
    #[serde(default)]
    pub font_color: Option<String>,
    /// Bold text when the condition holds.
    #[serde(default)]
    pub bold: Option<bool>,
}

/// The formatting fields shared by `format_table` and the optional
/// formatting bundled into `append_records`. Flattened into both arg structs so
/// the wire shape stays identical in either place.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatSpec {
    /// Per-range cell styles; only the properties you set change.
    #[serde(default)]
    pub formats: Vec<CellFormatArg>,
    /// Freeze this many top rows (0 unfreezes).
    #[serde(default)]
    pub freeze_rows: Option<i64>,
    /// Freeze this many left columns (0 unfreezes).
    #[serde(default)]
    pub freeze_columns: Option<i64>,
    /// Column widths in pixels.
    #[serde(default)]
    pub column_widths: Vec<ColumnWidthArg>,
    /// Native data validation: a dropdown (type "list" with values) or a
    /// checkbox on a range. Replaces any validation already on those cells.
    #[serde(default)]
    pub validations: Vec<ValidationArg>,
    /// Color rules (conditional formats): fill, text color, or bold when a
    /// condition holds. Each replaces existing rules on exactly the same range.
    #[serde(default)]
    pub conditional_formats: Vec<ConditionalFormatArg>,
    /// true makes conditionalFormats delete every existing rule whose range
    /// intersects a new rule's range (default false: exact range only).
    #[serde(default)]
    pub replace_intersecting: bool,
}

impl FormatSpec {
    /// Whether the caller supplied any formatting at all (used to decide if an
    /// append carries a bundled plan).
    pub fn is_present(&self) -> bool {
        !self.formats.is_empty()
            || self.freeze_rows.is_some()
            || self.freeze_columns.is_some()
            || !self.column_widths.is_empty()
            || !self.validations.is_empty()
            || !self.conditional_formats.is_empty()
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
        if self.validations.len() > VALIDATIONS_MAX {
            return Err(invalid(format!(
                "validations must contain at most {VALIDATIONS_MAX} items"
            )));
        }
        if self.conditional_formats.len() > CONDITIONAL_FORMATS_MAX {
            return Err(invalid(format!(
                "conditionalFormats must contain at most {CONDITIONAL_FORMATS_MAX} items"
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
        let validations = self
            .validations
            .iter()
            .enumerate()
            .map(|(index, validation)| convert_validation(index, validation))
            .collect::<Result<Vec<_>, _>>()?;
        let conditional_formats = self
            .conditional_formats
            .iter()
            .enumerate()
            .map(|(index, rule)| convert_conditional_format(index, rule))
            .collect::<Result<Vec<_>, _>>()?;

        let plan = FormatPlan {
            formats,
            freeze_rows: self.freeze_rows,
            freeze_columns: self.freeze_columns,
            column_widths,
            validations,
            conditional_formats,
            replace_intersecting: self.replace_intersecting,
        };
        if plan.is_empty() {
            return Err(invalid(
                "a formatting change must set at least one of formats, freezeRows, \
                 freezeColumns, columnWidths, validations, or conditionalFormats"
                    .to_string(),
            ));
        }
        Ok(plan)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatTableArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    // The formatting fields (formats, freezes, widths, validations,
    // conditionalFormats, replaceIntersecting), flattened to the top level.
    #[serde(flatten)]
    pub format: FormatSpec,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl FormatTableArgs {
    /// Validates ids and returns the typed [`FormatPlan`]; rejects an empty plan.
    pub fn validate(&self) -> Result<FormatPlan, CoreError> {
        require_source(self.source_id.as_deref())?;
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

fn convert_validation(index: usize, arg: &ValidationArg) -> Result<DataValidation, CoreError> {
    let field = |name: &str| format!("validations[{index}].{name}");
    require_non_empty(&arg.range, &field("range"))?;
    validate_a1_range(&arg.range)?;
    let kind = ValidationKind::from_wire(&arg.kind.to_ascii_lowercase())
        .ok_or_else(|| invalid(format!("{} must be one of list, checkbox", field("type"))))?;
    let values = match kind {
        ValidationKind::List => {
            let values = arg.values.clone().unwrap_or_default();
            if !(1..=VALIDATION_LIST_VALUES_MAX).contains(&values.len()) {
                return Err(invalid(format!(
                    "{} must contain between 1 and {VALIDATION_LIST_VALUES_MAX} items for type list",
                    field("values")
                )));
            }
            if let Some(position) = values.iter().position(|value| value.trim().is_empty()) {
                return Err(invalid(format!(
                    "{}[{position}] must be a non-empty string",
                    field("values")
                )));
            }
            values
        }
        ValidationKind::Checkbox => {
            if arg.values.is_some() {
                return Err(invalid(format!(
                    "{} applies only to type list",
                    field("values")
                )));
            }
            if arg.show_dropdown.is_some() {
                return Err(invalid(format!(
                    "{} applies only to type list",
                    field("showDropdown")
                )));
            }
            Vec::new()
        }
    };
    Ok(DataValidation {
        range: arg.range.clone(),
        kind,
        values,
        strict: arg.strict.unwrap_or(true),
        show_dropdown: match kind {
            ValidationKind::List => Some(arg.show_dropdown.unwrap_or(true)),
            ValidationKind::Checkbox => None,
        },
    })
}

fn convert_conditional_format(
    index: usize,
    arg: &ConditionalFormatArg,
) -> Result<ConditionalFormat, CoreError> {
    let field = |name: &str| format!("conditionalFormats[{index}].{name}");
    require_non_empty(&arg.range, &field("range"))?;
    validate_a1_range(&arg.range)?;
    let when = convert_condition(&arg.when, &field("when"))?;
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
    if font_color.is_none() && background_color.is_none() && arg.bold.is_none() {
        return Err(invalid(format!(
            "conditionalFormats[{index}] must set at least one of backgroundColor, fontColor, or bold"
        )));
    }
    Ok(ConditionalFormat {
        range: arg.range.clone(),
        when,
        background_color,
        font_color,
        bold: arg.bold,
    })
}

const CONDITION_KEYS: &str =
    "textEq, textContains, numberGt, numberLt, numberBetween, blank, notBlank, formula";

fn convert_condition(arg: &ConditionWhenArg, field: &str) -> Result<ConditionWhen, CoreError> {
    let when = ConditionWhen {
        text_eq: arg.text_eq.clone(),
        text_contains: arg.text_contains.clone(),
        number_gt: arg.number_gt,
        number_lt: arg.number_lt,
        number_between: arg.number_between,
        blank: arg.blank,
        not_blank: arg.not_blank,
        formula: arg.formula.clone(),
    };
    if when.set_count() != 1 {
        return Err(invalid(format!(
            "{field} must set exactly one of {CONDITION_KEYS}"
        )));
    }
    for (key, text) in [
        ("textEq", &when.text_eq),
        ("textContains", &when.text_contains),
    ] {
        if text.as_deref() == Some("") {
            return Err(invalid(format!("{field}.{key} must be a non-empty string")));
        }
    }
    let numbers = [when.number_gt, when.number_lt]
        .into_iter()
        .flatten()
        .chain(when.number_between.into_iter().flatten());
    for number in numbers {
        if !number.is_finite() {
            return Err(invalid(format!("{field} numbers must be finite")));
        }
    }
    if let Some([low, high]) = when.number_between {
        if low > high {
            return Err(invalid(format!(
                "{field}.numberBetween must be [low, high] with low <= high"
            )));
        }
    }
    if when.blank == Some(false) || when.not_blank == Some(false) {
        return Err(invalid(format!(
            "{field}.blank and {field}.notBlank must be true when set"
        )));
    }
    if let Some(formula) = &when.formula {
        if !formula.starts_with('=') || formula.len() < 2 {
            return Err(invalid(format!(
                "{field}.formula must be a formula starting with ="
            )));
        }
    }
    Ok(when)
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
/// Max characters of a single coordinate-level cell value (Sheets' own cell
/// limit is 50k characters).
const CELL_VALUE_MAX_LEN: usize = 50_000;

fn require_title(title: &str) -> Result<(), CoreError> {
    let length = title.chars().count();
    if !(1..=TITLE_MAX_LEN).contains(&length) {
        return Err(invalid(format!(
            "title must be between 1 and {TITLE_MAX_LEN} characters"
        )));
    }
    Ok(())
}

/// One agent-supplied cell write: an A1 reference plus the value to type.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CellWriteArg {
    /// A1 cell reference like "E48".
    pub cell: String,
    /// Value typed into the cell (USER_ENTERED: numbers parse as numbers, a
    /// leading `=` becomes a formula, anything else is text).
    pub value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCellsArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Cells to write (1-100).
    pub cells: Vec<CellWriteArg>,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl UpdateCellsArgs {
    /// Validates ids, batch bounds, and every cell reference, returning the
    /// typed writes the staged-change layer stores.
    pub fn validate(&self) -> Result<Vec<CellWrite>, CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")?;
        require_batch_size(self.cells.len(), "cells")?;
        self.cells
            .iter()
            .enumerate()
            .map(|(index, write)| {
                let (column, row) = parse_cell_ref(&write.cell)
                    .map_err(|error| invalid(format!("cells[{index}].cell: {error}")))?;
                if write.value.chars().count() > CELL_VALUE_MAX_LEN {
                    return Err(invalid(format!(
                        "cells[{index}].value must be at most {CELL_VALUE_MAX_LEN} characters"
                    )));
                }
                Ok(CellWrite {
                    column,
                    row,
                    value: write.value.clone(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpreadsheetArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Title of the new spreadsheet (1-200 characters).
    pub title: String,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl CreateSpreadsheetArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())?;
        require_title(&self.title)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSheetArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Title of the new tab (1-200 characters).
    pub title: String,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl CreateSheetArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")?;
        require_title(&self.title)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSheetArgs {
    /// Connected source to use. Omit to auto-route to the bridge that can open the spreadsheet.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Spreadsheet URL, id, id:gid, or id:SheetName.
    pub table_id: String,
    /// Must be true: deleting a tab is destructive.
    #[serde(default)]
    pub confirm: bool,
    /// true stages the change and returns its diff without applying it.
    #[serde(default)]
    pub dry_run: bool,
}

impl DeleteSheetArgs {
    pub fn validate(&self) -> Result<(), CoreError> {
        require_source(self.source_id.as_deref())?;
        require_non_empty(&self.table_id, "tableId")?;
        if !self.confirm {
            return Err(invalid("delete_sheet needs confirm: true".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetAuditLogArgs {
    /// Max events to return (1-500, default 100).
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
