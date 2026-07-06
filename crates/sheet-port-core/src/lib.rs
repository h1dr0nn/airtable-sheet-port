//! Airtable - Sheet Port broker core.
//!
//! Owns every behavior of the local permission broker: shared SQLite state
//! (schema + seed under sql/), permission rules, pending-change lifecycle,
//! audit log, connectors, MCP heartbeat, and keychain token status. The Tauri
//! desktop shell and the MCP server are thin wrappers over this crate.
//! error wording is part of the contract (docs/mcp-tools.md, docs/ipc.md).

pub mod audit;
pub mod changes;
pub mod connectors;
pub mod constants;
pub mod db;
pub mod error;
pub mod google;
pub mod heartbeat;
pub mod mcp_clients;
pub mod mock_data;
pub mod permissions;
pub mod sources;
pub mod types;
pub mod vault;

#[cfg(test)]
pub(crate) mod test_fixtures;

pub use error::CoreError;

// Re-exported so dependents share the exact rusqlite version used by the
// public function signatures (all take &rusqlite::Connection).
pub use rusqlite;
