//! Google Sheets account linking: OAuth 2.0 PKCE for desktop apps, keyring
//! token storage, and the connect/disconnect lifecycle. Raw tokens NEVER
//! leave this module; the connector obtains short-lived access tokens via
//! the crate-private [`access_token`], and other processes only ever see the
//! boolean from `vault::token_status`. Audit events are recorded by callers.

mod oauth;
mod tokens;

use std::time::Duration;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::constants::META_GOOGLE_CLIENT_ID;
use crate::db;
use crate::error::CoreError;
use crate::sources;
use crate::types::SourceKind;

pub(crate) use tokens::TokenSet;

/// The source-id prefix every connected Google account shares. A concrete
/// account's row id is "google-sheets:{accountKey}"; the bare prefix is the
/// legacy single-account id the migration rewrites.
pub const GOOGLE_SOURCE_ID: &str = "google-sheets";

/// Separator between the source-id prefix and an account key. Matches the
/// keyring user separator so ids and keychain entries stay parallel.
const SOURCE_ID_SEPARATOR: char = ':';

/// Account key used when migrating a legacy connection whose email is unknown.
const DEFAULT_ACCOUNT_KEY: &str = "default";

/// A connected Google account as surfaced to the desktop UI. `email` is parsed
/// from the source name; the raw tokens never appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccount {
    /// "google-sheets:{accountKey}" - the source row id.
    pub source_id: String,
    pub email: String,
}

/// Contract wording for expired/revoked credentials (docs/mcp-tools.md).
pub(crate) const TOKEN_EXPIRED_MESSAGE: &str =
    "Google token expired or revoked, reconnect in the desktop app";

/// Contract wording when Sheets is used before an account is linked.
pub(crate) const NOT_CONNECTED_MESSAGE: &str =
    "Google Sheets is not connected. Connect it in the Airtable - Sheet Port desktop app first";

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

/// The source row id a given email resolves to, mirroring what [`connect`]
/// writes. Public so command wrappers can audit the exact account scope.
pub fn source_id_for_email(email: &str) -> String {
    source_id_for(&account_key_from_email(email))
}

/// Extracts the account key from a "google-sheets:{accountKey}" source id.
/// Returns None for the bare legacy id (no key) or any non-Google id.
pub(crate) fn account_key_from_source_id(source_id: &str) -> Option<&str> {
    source_id
        .strip_prefix(GOOGLE_SOURCE_ID)?
        .strip_prefix(SOURCE_ID_SEPARATOR)
        .filter(|key| !key.is_empty())
}

/// Runs the full interactive OAuth flow for a NEW account: opens the system
/// browser on the consent page, waits on the loopback redirect, exchanges the
/// code (PKCE), derives the account key from the signed-in email, stores that
/// account's tokens, and upserts its "google-sheets:{accountKey}" source row.
/// Connecting the same email again updates that account in place. Returns the
/// connected account email.
///
/// Blocks the calling thread until the browser flow finishes or times out;
/// run it off any async runtime (e.g. `tokio::task::spawn_blocking`).
pub fn connect(conn: &Connection, client_id: &str) -> Result<String, CoreError> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err(CoreError::InvalidInput(
            "Google client ID must not be empty".to_string(),
        ));
    }

    let client_secret = tokens::load_client_secret()?;
    let flow = oauth::AuthFlow::start(client_id, client_secret.as_deref())?;
    open_in_browser(flow.consent_url())?;
    let (token_set, responder) = flow.wait_for_tokens()?;

    // The browser tab is still waiting on `responder`: only report success
    // there once the account is fully connected, so the page never lies.
    let finish = || -> Result<String, CoreError> {
        let email = oauth::fetch_user_email(&token_set.access_token)?;
        let account_key = account_key_from_email(&email);
        tokens::save(&account_key, &token_set)?;
        // The refresh flow needs the client id later even if the desktop app
        // never stored it explicitly.
        db::set_meta(conn, META_GOOGLE_CLIENT_ID, client_id)?;
        sources::upsert(
            conn,
            &source_id_for(&account_key),
            SourceKind::GoogleSheets,
            &format!("Google Sheets ({email})"),
            sources::SOURCE_STATUS_CONNECTED,
        )?;
        Ok(email)
    };

    match finish() {
        Ok(email) => {
            responder.succeed();
            Ok(email)
        }
        Err(error) => {
            responder.fail(&error.to_string());
            Err(error)
        }
    }
}

/// Removes ONE account: its keychain credential and its
/// "google-sheets:{accountKey}" source row. Idempotent: disconnecting an
/// already-removed account is not an error. Rejects a source id that is not a
/// keyed Google account so callers cannot delete arbitrary rows.
pub fn disconnect(conn: &Connection, source_id: &str) -> Result<(), CoreError> {
    let account_key = account_key_from_source_id(source_id).ok_or_else(|| {
        CoreError::InvalidInput(format!(
            "'{source_id}' is not a Google Sheets account source id"
        ))
    })?;
    tokens::delete(account_key)?;
    sources::delete(conn, source_id)?;
    Ok(())
}

/// Every connected Google account (source id + email), ordered by source id.
/// No keychain enumeration: accounts are the keyed Google source rows, which
/// the connect/disconnect flow keeps in lockstep with the keychain entries.
pub fn list_accounts(conn: &Connection) -> Result<Vec<GoogleAccount>, CoreError> {
    Ok(sources::list(conn)?
        .into_iter()
        .filter(|source| {
            source.kind == SourceKind::GoogleSheets
                && account_key_from_source_id(&source.id).is_some()
        })
        .map(|source| GoogleAccount {
            email: email_from_source_name(&source.name),
            source_id: source.id,
        })
        .collect())
}

/// True when at least one Google account is connected.
pub(crate) fn has_any_account(conn: &Connection) -> Result<bool, CoreError> {
    Ok(!list_accounts(conn)?.is_empty())
}

/// "Google Sheets (user@example.com)" -> "user@example.com"; any other shape
/// falls back to the raw source name so the UI still shows something.
fn email_from_source_name(name: &str) -> String {
    name.rfind('(')
        .and_then(|start| name[start + 1..].strip_suffix(')'))
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

/// A currently-valid access token for the account behind `source_id`,
/// refreshing through that account's stored refresh token when expired.
/// Crate-private on purpose: raw tokens never leave core.
pub(crate) fn access_token(conn: &Connection, source_id: &str) -> Result<String, CoreError> {
    let account_key = account_key_from_source_id(source_id)
        .ok_or_else(|| CoreError::PermissionDenied(NOT_CONNECTED_MESSAGE.to_string()))?;
    let Some(stored) = tokens::load(account_key)? else {
        return Err(CoreError::PermissionDenied(
            NOT_CONNECTED_MESSAGE.to_string(),
        ));
    };
    if !stored.is_expired() {
        return Ok(stored.access_token);
    }
    let Some(refresh_token) = stored.refresh_token.clone() else {
        return Err(CoreError::PermissionDenied(
            TOKEN_EXPIRED_MESSAGE.to_string(),
        ));
    };
    let client_id = db::get_meta(conn, META_GOOGLE_CLIENT_ID)?.ok_or_else(|| {
        CoreError::InvalidInput(
            "Google client ID is not configured. Set it in the desktop app settings".to_string(),
        )
    })?;
    let client_secret = tokens::load_client_secret()?;
    let refreshed =
        oauth::refresh_access_token(&client_id, client_secret.as_deref(), &refresh_token)?;
    let updated = TokenSet {
        access_token: refreshed.access_token,
        // Google may omit the refresh token on refresh; keep the old one.
        refresh_token: refreshed.refresh_token.or(Some(refresh_token)),
        expires_at: tokens::expiry_from_now(refreshed.expires_in),
    };
    tokens::save(account_key, &updated)?;
    Ok(updated.access_token)
}

/// One-time startup migration from the pre-multi-account single-account
/// scheme. If a legacy "google_sheets" keychain entry still exists, moves its
/// tokens under a keyed entry and rewrites the bare "google-sheets" source row
/// into "google-sheets:{accountKey}". The account key comes from the legacy
/// source name's email, falling back to "default". Idempotent and best-effort:
/// a missing legacy entry is a no-op, and any failure is returned so the caller
/// can log it without blocking startup.
pub fn migrate_legacy_account(conn: &Connection) -> Result<(), CoreError> {
    let Some(legacy_tokens) = tokens::load_legacy()? else {
        // Nothing to migrate. Clear a stray bare source row if the legacy
        // token is already gone but the row lingers, so listings stay clean.
        if let Some(email) = legacy_source_email(conn)? {
            let account_key = account_key_from_email(&email);
            rewrite_legacy_source(conn, &account_key, &email)?;
        }
        return Ok(());
    };

    let email = match legacy_source_email(conn)? {
        Some(email) => email,
        // No source row: recover the email from the token so the account is
        // still labelled; fall back to the default key when even that fails.
        None => oauth::fetch_user_email(&legacy_tokens.access_token)
            .unwrap_or_else(|_| DEFAULT_ACCOUNT_KEY.to_string()),
    };
    let account_key = account_key_from_email(&email);

    tokens::save(&account_key, &legacy_tokens)?;
    rewrite_legacy_source(conn, &account_key, &email)?;
    tokens::delete_legacy()?;
    Ok(())
}

/// The email on the legacy bare "google-sheets" source row, if that row exists.
fn legacy_source_email(conn: &Connection) -> Result<Option<String>, CoreError> {
    Ok(sources::list(conn)?
        .into_iter()
        .find(|source| source.id == GOOGLE_SOURCE_ID)
        .map(|source| email_from_source_name(&source.name)))
}

/// Replaces the bare "google-sheets" source row with the keyed one, preserving
/// the email label. Removing the old row first keeps the id set clean.
fn rewrite_legacy_source(
    conn: &Connection,
    account_key: &str,
    email: &str,
) -> Result<(), CoreError> {
    sources::upsert(
        conn,
        &source_id_for(account_key),
        SourceKind::GoogleSheets,
        &format!("Google Sheets ({email})"),
        sources::SOURCE_STATUS_CONNECTED,
    )?;
    sources::delete(conn, GOOGLE_SOURCE_ID)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared HTTP plumbing for Google endpoints (also used by the connector)
// ---------------------------------------------------------------------------

pub(crate) fn http_client() -> Result<reqwest::blocking::Client, CoreError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|error| CoreError::Storage(format!("Could not build the HTTP client: {error}")))
}

pub(crate) fn get_json(token: &str, url: &str) -> Result<Value, CoreError> {
    let response = http_client()?
        .get(url)
        .bearer_auth(token)
        .send()
        .map_err(transport_error)?;
    parse_api_response(response)
}

pub(crate) fn post_json(token: &str, url: &str, body: &Value) -> Result<Value, CoreError> {
    let response = http_client()?
        .post(url)
        .bearer_auth(token)
        .json(body)
        .send()
        .map_err(transport_error)?;
    parse_api_response(response)
}

fn transport_error(error: reqwest::Error) -> CoreError {
    CoreError::Storage(format!("Could not reach Google: {error}"))
}

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
/// reconnect instruction agents and the desktop UI display verbatim.
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

fn open_in_browser(url: &str) -> Result<(), CoreError> {
    let spawned = if cfg!(target_os = "windows") {
        // rundll32 avoids cmd.exe quoting pitfalls around '&' in the URL.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    spawned.map(|_| ()).map_err(|error| {
        CoreError::Storage(format!(
            "Could not open the system browser for Google sign-in: {error}"
        ))
    })
}

/// Stores (or clears, when empty) the OAuth client secret Google issues for
/// desktop clients. Kept in the OS keychain next to the tokens.
pub fn set_client_secret(secret: &str) -> Result<(), CoreError> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        tokens::delete_client_secret()
    } else {
        tokens::save_client_secret(trimmed)
    }
}

/// Whether a client secret is stored; the secret itself never leaves the
/// google module.
pub fn has_client_secret() -> Result<bool, CoreError> {
    Ok(tokens::load_client_secret()?.is_some())
}

#[cfg(test)]
#[path = "google_tests.rs"]
mod multi_account_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_maps_401_to_the_reconnect_contract_message() {
        let error = api_error(401, "{\"error\":{\"message\":\"Invalid Credentials\"}}");
        assert!(matches!(error, CoreError::PermissionDenied(_)));
        assert_eq!(
            error.to_string(),
            "Google token expired or revoked, reconnect in the desktop app"
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
    fn connect_rejects_an_empty_client_id() {
        let conn = crate::db::test_support::open_temp_db();
        let error = connect(&conn, "   ").expect_err("must reject");
        assert!(matches!(error, CoreError::InvalidInput(_)));
        assert_eq!(error.to_string(), "Google client ID must not be empty");
    }
}
