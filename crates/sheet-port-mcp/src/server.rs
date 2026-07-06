//! rmcp glue: registers exactly the 9 contract tools (docs/mcp-tools.md) and
//! the server identity. All behavior lives in `tools`; this layer only maps
//! results onto the MCP wire shape.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use sheet_port_core::CoreError;

use crate::args::{
    AppendRecordsArgs, CommitChangeArgs, FindRecordsArgs, GetAuditLogArgs, ListTablesArgs,
    PreviewUpdateArgs, ReadTableArgs, SourceTableArgs,
};
use crate::state::BrokerState;
use crate::tools;

/// Server identity agents see in `initialize`; matches the TypeScript sidecar.
const SERVER_NAME: &str = "sheet-port";
const SERVER_VERSION: &str = "0.3.0";

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
        description = "List connected data sources.",
        annotations(read_only_hint = true)
    )]
    async fn list_sources(&self) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_sources(&state)).await
    }

    #[tool(
        name = "list_tables",
        description = "List tables for a data source.",
        annotations(read_only_hint = true)
    )]
    async fn list_tables(&self, Parameters(args): Parameters<ListTablesArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::list_tables(&state, &args)).await
    }

    #[tool(
        name = "describe_table",
        description = "Describe a table schema.",
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
        description = "Read bounded records from a table.",
        annotations(read_only_hint = true)
    )]
    async fn read_table(&self, Parameters(args): Parameters<ReadTableArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::read_table(&state, &args)).await
    }

    #[tool(
        name = "find_records",
        description = "Find records by text query.",
        annotations(read_only_hint = true)
    )]
    async fn find_records(&self, Parameters(args): Parameters<FindRecordsArgs>) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::find_records(&state, &args)).await
    }

    #[tool(
        name = "preview_update_records",
        description = "Create a pending update change and return its diff."
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
        description = "Create a pending append change and return its diff."
    )]
    async fn append_records(
        &self,
        Parameters(args): Parameters<AppendRecordsArgs>,
    ) -> CallToolResult {
        let state = Arc::clone(&self.state);
        respond_blocking(move || tools::append_records(&state, args)).await
    }

    #[tool(
        name = "commit_change",
        description = "Commit a pending change after policy checks."
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
        description = "Return recent audit events.",
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
        info
    }
}
