//! Keyring-backed Google bridge credential storage, keyed per connected
//! account. Each account's JSON credential
//! `{bridgeUrl, secret, deploymentId, accessToken, expiresAt}` lives under
//! service "sheet-port", user "google_sheets:{accountKey}" (the entry
//! `vault::entry_exists` reports on). The bridge secret and the cached access
//! token never leave the google module.

use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::CoreError;
use crate::vault::{KEYRING_SERVICE, KEYRING_USER_GOOGLE_SHEETS};

/// Refetch this long before the actual expiry so in-flight requests never
/// race the deadline.
const EXPIRY_MARGIN_MS: i64 = 60_000;

/// Separator between the keyring user prefix and an account key.
pub(crate) const ACCOUNT_KEY_SEPARATOR: char = ':';

/// Everything needed to reach one account's Apps Script bridge, plus the most
/// recent access token it issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BridgeCredential {
    /// Canonical web app URL: "https://script.google.com/.../s/{id}/exec".
    pub bridge_url: String,
    /// Shared secret the bridge checks before issuing a token.
    pub secret: String,
    /// The Apps Script deployment id parsed from `bridge_url`.
    pub deployment_id: String,
    /// Cached Google OAuth access token issued by the bridge.
    pub access_token: String,
    /// ISO-8601 UTC with milliseconds (db::now_iso shape); ISO strings
    /// compare lexicographically.
    pub expires_at: String,
}

impl BridgeCredential {
    /// True when the cached access token is past (or within the safety margin
    /// of) its expiry and must be refetched from the bridge before use.
    pub(crate) fn is_expired(&self) -> bool {
        self.expires_at <= db::iso_after(EXPIRY_MARGIN_MS)
    }
}

/// Expiry timestamp for a token issued now with the given lifetime.
pub(crate) fn expiry_from_now(expires_in_secs: i64) -> String {
    db::iso_after(expires_in_secs.saturating_mul(1000))
}

/// The keyring user name that stores one account's credential:
/// "google_sheets:{accountKey}".
pub(crate) fn keyring_user_for(account_key: &str) -> String {
    format!("{KEYRING_USER_GOOGLE_SHEETS}{ACCOUNT_KEY_SEPARATOR}{account_key}")
}

/// Opens the keychain entry for one account.
fn entry(account_key: &str) -> Result<keyring::Entry, CoreError> {
    keyring::Entry::new(KEYRING_SERVICE, &keyring_user_for(account_key)).map_err(|error| {
        CoreError::Storage(format!(
            "Could not open the OS keychain entry for Google Sheets: {error}"
        ))
    })
}

/// Stores (or replaces) one account's bridge credential.
pub(crate) fn save(account_key: &str, credential: &BridgeCredential) -> Result<(), CoreError> {
    let json = serde_json::to_string(credential).map_err(|error| {
        CoreError::Storage(format!(
            "Could not encode the Google bridge credential: {error}"
        ))
    })?;
    entry(account_key)?.set_password(&json).map_err(|error| {
        CoreError::Storage(format!(
            "Could not store the Google bridge credential in the OS keychain: {error}"
        ))
    })
}

/// Outcome of reading one account's keychain entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoredCredential {
    /// No entry exists for the account.
    Missing,
    /// An entry exists but is not a bridge credential (for example a token set
    /// left behind by the removed OAuth flow).
    Unreadable,
    /// A valid bridge credential.
    Bridge(BridgeCredential),
}

/// Reads one account's keychain entry, classifying it without failing on an
/// entry in an older shape. Only keychain access failures are errors.
pub(crate) fn load(account_key: &str) -> Result<StoredCredential, CoreError> {
    match entry(account_key)?.get_password() {
        Ok(raw) => Ok(parse_credential(&raw)),
        Err(keyring::Error::NoEntry) => Ok(StoredCredential::Missing),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not read the Google bridge credential from the OS keychain: {error}"
        ))),
    }
}

/// Classifies a raw keychain value as a bridge credential or an unreadable one.
fn parse_credential(raw: &str) -> StoredCredential {
    serde_json::from_str::<BridgeCredential>(raw)
        .map(StoredCredential::Bridge)
        .unwrap_or(StoredCredential::Unreadable)
}

/// Removes one account's stored credential; a missing entry is not an error so
/// removal stays idempotent.
pub(crate) fn delete(account_key: &str) -> Result<(), CoreError> {
    match entry(account_key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not delete the Google bridge credential from the OS keychain: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::iso_before;

    /// A credential with fixed values and the given expiry.
    fn credential(expires_at: &str) -> BridgeCredential {
        BridgeCredential {
            bridge_url: "https://script.google.com/macros/s/AKfy_123/exec".to_string(),
            secret: "s3cret".to_string(),
            deployment_id: "AKfy_123".to_string(),
            access_token: "ya29.token".to_string(),
            expires_at: expires_at.to_string(),
        }
    }

    #[test]
    fn credential_round_trips_through_camel_case_json() {
        let value = credential("2026-01-01T00:00:00.000Z");
        let json = serde_json::to_string(&value).expect("serialize");

        assert!(json.contains("\"bridgeUrl\":\"https://script.google.com/macros/s/AKfy_123/exec\""));
        assert!(json.contains("\"secret\":\"s3cret\""));
        assert!(json.contains("\"deploymentId\":\"AKfy_123\""));
        assert!(json.contains("\"accessToken\":\"ya29.token\""));
        assert!(json.contains("\"expiresAt\":\"2026-01-01T00:00:00.000Z\""));

        assert_eq!(parse_credential(&json), StoredCredential::Bridge(value));
    }

    #[test]
    fn an_old_oauth_token_set_reads_as_unreadable() {
        let legacy =
            r#"{"accessToken":"a","refreshToken":"r","expiresAt":"2026-01-01T00:00:00.000Z"}"#;
        assert_eq!(parse_credential(legacy), StoredCredential::Unreadable);
        assert_eq!(parse_credential("not json"), StoredCredential::Unreadable);
    }

    #[test]
    fn keyring_user_for_appends_the_account_key() {
        assert_eq!(
            keyring_user_for("alice_example_com"),
            "google_sheets:alice_example_com"
        );
    }

    #[test]
    fn expiry_check_applies_the_safety_margin() {
        assert!(!credential(&expiry_from_now(3600)).is_expired());
        assert!(credential(&iso_before(1_000)).is_expired());
        // Inside the 60s margin: treated as expired so the refetch happens early.
        assert!(credential(&expiry_from_now(30)).is_expired());
    }

    #[test]
    fn expiry_from_now_produces_iso_shape_in_the_future() {
        let value = expiry_from_now(3600);
        assert_eq!(value.len(), 24);
        assert!(value.ends_with('Z'));
        assert!(value > crate::db::now_iso());
    }
}
