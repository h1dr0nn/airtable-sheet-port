//! rmcp glue: registers the contract tools (docs/mcp-tools.md) and the server
//! identity. All behavior lives in `tools`; this layer only maps results onto
//! the MCP wire shape. Tool descriptions stay short; the shared rules (tableId
//! forms, source routing, write semantics) live once in [`SERVER_INSTRUCTIONS`].

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use sheet_port_core::CoreError;

use crate::args::{
    AppendRecordsArgs, CommitChangeArgs, CreateSheetArgs, CreateSpreadsheetArgs, DeleteSheetArgs,
    FindRecordsArgs, FormatTableArgs, GetAuditLogArgs, ListTablesArgs, ReadCellsArgs,
    ReadTableArgs, SourceTableArgs, UpdateCellsArgs, UpdateRecordsArgs,
};
use crate::state::BrokerState;
use crate::tools;

/// Server identity agents see in `initialize`; matches the TypeScript sidecar.
const SERVER_NAME: &str = "sheet-port";
const SERVER_VERSION: &str = "2.1.0";

/// Guidance returned in `initialize`: the rules every tool shares, so each tool
/// description can stay to one or two sentences. Kept accurate to the
/// connector behavior (tableId resolution, bridge routing, direct writes).
const SERVER_INSTRUCTIONS: &str = "Airtable - Sheet Port reads and edits the user's Google Sheets. When the user mentions a spreadsheet or pastes a Sheets link, use these tools instead of guessing, and never fabricate spreadsheet contents.

tableId: pass a Google Sheets URL, a bare spreadsheet id, spreadsheetId:gid, or spreadsheetId:SheetName. A pasted link can be passed as is; the tab comes from its gid or the sheet name, and without a selector the first tab is used. list_sheets shows a spreadsheet's tabs.

sourceId: optional on every tool. Omit it and the call is routed to the connected bridge that can open the spreadsheet (list_sources shows the bridges). Pass it only to force a specific source. Access is configured in the desktop app; never ask the user for passwords or tokens.

Reading: the record tools (read_table, find_records, read_formulas, describe_table) treat row 1 as the header. When a sheet is document-style (banner rows, headers further down, totals, several blocks) use read_cells, which returns raw cells by A1 coordinate with real row numbers, and update_cells to write any single cell. Never tell the user a cell cannot be edited. Use read_formulas before overwriting cells that may hold formulas.

Writing: update_records, append_records, update_cells, format_table, create_spreadsheet, create_sheet, and delete_sheet apply immediately and return the committed change with its diff, plus records, created, or formatError when relevant. Review the diff against what you meant to write and fix anything wrong with a follow-up call. Pass dryRun: true to only stage a change; it then returns a changeId that commit_change applies later (changeIds commits several in one call). An empty tab is never a reason to refuse: append_records writes the field names as the header row. delete_sheet also needs confirm: true.

Locale: call list_sheets to learn the spreadsheet's locale before writing formulas or decimals. In comma-decimal locales (e.g. vi_VN, de_DE, fr_FR, pt_BR, es_ES, it_IT, ru_RU, id_ID, tr_TR) separate formula arguments with ; (=COUNTIF(D2:D9;\"Done\")) and write decimals with a comma (0,65); percentages like 65% and plain integers are safe everywhere. Values are written exactly as given, never rewritten.

Style: call get_table_style first when a sheet already has data or formatting, and match it. For a fresh sheet, freeze the header row, make it bold with a light neutral fill (such as #f3f4f6) and a thin bottom border, give numeric and date columns a consistent numberFormat, right-align numbers, and set columnWidths so nothing is clipped. Keep it restrained: one or two muted accents, no full gridlines, no loud fills. Pass format fields to append_records to write and style new data in one call.";

pub struct SheetPortServer {
    state: Arc<BrokerState>,
}

impl SheetPortServer {
    pub fn new(state: Arc<BrokerState>) -> Self {
        Self { state }
    }
}

/// Tool failures surface as MCP tool results with isError=true and the plain
/// message as text content (the TypeScript SDK behavior the e2e asserts);
/// they are never JSON-RPC protocol errors.
fn respond(result: Result<String, CoreError>) -> CallToolResult {
    match result {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
    }
}

/// Connector calls may perform blocking HTTP (Google Sheets), so every tool
/// body runs on `spawn_blocking` to keep the async runtime responsive. A
/// panicked or cancelled task is reported as a normal tool error.
async fn respond_blocking<F>(task: F) -> CallToolResult
where
    F: FnOnce() -> Result<String, CoreError> + Send + 'static,
{
    match tokio::task::spawn_blocking(task).await {
        Ok(result) => respond(result),
        Err(error) => CallToolResult::error(vec![ContentBlock::text(format!(
            "Tool task failed: {error}"
        ))]),
    }
}

#[tool_router]
impl SheetPortServer {
    #[tool(
        name = "list_sources",
        description = "List the connected sources (one per Google Apps Script bridge).",
        annotations(read_only_hint = true)
    )]
    async fn list_sources(&self) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_sources(&state)).await
    }

    #[tool(
        name = "list_tables",
        description = "List the spreadsheets (tables) in a source; each tableId is a spreadsheet id.",
        annotations(read_only_hint = true)
    )]
    async fn list_tables(&self, Parameters(args): Parameters<ListTablesArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_tables(&state, &args)).await
    }

    #[tool(
        name = "list_sheets",
        description = "List the tabs of a spreadsheet as {gid, title}, plus its locale and timeZone. Use a gid as spreadsheetId:gid to target that tab.",
        annotations(read_only_hint = true)
    )]
    async fn list_sheets(&self, Parameters(args): Parameters<SourceTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_sheets(&state, &args)).await
    }

    #[tool(
        name = "describe_table",
        description = "Describe a tab's fields (row 1 as the header) with inferred types.",
        annotations(read_only_hint = true)
    )]
    async fn describe_table(
        &self,
        Parameters(args): Parameters<SourceTableArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::describe_table(&state, &args)).await
    }

    #[tool(
        name = "read_table",
        description = "Read records from a tab, treating row 1 as the header. limit/offset page over data rows.",
        annotations(read_only_hint = true)
    )]
    async fn read_table(&self, Parameters(args): Parameters<ReadTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::read_table(&state, &args)).await
    }

    #[tool(
        name = "read_formulas",
        description = "Like read_table, but formula cells return their =... text instead of the computed value. Check it before overwriting cells that may be computed.",
        annotations(read_only_hint = true)
    )]
    async fn read_formulas(&self, Parameters(args): Parameters<ReadTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::read_formulas(&state, &args)).await
    }

    #[tool(
        name = "find_records",
        description = "Case-insensitive text search across every field of a tab's records.",
        annotations(read_only_hint = true)
    )]
    async fn find_records(&self, Parameters(args): Parameters<FindRecordsArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::find_records(&state, &args)).await
    }

    #[tool(
        name = "read_cells",
        description = "Read raw cells keyed by A1 column letter, each row with its real sheet row number and no header interpretation. Pass range (e.g. B40:F60, A:C, 5:9) to fetch only that window; limit/offset page within it. Use for document-style sheets read_table cannot see.",
        annotations(read_only_hint = true)
    )]
    async fn read_cells(&self, Parameters(args): Parameters<ReadCellsArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::read_cells(&state, &args)).await
    }

    #[tool(
        name = "get_table_style",
        description = "Read a tab's existing look: header and first-row cell styles, frozen rows/columns, and column widths. Call before format_table on a sheet that already has styling.",
        annotations(read_only_hint = true)
    )]
    async fn get_table_style(
        &self,
        Parameters(args): Parameters<SourceTableArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::get_table_style(&state, &args)).await
    }

    #[tool(
        name = "update_records",
        description = "Patch existing records by recordId (from read_table). Returns the before/after diff."
    )]
    async fn update_records(
        &self,
        Parameters(args): Parameters<UpdateRecordsArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::update_records(&state, args)).await
    }

    #[tool(
        name = "append_records",
        description = "Append records as rows; on an empty tab the field names become the header row. Optional format fields (as in format_table) are applied in the same write."
    )]
    async fn append_records(
        &self,
        Parameters(args): Parameters<AppendRecordsArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::append_records(&state, args)).await
    }

    #[tool(
        name = "update_cells",
        description = "Write individual cells by A1 reference (e.g. E48), typed as a user would: numbers stay numbers and a leading = makes a formula. Reaches any cell, including document-style sheets."
    )]
    async fn update_cells(&self, Parameters(args): Parameters<UpdateCellsArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::update_cells(&state, args)).await
    }

    #[tool(
        name = "format_table",
        description = "Format a tab: per-range bold, italic, fontSize, fontColor/backgroundColor (#rrggbb), horizontalAlignment, numberFormat, wrap, border, plus freezeRows, freezeColumns, columnWidths, validations (native dropdown list or checkbox) and conditionalFormats (which replace existing rules on intersecting ranges). Only the properties you set change."
    )]
    async fn format_table(&self, Parameters(args): Parameters<FormatTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::format_table(&state, args)).await
    }

    #[tool(
        name = "create_spreadsheet",
        description = "Create a new spreadsheet titled title. The result's created field carries its spreadsheetId and url."
    )]
    async fn create_spreadsheet(
        &self,
        Parameters(args): Parameters<CreateSpreadsheetArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::create_spreadsheet(&state, args)).await
    }

    #[tool(
        name = "create_sheet",
        description = "Add a tab titled title to the spreadsheet in tableId. The result's created field carries the new gid."
    )]
    async fn create_sheet(&self, Parameters(args): Parameters<CreateSheetArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::create_sheet(&state, args)).await
    }

    #[tool(
        name = "delete_sheet",
        description = "Delete the tab named by tableId. Destructive: needs confirm: true and the source's delete permission.",
        annotations(destructive_hint = true)
    )]
    async fn delete_sheet(&self, Parameters(args): Parameters<DeleteSheetArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::delete_sheet(&state, args)).await
    }

    #[tool(
        name = "commit_change",
        description = "Apply changes staged with dryRun: changeId returns one result like a direct write, changeIds returns {committed: [...]} in order."
    )]
    async fn commit_change(
        &self,
        Parameters(args): Parameters<CommitChangeArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::commit_change(&state, &args)).await
    }

    #[tool(
        name = "get_audit_log",
        description = "Recent audit events (reads, writes, commits), newest first.",
        annotations(read_only_hint = true)
    )]
    async fn get_audit_log(&self, Parameters(args): Parameters<GetAuditLogArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::get_audit_log(&state, &args)).await
    }
}

#[tool_handler]
impl ServerHandler for SheetPortServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info = Implementation::new(SERVER_NAME, SERVER_VERSION);
        info.instructions = Some(SERVER_INSTRUCTIONS.to_string());
        info
    }
}
