//! Keyring-backed Google token storage, keyed per connected account. Each
//! account's JSON credential `{accessToken, refreshToken, expiresAt}` lives
//! under service "sheet-port", user "google_sheets:{accountKey}" (the entry
//! `vault::entry_exists` reports on). The shared, single-OAuth-app client
//! secret stays under user "google_client_secret". Raw tokens never leave the
//! google module.

use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::CoreError;
use crate::vault::{
    KEYRING_SERVICE, KEYRING_USER_GOOGLE_CLIENT_SECRET, KEYRING_USER_GOOGLE_SHEETS,
};

/// Refresh this long before the actual expiry so in-flight requests never
/// race the deadline.
const EXPIRY_MARGIN_MS: i64 = 60_000;

/// Separator between the keyring user prefix and an account key.
pub(crate) const ACCOUNT_KEY_SEPARATOR: char = ':';

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenSet {
    pub access_token: String,
    /// Absent when Google did not issue one (e.g. re-consent without
    /// `prompt=consent`); the previous refresh token is kept in that case.
    pub refresh_token: Option<String>,
    /// ISO-8601 UTC with milliseconds (db::now_iso shape); ISO strings
    /// compare lexicographically.
    pub expires_at: String,
}

impl TokenSet {
    /// True when the access token is past (or within the safety margin of)
    /// its expiry and must be refreshed before use.
    pub(crate) fn is_expired(&self) -> bool {
        self.expires_at <= db::iso_after(EXPIRY_MARGIN_MS)
    }
}

/// Expiry timestamp for a token issued now with the given lifetime.
pub(crate) fn expiry_from_now(expires_in_secs: i64) -> String {
    db::iso_after(expires_in_secs.saturating_mul(1000))
}

/// The keyring user name that stores one account's tokens:
/// "google_sheets:{accountKey}".
pub(crate) fn keyring_user_for(account_key: &str) -> String {
    format!("{KEYRING_USER_GOOGLE_SHEETS}{ACCOUNT_KEY_SEPARATOR}{account_key}")
}

fn entry(account_key: &str) -> Result<keyring::Entry, CoreError> {
    keyring::Entry::new(KEYRING_SERVICE, &keyring_user_for(account_key)).map_err(|error| {
        CoreError::Storage(format!(
            "Could not open the OS keychain entry for Google Sheets: {error}"
        ))
    })
}

pub(crate) fn save(account_key: &str, tokens: &TokenSet) -> Result<(), CoreError> {
    let json = serde_json::to_string(tokens)
        .map_err(|error| CoreError::Storage(format!("Could not encode Google tokens: {error}")))?;
    entry(account_key)?.set_password(&json).map_err(|error| {
        CoreError::Storage(format!(
            "Could not store Google tokens in the OS keychain: {error}"
        ))
    })
}

pub(crate) fn load(account_key: &str) -> Result<Option<TokenSet>, CoreError> {
    match entry(account_key)?.get_password() {
        Ok(raw) => serde_json::from_str(&raw).map(Some).map_err(|error| {
            CoreError::Storage(format!(
                "Stored Google token entry is not valid JSON: {error}"
            ))
        }),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not read Google tokens from the OS keychain: {error}"
        ))),
    }
}

/// Removes one account's stored credential; a missing entry is not an error so
/// disconnect stays idempotent.
pub(crate) fn delete(account_key: &str) -> Result<(), CoreError> {
    match entry(account_key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not delete Google tokens from the OS keychain: {error}"
        ))),
    }
}

fn secret_entry() -> Result<keyring::Entry, CoreError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_GOOGLE_CLIENT_SECRET).map_err(|error| {
        CoreError::Storage(format!(
            "Could not open the OS keychain entry for the Google client secret: {error}"
        ))
    })
}

/// Google requires the (non-confidential) desktop client secret on token
/// exchange; it still lives in the keychain rather than the database. It is
/// shared across all accounts because there is a single OAuth app.
pub(crate) fn save_client_secret(secret: &str) -> Result<(), CoreError> {
    secret_entry()?.set_password(secret).map_err(|error| {
        CoreError::Storage(format!(
            "Could not store the Google client secret in the OS keychain: {error}"
        ))
    })
}

pub(crate) fn load_client_secret() -> Result<Option<String>, CoreError> {
    match secret_entry()?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not read the Google client secret from the OS keychain: {error}"
        ))),
    }
}

pub(crate) fn delete_client_secret() -> Result<(), CoreError> {
    match secret_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not delete the Google client secret from the OS keychain: {error}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Legacy single-account entry (pre multi-account). Used only by the one-time
// startup migration in the parent module. The old scheme stored a single
// credential under user "google_sheets" (no account key suffix).
// ---------------------------------------------------------------------------

fn legacy_entry() -> Result<keyring::Entry, CoreError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_GOOGLE_SHEETS).map_err(|error| {
        CoreError::Storage(format!(
            "Could not open the legacy Google Sheets keychain entry: {error}"
        ))
    })
}

/// Reads the legacy single-account token, if one exists.
pub(crate) fn load_legacy() -> Result<Option<TokenSet>, CoreError> {
    match legacy_entry()?.get_password() {
        Ok(raw) => serde_json::from_str(&raw).map(Some).map_err(|error| {
            CoreError::Storage(format!(
                "Legacy Google token entry is not valid JSON: {error}"
            ))
        }),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not read the legacy Google tokens from the OS keychain: {error}"
        ))),
    }
}

/// Writes the legacy single-account credential. Test-only: production code
/// never creates the legacy entry, but migration tests need to arrange one.
#[cfg(test)]
pub(crate) fn save_legacy_for_test(tokens: &TokenSet) {
    let json = serde_json::to_string(tokens).expect("encode legacy tokens");
    legacy_entry()
        .expect("legacy entry")
        .set_password(&json)
        .expect("write legacy tokens");
}

/// Removes the legacy credential once migrated; idempotent.
pub(crate) fn delete_legacy() -> Result<(), CoreError> {
    match legacy_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not delete the legacy Google tokens from the OS keychain: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::iso_before;

    fn token_set(expires_at: &str) -> TokenSet {
        TokenSet {
            access_token: "access-123".to_string(),
            refresh_token: Some("refresh-456".to_string()),
            expires_at: expires_at.to_string(),
        }
    }

    #[test]
    fn token_set_round_trips_through_camel_case_json() {
        let tokens = token_set("2026-01-01T00:00:00.000Z");
        let json = serde_json::to_string(&tokens).expect("serialize");

        assert!(json.contains("\"accessToken\":\"access-123\""));
        assert!(json.contains("\"refreshToken\":\"refresh-456\""));
        assert!(json.contains("\"expiresAt\":\"2026-01-01T00:00:00.000Z\""));

        let parsed: TokenSet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, tokens);
    }

    #[test]
    fn token_set_round_trips_without_refresh_token() {
        let tokens = TokenSet {
            refresh_token: None,
            ..token_set("2026-01-01T00:00:00.000Z")
        };
        let json = serde_json::to_string(&tokens).expect("serialize");
        let parsed: TokenSet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.refresh_token, None);
        assert_eq!(parsed, tokens);
    }

    #[test]
    fn keyring_user_for_appends_the_account_key() {
        assert_eq!(
            keyring_user_for("alice_example_com"),
            "google_sheets:alice_example_com"
        );
        assert_eq!(keyring_user_for("default"), "google_sheets:default");
    }

    #[test]
    fn expiry_check_applies_the_safety_margin() {
        // Far in the future: fresh.
        let fresh = TokenSet {
            expires_at: expiry_from_now(3600),
            ..token_set("")
        };
        assert!(!fresh.is_expired());

        // Already past: expired.
        let past = TokenSet {
            expires_at: iso_before(1_000),
            ..token_set("")
        };
        assert!(past.is_expired());

        // Inside the 60s margin: treated as expired so refresh happens early.
        let almost = TokenSet {
            expires_at: expiry_from_now(30),
            ..token_set("")
        };
        assert!(almost.is_expired());
    }

    #[test]
    fn expiry_from_now_produces_iso_shape_in_the_future() {
        let value = expiry_from_now(3600);
        assert_eq!(value.len(), 24);
        assert!(value.ends_with('Z'));
        assert!(value > crate::db::now_iso());
    }
}
