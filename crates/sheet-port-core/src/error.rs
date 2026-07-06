//! Typed broker errors. `Display` prints ONLY the inner message because the
//! wording is part of the observable contract: agents and the desktop UI
//! match on strings like "requires user approval" (see docs/mcp-tools.md and
//! the protocol e2e smoke).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// A read or write was blocked by a permission rule. Mirrors the
    /// TypeScript `PermissionDeniedError` so callers can special-case it.
    PermissionDenied(String),
    /// The caller supplied invalid input (unknown status filter, unknown
    /// confirmation action, ...).
    InvalidInput(String),
    /// A referenced entity (source, table, change, rule) does not exist.
    NotFound(String),
    /// The operation conflicts with current state (already committed,
    /// rejected, awaiting approval, ...).
    Conflict(String),
    /// The operation is not implemented yet (connector stubs, delete MVP).
    Unsupported(String),
    /// SQLite, JSON (de)serialization, keychain, or network/API failure,
    /// with context.
    Storage(String),
}

impl CoreError {
    pub fn message(&self) -> &str {
        match self {
            Self::PermissionDenied(message)
            | Self::InvalidInput(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::Unsupported(message)
            | Self::Storage(message) => message,
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for CoreError {}

/// Wraps a rusqlite error with a human-readable context prefix, matching the
/// "Could not ...: {error}" wording of the previous desktop backend.
pub(crate) fn db_error(context: &str, error: rusqlite::Error) -> CoreError {
    CoreError::Storage(format!("{context}: {error}"))
}

/// Parses a JSON TEXT column, reporting which column was malformed.
pub(crate) fn parse_json<T: serde::de::DeserializeOwned>(
    raw: &str,
    context: &str,
) -> Result<T, CoreError> {
    serde_json::from_str(raw)
        .map_err(|error| CoreError::Storage(format!("{context} is not valid JSON: {error}")))
}
