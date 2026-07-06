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
const SERVER_VERSION: &str = "0.1.0";

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

#[tool_router]
impl SheetPortServer {
    #[tool(
        name = "list_sources",
        description = "List connected data sources.",
        annotations(read_only_hint = true)
    )]
    fn list_sources(&self) -> CallToolResult {
        respond(tools::list_sources(&self.state))
    }

    #[tool(
        name = "list_tables",
        description = "List tables for a data source.",
        annotations(read_only_hint = true)
    )]
    fn list_tables(&self, Parameters(args): Parameters<ListTablesArgs>) -> CallToolResult {
        respond(tools::list_tables(&self.state, &args))
    }

    #[tool(
        name = "describe_table",
        description = "Describe a table schema.",
        annotations(read_only_hint = true)
    )]
    fn describe_table(&self, Parameters(args): Parameters<SourceTableArgs>) -> CallToolResult {
        respond(tools::describe_table(&self.state, &args))
    }

    #[tool(
        name = "read_table",
        description = "Read bounded records from a table.",
        annotations(read_only_hint = true)
    )]
    fn read_table(&self, Parameters(args): Parameters<ReadTableArgs>) -> CallToolResult {
        respond(tools::read_table(&self.state, &args))
    }

    #[tool(
        name = "find_records",
        description = "Find records by text query.",
        annotations(read_only_hint = true)
    )]
    fn find_records(&self, Parameters(args): Parameters<FindRecordsArgs>) -> CallToolResult {
        respond(tools::find_records(&self.state, &args))
    }

    #[tool(
        name = "preview_update_records",
        description = "Create a pending update change and return its diff."
    )]
    fn preview_update_records(
        &self,
        Parameters(args): Parameters<PreviewUpdateArgs>,
    ) -> CallToolResult {
        respond(tools::preview_update_records(&self.state, args))
    }

    #[tool(
        name = "append_records",
        description = "Create a pending append change and return its diff."
    )]
    fn append_records(&self, Parameters(args): Parameters<AppendRecordsArgs>) -> CallToolResult {
        respond(tools::append_records(&self.state, args))
    }

    #[tool(
        name = "commit_change",
        description = "Commit a pending change after policy checks."
    )]
    fn commit_change(&self, Parameters(args): Parameters<CommitChangeArgs>) -> CallToolResult {
        respond(tools::commit_change(&self.state, &args))
    }

    #[tool(
        name = "get_audit_log",
        description = "Return recent audit events.",
        annotations(read_only_hint = true)
    )]
    fn get_audit_log(&self, Parameters(args): Parameters<GetAuditLogArgs>) -> CallToolResult {
        respond(tools::get_audit_log(&self.state, &args))
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
