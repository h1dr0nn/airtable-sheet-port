//! Google Sheets account linking through Apps Script bridges. Each connected
//! account is one bridge (a web app deployed by the user) that hands out
//! short-lived Google access tokens; its URL, secret and cached token live in
//! the OS keychain. Raw secrets and tokens NEVER leave this module; the
//! connector obtains access tokens via the crate-private [`access_token`], and
//! other processes only ever see the boolean from `vault::token_status`. Audit
//! events are recorded by callers.

mod bridge;
mod tokens;

use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

use crate::connectors::parse_spreadsheet_id;
use crate::db;
use crate::error::{db_error, CoreError};
use crate::permissions;
use crate::sources;
use crate::types::{SavePermissionRule, SourceKind};

use tokens::{BridgeCredential, StoredCredential};

/// The source-id prefix every connected Google account shares. A concrete
/// account's row id is "google-sheets:{accountKey}".
pub const GOOGLE_SOURCE_ID: &str = "google-sheets";

/// Separator between the source-id prefix and an account key. Matches the
/// keyring user separator so ids and keychain entries stay parallel.
const SOURCE_ID_SEPARATOR: char = ':';

/// Account key used when an email carries no alphanumerics at all.
const DEFAULT_ACCOUNT_KEY: &str = "default";

/// Meta key prefix caching which account opened a spreadsheet:
/// "google_route:{spreadsheetId}" -> source id.
const ROUTE_META_PREFIX: &str = "google_route:";

/// Endpoint probed to check whether an account can open a spreadsheet.
const SHEETS_API_BASE: &str = "https://sheets.googleapis.com/v4/spreadsheets";

/// A connected Google account as surfaced to the desktop UI. The secret and
/// tokens never appear here. `deployment_id` and `bridge_url` are empty when
/// the stored credential is missing or from the removed OAuth flow, so the UI
/// can offer "re-add".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccount {
    /// "google-sheets:{accountKey}" - the source row id.
    pub source_id: String,
    /// The Google account the bridge executes as.
    pub email: String,
    /// The Apps Script deployment id.
    pub deployment_id: String,
    /// The canonical web app URL of the bridge.
    pub bridge_url: String,
}

/// Contract wording when Google rejects a bridge-issued token (docs/mcp-tools.md).
pub(crate) const TOKEN_EXPIRED_MESSAGE: &str =
    "Google rejected the bridge token; test the bridge in the Airtable - Sheet Port desktop app";

/// Contract wording when Sheets is used before any bridge is added.
pub(crate) const NOT_CONNECTED_MESSAGE: &str =
    "Google Sheets is not connected. Add an Apps Script bridge in the Airtable - Sheet Port desktop app first";

/// Wording when an account's keychain entry is not a bridge credential (for
/// example one left behind by the removed OAuth sign-in).
const STALE_CREDENTIAL_MESSAGE: &str =
    "This Google account has no Apps Script bridge. Remove it and add a bridge in the Airtable - Sheet Port desktop app";

/// Wording when a bridge now signs in as a different Google account than the
/// one it was added for.
const EMAIL_CHANGED_MESSAGE: &str =
    "The bridge now signs in as a different Google account. Remove it and add it again in the Airtable - Sheet Port desktop app";

/// Request timeout for every Google and bridge call.
const HTTP_TIMEOUT_SECS: u64 = 30;
/// Longest raw body slice quoted back in error messages.
const ERROR_SNIPPET_MAX_CHARS: usize = 200;

/// Derives a stable, keychain-safe account key from an email. Lowercases and
/// replaces every character outside `[a-z0-9]` with `_` so the key is safe as a
/// keyring user suffix and a source-id suffix. Empty results fall back to the
/// default key so a malformed email never yields an empty key.
pub(crate) fn account_key_from_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    // An email with no alphanumerics carries no distinguishing key, so fall
    // back to the default rather than an all-underscore (or empty) suffix.
    if !normalized
        .chars()
        .any(|character| character.is_ascii_alphanumeric())
    {
        return DEFAULT_ACCOUNT_KEY.to_string();
    }
    normalized
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

/// "google-sheets:{accountKey}" - the source row id for one account.
pub(crate) fn source_id_for(account_key: &str) -> String {
    format!("{GOOGLE_SOURCE_ID}{SOURCE_ID_SEPARATOR}{account_key}")
}

/// The source row id a given email resolves to, mirroring what [`add_bridge`]
/// writes. Public so command wrappers can audit the exact account scope.
pub fn source_id_for_email(email: &str) -> String {
    source_id_for(&account_key_from_email(email))
}

/// Extracts the account key from a "google-sheets:{accountKey}" source id.
/// Returns None for the bare prefix (no key) or any non-Google id.
pub(crate) fn account_key_from_source_id(source_id: &str) -> Option<&str> {
    source_id
        .strip_prefix(GOOGLE_SOURCE_ID)?
        .strip_prefix(SOURCE_ID_SEPARATOR)
        .filter(|key| !key.is_empty())
}

/// Validates an Apps Script web app URL and returns its deployment id. Accepts
/// `https://script.google.com/macros/s/{id}/exec` and the Workspace form
/// `https://script.google.com/a/macros/{domain}/s/{id}/exec`.
pub fn parse_deployment_id(url: &str) -> Result<String, CoreError> {
    Ok(bridge::parse_bridge_url(url)?.deployment_id)
}

/// Adds (or replaces) a bridge: validates the URL and secret, fetches a token
/// to learn the signed-in email, stores the credential under that account's
/// keychain entry, upserts its "google-sheets:{accountKey}" source row, and
/// grants a source-wide read/write/delete rule when the source has none yet.
/// Adding the same email again replaces the stored credential. Blocks on the
/// network; run it off any async runtime.
pub fn add_bridge(conn: &Connection, url: &str, secret: &str) -> Result<GoogleAccount, CoreError> {
    let location = bridge::parse_bridge_url(url)?;
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(CoreError::InvalidInput(
            "The bridge secret must not be empty".to_string(),
        ));
    }
    let token = bridge::fetch_token(&location.url, secret)?;
    let credential = BridgeCredential {
        bridge_url: location.url,
        secret: secret.to_string(),
        deployment_id: location.deployment_id,
        access_token: token.access_token,
        expires_at: tokens::expiry_from_now(token.expires_in_secs),
    };
    let account_key = account_key_from_email(&token.email);
    tokens::save(&account_key, &credential)?;
    let source_id = source_id_for(&account_key);
    upsert_account_source(conn, &source_id, &token.email)?;
    ensure_default_rule(conn, &source_id)?;
    Ok(account_from(source_id, token.email, &credential))
}

/// Removes ONE bridge: its keychain credential, its source row, and any cached
/// spreadsheet routes pointing at it. Idempotent: removing an already-removed
/// bridge is not an error. Rejects a source id that is not a keyed Google
/// account so callers cannot delete arbitrary rows.
pub fn remove_bridge(conn: &Connection, source_id: &str) -> Result<(), CoreError> {
    let account_key = require_account_key(source_id)?;
    tokens::delete(account_key)?;
    sources::delete(conn, source_id)?;
    conn.execute(
        "DELETE FROM meta WHERE key LIKE ?1 AND value = ?2",
        params![format!("{ROUTE_META_PREFIX}%"), source_id],
    )
    .map_err(|error| db_error("Could not clear cached spreadsheet routes", error))?;
    // Workbench entries opened through this account can no longer be read.
    conn.execute(
        "DELETE FROM workbench_items WHERE source_id = ?1",
        params![source_id],
    )
    .map_err(|error| db_error("Could not remove the account's workbench items", error))?;
    Ok(())
}

/// Fetches a fresh token from the account's bridge (ignoring the cache),
/// stores it, and marks the source connected. Fails when the bridge is
/// unreachable, rejects the secret, or now signs in as a different account.
pub fn test_bridge(conn: &Connection, source_id: &str) -> Result<GoogleAccount, CoreError> {
    let account_key = require_account_key(source_id)?;
    let credential = match tokens::load(account_key)? {
        StoredCredential::Bridge(credential) => credential,
        StoredCredential::Missing => {
            return Err(CoreError::NotFound(format!(
                "No bridge is stored for {source_id}; add it again in the Airtable - Sheet Port desktop app"
            )))
        }
        StoredCredential::Unreadable => {
            return Err(CoreError::PermissionDenied(
                STALE_CREDENTIAL_MESSAGE.to_string(),
            ))
        }
    };
    let (credential, email) = refetch(account_key, credential)?;
    upsert_account_source(conn, source_id, &email)?;
    Ok(account_from(source_id.to_string(), email, &credential))
}

/// Every connected Google account, ordered by source id: the keyed Google
/// source rows joined with their keychain credential. An account whose
/// credential is missing or unreadable is still listed with an empty
/// deployment id and bridge URL.
pub fn list_accounts(conn: &Connection) -> Result<Vec<GoogleAccount>, CoreError> {
    let mut accounts = Vec::new();
    for source in sources::list(conn)? {
        if source.kind != SourceKind::GoogleSheets {
            continue;
        }
        let Some(account_key) = account_key_from_source_id(&source.id) else {
            continue;
        };
        let (deployment_id, bridge_url) = match tokens::load(account_key) {
            Ok(StoredCredential::Bridge(credential)) => {
                (credential.deployment_id, credential.bridge_url)
            }
            _ => (String::new(), String::new()),
        };
        accounts.push(GoogleAccount {
            email: email_from_source_name(&source.name),
            source_id: source.id,
            deployment_id,
            bridge_url,
        });
    }
    Ok(accounts)
}

/// True when at least one Google account is connected.
pub(crate) fn has_any_account(conn: &Connection) -> Result<bool, CoreError> {
    Ok(!google_source_ids(conn)?.is_empty())
}

/// Picks the source for a tool call. An explicit id is returned unchanged (any
/// source kind). Without one, only Google accounts are considered: none is an
/// error, a single account is used directly, and with several the spreadsheet
/// behind `table_id` is routed to the first account (in list order) that can
/// open it, caching the answer in meta. Several accounts and no table id pick
/// the first account.
pub fn resolve_source(
    conn: &Connection,
    source_id: Option<&str>,
    table_id: Option<&str>,
) -> Result<String, CoreError> {
    if let Some(source_id) = source_id {
        return Ok(source_id.to_string());
    }
    let accounts = google_source_ids(conn)?;
    let Some(first) = accounts.first() else {
        return Err(CoreError::PermissionDenied(
            NOT_CONNECTED_MESSAGE.to_string(),
        ));
    };
    if accounts.len() == 1 {
        return Ok(first.clone());
    }
    let Some(table_id) = table_id else {
        return Ok(first.clone());
    };

    let spreadsheet_id = parse_spreadsheet_id(table_id)?;
    let route_key = format!("{ROUTE_META_PREFIX}{spreadsheet_id}");
    if let Some(cached) = db::get_meta(conn, &route_key)? {
        if accounts.contains(&cached) {
            return Ok(cached);
        }
    }
    let probe_url = format!("{SHEETS_API_BASE}/{spreadsheet_id}?fields=spreadsheetId");
    for candidate in &accounts {
        let opened = access_token(conn, candidate)
            .and_then(|token| get_json(&token, &probe_url))
            .is_ok();
        if opened {
            db::set_meta(conn, &route_key, candidate)?;
            return Ok(candidate.clone());
        }
    }
    Err(CoreError::NotFound(format!(
        "No connected bridge can open spreadsheet {spreadsheet_id}"
    )))
}

/// A currently-valid access token for the account behind `source_id`, served
/// from the keychain cache and refetched from the bridge once it is within the
/// 60s expiry margin. Crate-private on purpose: raw tokens never leave core.
pub(crate) fn access_token(_conn: &Connection, source_id: &str) -> Result<String, CoreError> {
    let account_key = account_key_from_source_id(source_id)
        .ok_or_else(|| CoreError::PermissionDenied(NOT_CONNECTED_MESSAGE.to_string()))?;
    let credential = match tokens::load(account_key)? {
        StoredCredential::Bridge(credential) => credential,
        StoredCredential::Missing => {
            return Err(CoreError::PermissionDenied(
                NOT_CONNECTED_MESSAGE.to_string(),
            ))
        }
        StoredCredential::Unreadable => {
            return Err(CoreError::PermissionDenied(
                STALE_CREDENTIAL_MESSAGE.to_string(),
            ))
        }
    };
    if !credential.is_expired() {
        return Ok(credential.access_token);
    }
    let (credential, _) = refetch(account_key, credential)?;
    Ok(credential.access_token)
}

/// Asks the bridge for a new token, checks it still belongs to the same
/// account, and stores it. Returns the updated credential and the email.
fn refetch(
    account_key: &str,
    credential: BridgeCredential,
) -> Result<(BridgeCredential, String), CoreError> {
    let token = bridge::fetch_token(&credential.bridge_url, &credential.secret)?;
    if account_key_from_email(&token.email) != account_key {
        return Err(CoreError::PermissionDenied(
            EMAIL_CHANGED_MESSAGE.to_string(),
        ));
    }
    let updated = BridgeCredential {
        access_token: token.access_token,
        expires_at: tokens::expiry_from_now(token.expires_in_secs),
        ..credential
    };
    tokens::save(account_key, &updated)?;
    Ok((updated, token.email))
}

/// The account key of a keyed Google source id, or InvalidInput.
fn require_account_key(source_id: &str) -> Result<&str, CoreError> {
    account_key_from_source_id(source_id).ok_or_else(|| {
        CoreError::InvalidInput(format!(
            "'{source_id}' is not a Google Sheets account source id"
        ))
    })
}

/// Ids of the keyed Google source rows, ordered by id. No keychain access.
fn google_source_ids(conn: &Connection) -> Result<Vec<String>, CoreError> {
    Ok(sources::list(conn)?
        .into_iter()
        .filter(|source| {
            source.kind == SourceKind::GoogleSheets
                && account_key_from_source_id(&source.id).is_some()
        })
        .map(|source| source.id)
        .collect())
}

/// Upserts the "Google Sheets ({email})" source row as connected.
fn upsert_account_source(conn: &Connection, source_id: &str, email: &str) -> Result<(), CoreError> {
    sources::upsert(
        conn,
        source_id,
        SourceKind::GoogleSheets,
        &format!("Google Sheets ({email})"),
        sources::SOURCE_STATUS_CONNECTED,
    )
}

/// Grants a source-wide read/write/delete rule (no confirmations) when the
/// source has no source-wide rule yet; an existing rule is left untouched.
fn ensure_default_rule(conn: &Connection, source_id: &str) -> Result<(), CoreError> {
    // With no table id, find_rule only ever returns the source-wide rule.
    if permissions::find_rule(conn, source_id, None)?.is_some() {
        return Ok(());
    }
    permissions::save_rule(
        conn,
        &SavePermissionRule {
            id: None,
            source_id: source_id.to_string(),
            table_id: None,
            read: true,
            write: true,
            delete_records: true,
        },
    )?;
    Ok(())
}

/// Builds the UI view of an account from its stored credential.
fn account_from(source_id: String, email: String, credential: &BridgeCredential) -> GoogleAccount {
    GoogleAccount {
        source_id,
        email,
        deployment_id: credential.deployment_id.clone(),
        bridge_url: credential.bridge_url.clone(),
    }
}

/// "Google Sheets (user@example.com)" -> "user@example.com"; any other shape
/// falls back to the raw source name so the UI still shows something.
fn email_from_source_name(name: &str) -> String {
    name.rfind('(')
        .and_then(|start| name[start + 1..].strip_suffix(')'))
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

// ---------------------------------------------------------------------------
// Shared HTTP plumbing for Google endpoints (also used by the connector)
// ---------------------------------------------------------------------------

/// The blocking HTTP client shared by the bridge and the Sheets connector,
/// with the default redirect policy (needed to follow the bridge's 302).
pub(crate) fn http_client() -> Result<reqwest::blocking::Client, CoreError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|error| CoreError::Storage(format!("Could not build the HTTP client: {error}")))
}

/// Authenticated GET returning the parsed JSON body.
pub(crate) fn get_json(token: &str, url: &str) -> Result<Value, CoreError> {
    let response = http_client()?
        .get(url)
        .bearer_auth(token)
        .send()
        .map_err(transport_error)?;
    parse_api_response(response)
}

/// Authenticated JSON POST returning the parsed JSON body.
pub(crate) fn post_json(token: &str, url: &str, body: &Value) -> Result<Value, CoreError> {
    let response = http_client()?
        .post(url)
        .bearer_auth(token)
        .json(body)
        .send()
        .map_err(transport_error)?;
    parse_api_response(response)
}

/// Wraps a transport failure (DNS, TLS, timeout) in contract wording.
fn transport_error(error: reqwest::Error) -> CoreError {
    CoreError::Storage(format!("Could not reach Google: {error}"))
}

/// Maps a Google API response to JSON, or to a contract error on failure.
fn parse_api_response(response: reqwest::blocking::Response) -> Result<Value, CoreError> {
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(api_error(status.as_u16(), &body));
    }
    if body.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&body).map_err(|error| {
        CoreError::Storage(format!("Google API response was not valid JSON: {error}"))
    })
}

/// Maps Google HTTP failures onto contract errors; 401 always reads as the
/// test-the-bridge instruction agents and the desktop UI display verbatim.
pub(crate) fn api_error(status: u16, body: &str) -> CoreError {
    let snippet = error_snippet(body);
    match status {
        401 => CoreError::PermissionDenied(TOKEN_EXPIRED_MESSAGE.to_string()),
        403 => CoreError::PermissionDenied(format!("Google API access was denied: {snippet}")),
        404 => CoreError::NotFound(format!("Google Sheets resource was not found: {snippet}")),
        _ => CoreError::Storage(format!("Google API error {status}: {snippet}")),
    }
}

/// Prefers the message from Google's standard error envelopes; falls back to
/// a bounded slice of the raw body.
pub(crate) fn error_snippet(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = value["error"]["message"].as_str() {
            return message.to_string();
        }
        if let Some(description) = value["error_description"].as_str() {
            return description.to_string();
        }
        if let Some(code) = value["error"].as_str() {
            return code.to_string();
        }
    }
    body.chars().take(ERROR_SNIPPET_MAX_CHARS).collect()
}

#[cfg(test)]
#[path = "google_tests.rs"]
mod multi_account_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_maps_401_to_the_test_bridge_contract_message() {
        let error = api_error(401, "{\"error\":{\"message\":\"Invalid Credentials\"}}");
        assert!(matches!(error, CoreError::PermissionDenied(_)));
        assert_eq!(
            error.to_string(),
            "Google rejected the bridge token; test the bridge in the Airtable - Sheet Port desktop app"
        );
    }

    #[test]
    fn api_error_maps_403_404_and_others() {
        let denied = api_error(403, "{\"error\":{\"message\":\"Rate limit\"}}");
        assert!(matches!(denied, CoreError::PermissionDenied(_)));
        assert_eq!(
            denied.to_string(),
            "Google API access was denied: Rate limit"
        );

        let missing = api_error(
            404,
            "{\"error\":{\"message\":\"Requested entity was not found.\"}}",
        );
        assert!(matches!(missing, CoreError::NotFound(_)));

        let server = api_error(500, "oops");
        assert!(matches!(server, CoreError::Storage(_)));
        assert_eq!(server.to_string(), "Google API error 500: oops");
    }

    #[test]
    fn error_snippet_prefers_structured_messages_and_bounds_raw_bodies() {
        assert_eq!(
            error_snippet("{\"error\":{\"message\":\"Quota exceeded\"}}"),
            "Quota exceeded"
        );
        assert_eq!(
            error_snippet("{\"error\":\"invalid_grant\",\"error_description\":\"Bad token\"}"),
            "Bad token"
        );
        assert_eq!(
            error_snippet("{\"error\":\"invalid_grant\"}"),
            "invalid_grant"
        );

        let long_body = "x".repeat(500);
        assert_eq!(error_snippet(&long_body).len(), 200);
    }

    #[test]
    fn parse_deployment_id_accepts_both_url_forms_and_rejects_others() {
        assert_eq!(
            parse_deployment_id("https://script.google.com/macros/s/AKfy_1-2/exec").expect("ok"),
            "AKfy_1-2"
        );
        assert_eq!(
            parse_deployment_id("https://script.google.com/a/macros/corp.io/s/AKfy/exec")
                .expect("workspace"),
            "AKfy"
        );
        for input in [
            "https://docs.google.com/macros/s/AKfy/exec",
            "http://script.google.com/macros/s/AKfy/exec",
            "https://script.google.com/macros/s/AKfy",
            "https://script.google.com/macros/s/AK$fy/exec",
        ] {
            assert!(
                matches!(parse_deployment_id(input), Err(CoreError::InvalidInput(_))),
                "{input}"
            );
        }
    }

    #[test]
    fn add_bridge_validates_before_any_network_call() {
        let conn = crate::db::test_support::open_temp_db();
        let bad_url = add_bridge(&conn, "https://example.com/x", "secret").expect_err("url");
        assert!(matches!(bad_url, CoreError::InvalidInput(_)));

        let empty_secret = add_bridge(&conn, "https://script.google.com/macros/s/AKfy/exec", "   ")
            .expect_err("secret");
        assert_eq!(
            empty_secret.to_string(),
            "The bridge secret must not be empty"
        );
    }
}
