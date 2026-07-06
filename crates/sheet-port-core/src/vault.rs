//! OS keychain token presence checks. Secrets themselves NEVER leave the
//! keychain; only booleans cross process boundaries.

use crate::types::TokenStatus;

// Shared with the google module: token storage writes the same entry this
// status check reads.
pub(crate) const KEYRING_SERVICE: &str = "sheet-port";
pub(crate) const KEYRING_USER_GOOGLE_SHEETS: &str = "google_sheets";
const KEYRING_USER_PROVIDER: &str = "provider";

pub fn token_status() -> TokenStatus {
    TokenStatus {
        google_sheets: entry_exists(KEYRING_USER_GOOGLE_SHEETS),
        provider: entry_exists(KEYRING_USER_PROVIDER),
    }
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
