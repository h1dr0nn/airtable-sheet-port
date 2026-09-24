//! Keychain constants and token presence status. Secrets themselves NEVER
//! leave the keychain; only booleans cross process boundaries.

use rusqlite::Connection;

use crate::error::CoreError;
use crate::types::TokenStatus;

// Shared with the google module: each bridge credential lives under a user
// name that starts with this prefix ("google_sheets:{accountKey}").
pub(crate) const KEYRING_SERVICE: &str = "sheet-port";
pub(crate) const KEYRING_USER_GOOGLE_SHEETS: &str = "google_sheets";

/// Token presence booleans for the desktop UI. `google_sheets` is true when at
/// least one Google account is connected (a keyed source row exists); the OS
/// keychain cannot be enumerated, so account presence is derived from the
/// `sources` table which the connect/disconnect flow keeps in lockstep with
/// the keychain entries.
pub fn token_status(conn: &Connection) -> Result<TokenStatus, CoreError> {
    Ok(TokenStatus {
        google_sheets: crate::google::has_any_account(conn)?,
    })
}
