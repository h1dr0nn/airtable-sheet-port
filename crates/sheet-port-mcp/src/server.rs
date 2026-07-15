//! rmcp glue: registers exactly the 11 contract tools (docs/mcp-tools.md) and
//! the server identity. All behavior lives in `tools`; this layer only maps
//! results onto the MCP wire shape.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use sheet_port_core::CoreError;

use crate::args::{
    AppendRecordsArgs, CommitChangeArgs, CreateSheetArgs, CreateSpreadsheetArgs, DeleteSheetArgs,
    FindRecordsArgs, FormatTableArgs, GetAuditLogArgs, ListTablesArgs, PreviewUpdateArgs,
    ReadTableArgs, SourceTableArgs,
};
use crate::state::BrokerState;
use crate::tools;

/// Server identity agents see in `initialize`; matches the TypeScript sidecar.
const SERVER_NAME: &str = "sheet-port";
const SERVER_VERSION: &str = "0.3.0";

/// Guidance returned in `initialize` so agents reliably choose these tools and
/// pass a pasted spreadsheet link straight through as the tableId. Kept
/// accurate to the connector behavior (URL / id / id:gid / id:SheetName
/// resolution, preview -> commit writes, tokens held by the desktop app).
const SERVER_INSTRUCTIONS: &str = "Airtable - Sheet Port exposes safe tools to read and edit the user's connected Google Sheets (and future providers). When the user mentions a Google Sheets link or a spreadsheet, use these tools instead of guessing: call list_sources to find the connected account, then read_table or find_records. read_formulas reads the same rows but keeps each cell's raw formula (its =... text) instead of the computed value, so check it before overwriting cells that may be computed and would otherwise be clobbered. read_table and the other table tools accept a Google Sheets URL, a bare spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName as the tableId - so you can pass a pasted spreadsheet link directly, and the exact tab is selected from the gid or sheet name (no selector reads the first tab). All writes are staged: preview_update_records, append_records, or preview_format_table return a changeId, then commit_change applies it (some changes need the user to approve in the desktop app first). To fill an empty or brand-new tab, call append_records directly: the field names of the records become the header row and the data is written beneath it, so an empty sheet is never a reason to refuse a write. When you already know the layout, pass the formatting fields to append_records so the data and its house-style styling apply in a single commit instead of a separate preview_format_table. commit_change also takes a changeIds array to commit several staged changes in one call, which saves round-trips when you have staged more than one. Two tools handle appearance: get_table_style reads a tab's existing look (header and first-row cell styles, frozen rows, column widths), and preview_format_table stages formatting - per-range bold, italic, fontSize, fontColor and backgroundColor as #rrggbb, horizontalAlignment, numberFormat (with an optional numberFormatType such as DATE or CURRENCY), wrap and border (none/all/outer/bottom), plus freezeRows, freezeColumns, and columnWidths. House style whenever you lay out a fresh sheet or write new data: freeze the header row, make the header bold with a light neutral fill (for example #f3f4f6) and a thin bottom border, give numeric and date columns a consistent numberFormat and right-align numbers, and set columnWidths so nothing is clipped. Keep it restrained - one or two muted accent colors, no full gridlines, no loud fills. When the sheet ALREADY has data or formatting, call get_table_style first and match its existing header and data styling instead of imposing a new look. Never fabricate spreadsheet contents; read them with these tools. Never ask for or handle OAuth tokens - the desktop app holds them.";

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
        description = "List the user's connected data sources (Google Sheets accounts and other providers). Call this first to get the sourceId for the connected Google account before reading or editing a spreadsheet.",
        annotations(read_only_hint = true)
    )]
    async fn list_sources(&self) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_sources(&state)).await
    }

    #[tool(
        name = "list_tables",
        description = "List the spreadsheets (tables) in a source. For a Google Sheets account each spreadsheet is one entry; the tableId is the spreadsheet id. To target a specific tab, pass a Google Sheets URL, spreadsheetId:gid, or spreadsheetId:SheetName as the tableId to the read tools instead.",
        annotations(read_only_hint = true)
    )]
    async fn list_tables(&self, Parameters(args): Parameters<ListTablesArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_tables(&state, &args)).await
    }

    #[tool(
        name = "describe_table",
        description = "Describe a table's field schema (column names and inferred types). For Google Sheets the tableId may be a Google Sheets URL, a bare spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName to pick a specific tab; without a selector the first tab is used.",
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
        description = "Read bounded records (rows) from a table. For Google Sheets the tableId may be a Google Sheets URL, a bare spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName to pick a specific tab; without a selector the first tab is used. Paste a spreadsheet link directly to read the exact sheet the user shared.",
        annotations(read_only_hint = true)
    )]
    async fn read_table(&self, Parameters(args): Parameters<ReadTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::read_table(&state, &args)).await
    }

    #[tool(
        name = "find_records",
        description = "Find records by case-insensitive text search across all field values. For Google Sheets the tableId may be a Google Sheets URL, a bare spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName to pick a specific tab; without a selector the first tab is used.",
        annotations(read_only_hint = true)
    )]
    async fn find_records(&self, Parameters(args): Parameters<FindRecordsArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::find_records(&state, &args)).await
    }

    #[tool(
        name = "read_formulas",
        description = "Read records like read_table but with each cell's raw formula preserved (a formula cell returns its `=...` text instead of the computed value). Call this before overwriting cells you suspect are computed, so you can see and keep the formula logic instead of clobbering it. Same tableId forms and paging as read_table.",
        annotations(read_only_hint = true)
    )]
    async fn read_formulas(&self, Parameters(args): Parameters<ReadTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::read_formulas(&state, &args)).await
    }

    #[tool(
        name = "get_table_style",
        description = "Read a tab's existing cell formatting so you can match it: the effective style (bold, colors, alignment, number format, wrap) of the header row and the first data row, plus frozen row/column counts and column pixel widths. Call this before preview_format_table when the sheet already has data or a look you should keep consistent. For Google Sheets the tableId may be a URL, a spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName.",
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
        name = "preview_update_records",
        description = "Stage an update to existing records and return its diff (before/after). This does NOT write anything: it returns a changeId you then pass to commit_change. Some changes require the user to approve them in the desktop app before commit_change will apply them. For Google Sheets the tableId may be a Google Sheets URL, a bare spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName to pick a specific tab."
    )]
    async fn preview_update_records(
        &self,
        Parameters(args): Parameters<PreviewUpdateArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::preview_update_records(&state, args)).await
    }

    #[tool(
        name = "append_records",
        description = "Stage new rows to append to a table and return the pending change. Works on an empty tab too: if the sheet has no header yet, the field names of the records you pass become the header row (row 1) and the records are written below it, so you can populate a blank sheet in one call - never refuse to write just because a sheet is empty. You may also pass formatting fields (the same formats, freezeRows, freezeColumns, and columnWidths as preview_format_table); when present they are applied in the SAME commit right after the rows land, so you can write and style a fresh table in one preview+commit instead of two - prefer this over a separate preview_format_table when you already know the layout. This does NOT write anything: it returns a changeId you then pass to commit_change. Some changes require the user to approve them in the desktop app before commit_change will apply them. For Google Sheets the tableId may be a Google Sheets URL, a bare spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName to pick a specific tab."
    )]
    async fn append_records(
        &self,
        Parameters(args): Parameters<AppendRecordsArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::append_records(&state, args)).await
    }

    #[tool(
        name = "preview_format_table",
        description = "Stage cell formatting for a tab and return the pending change. This does NOT write anything: it returns a changeId you then pass to commit_change (some changes require the user to approve them in the desktop app first). Provide any of: `formats` (a list of operations, each with a `range` like A1:D1 plus optional bold, italic, fontSize, fontColor and backgroundColor as #rrggbb, horizontalAlignment LEFT/CENTER/RIGHT, numberFormat pattern with optional numberFormatType, wrap, and border none/all/outer/bottom), `freezeRows`, `freezeColumns`, and `columnWidths` (per-column pixel sizes). Only the properties you set are changed. Call get_table_style first to match an existing sheet's look. For Google Sheets the tableId may be a URL, a spreadsheet id, or spreadsheetId:gid / spreadsheetId:SheetName."
    )]
    async fn preview_format_table(
        &self,
        Parameters(args): Parameters<FormatTableArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::preview_format_table(&state, args)).await
    }

    #[tool(
        name = "preview_create_spreadsheet",
        description = "Stage the creation of a brand-new spreadsheet titled `title` on the connected account and return the pending change. This does NOT create anything until commit_change; on commit the outcome's `created` carries the new spreadsheetId and url so you can write into it next. Needs source-wide write permission."
    )]
    async fn preview_create_spreadsheet(
        &self,
        Parameters(args): Parameters<CreateSpreadsheetArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::preview_create_spreadsheet(&state, args)).await
    }

    #[tool(
        name = "preview_create_sheet",
        description = "Stage adding a new sheet tab titled `title` to an existing spreadsheet (tableId = the spreadsheet URL or id) and return the pending change. Nothing is created until commit_change; on commit the outcome's `created` carries the new tab's gid. Needs write permission on the spreadsheet."
    )]
    async fn preview_create_sheet(
        &self,
        Parameters(args): Parameters<CreateSheetArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::preview_create_sheet(&state, args)).await
    }

    #[tool(
        name = "preview_delete_sheet",
        description = "Stage deleting a sheet tab (tableId = URL, spreadsheetId:gid, or spreadsheetId:SheetName) and return the pending change. Nothing is deleted until commit_change. This is destructive and needs the delete permission (the Bypass access preset); auto-approve alone never authorizes deleting a sheet, so if the source is not set to Bypass this is refused."
    )]
    async fn preview_delete_sheet(
        &self,
        Parameters(args): Parameters<DeleteSheetArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::preview_delete_sheet(&state, args)).await
    }

    #[tool(
        name = "commit_change",
        description = "Apply one or more changes previously staged by preview_update_records, append_records, or preview_format_table. Pass changeId to commit a single change (returns one outcome), or changeIds (an array) to commit several in one call (returns { committed: [...] }, one outcome per change in order) - use the batch form to save round-trips when you have staged multiple changes. This is the only tool that writes to a spreadsheet. If a change requires confirmation and the user has not approved it in the desktop app, the commit is refused with an error telling you to ask the user to approve."
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
        description = "Return recent audit events (reads, previews, commits, and user approvals), newest first, so you can review what has been done in this workspace.",
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
