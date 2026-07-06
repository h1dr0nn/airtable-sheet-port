//! stdout carries the MCP stdio transport; every log line must go to stderr.
//! The line format matches the TypeScript sidecar so existing log tooling
//! keeps working: `[sheet-port-mcp] <ISO timestamp> <message>`.

use sheet_port_core::db::now_iso;

pub fn log(message: &str) {
    eprintln!("[sheet-port-mcp] {} {message}", now_iso());
}
