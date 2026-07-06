//! OS keychain token presence checks. Secrets themselves NEVER leave the
//! keychain; only booleans cross process boundaries.

use rusqlite::Connection;

use crate::error::CoreError;
use crate::types::TokenStatus;

// Shared with the google module: token storage writes entries whose user names
// start with this prefix ("google_sheets:{accountKey}"). The bare prefix is
// also the legacy single-account user the migration reads and clears.
pub(crate) const KEYRING_SERVICE: &str = "sheet-port";
pub(crate) const KEYRING_USER_GOOGLE_SHEETS: &str = "google_sheets";
pub(crate) const KEYRING_USER_GOOGLE_CLIENT_SECRET: &str = "google_client_secret";
const KEYRING_USER_PROVIDER: &str = "provider";

/// Token presence booleans for the desktop UI. `google_sheets` is true when at
/// least one Google account is connected (a keyed source row exists); the OS
/// keychain cannot be enumerated, so account presence is derived from the
/// `sources` table which the connect/disconnect flow keeps in lockstep with
/// the keychain entries.
pub fn token_status(conn: &Connection) -> Result<TokenStatus, CoreError> {
    Ok(TokenStatus {
        google_sheets: crate::google::has_any_account(conn)?,
        provider: entry_exists(KEYRING_USER_PROVIDER),
    })
}

/// True only when a credential exists. Unexpected keychain errors are logged
/// to stderr and reported as "absent" so a broken keychain never blocks the
/// UI.
pub(crate) fn entry_exists(user: &str) -> bool {
    let entry = match keyring::Entry::new(KEYRING_SERVICE, user) {
        Ok(entry) => entry,
        Err(error) => {
            eprintln!("[sheet-port] keyring entry '{user}' unavailable: {error}");
            return false;
        }
    };
    match entry.get_password() {
        Ok(_) => true,
        Err(keyring::Error::NoEntry) => false,
        Err(error) => {
            eprintln!("[sheet-port] keyring read for '{user}' failed: {error}");
            false
        }
    }
}
