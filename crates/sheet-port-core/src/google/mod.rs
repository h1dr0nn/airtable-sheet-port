//! Google Sheets account linking: OAuth 2.0 PKCE for desktop apps, keyring
//! token storage, and the connect/disconnect lifecycle. Raw tokens NEVER
//! leave this module; the connector obtains short-lived access tokens via
//! the crate-private [`access_token`], and other processes only ever see the
//! boolean from `vault::token_status`. Audit events are recorded by callers.

mod oauth;
mod tokens;

use std::time::Duration;

use rusqlite::Connection;
use serde_json::Value;

use crate::constants::META_GOOGLE_CLIENT_ID;
use crate::db;
use crate::error::CoreError;
use crate::sources;
use crate::types::SourceKind;

pub(crate) use tokens::TokenSet;

/// The single Google Sheets source row the desktop app manages.
pub const GOOGLE_SOURCE_ID: &str = "google-sheets";

/// Contract wording for expired/revoked credentials (docs/mcp-tools.md).
pub(crate) const TOKEN_EXPIRED_MESSAGE: &str =
    "Google token expired or revoked, reconnect in the desktop app";

/// Contract wording when Sheets is used before an account is linked.
pub(crate) const NOT_CONNECTED_MESSAGE: &str =
    "Google Sheets is not connected. Connect it in the Airtable - Sheet Port desktop app first";

const HTTP_TIMEOUT_SECS: u64 = 30;
/// Longest raw body slice quoted back in error messages.
const ERROR_SNIPPET_MAX_CHARS: usize = 200;

/// Runs the full interactive OAuth flow: opens the system browser on the
/// consent page, waits on the loopback redirect, exchanges the code (PKCE, no
/// client secret), stores tokens in the OS keychain, and upserts the
/// `google-sheets` source row. Returns the connected account email.
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
        tokens::save(&token_set)?;
        // The refresh flow needs the client id later even if the desktop app
        // never stored it explicitly.
        db::set_meta(conn, META_GOOGLE_CLIENT_ID, client_id)?;
        sources::upsert(
            conn,
            GOOGLE_SOURCE_ID,
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

/// Removes the keychain credential and the `google-sheets` source row.
/// Idempotent: disconnecting twice is not an error.
pub fn disconnect(conn: &Connection) -> Result<(), CoreError> {
    tokens::delete()?;
    sources::delete(conn, GOOGLE_SOURCE_ID)?;
    Ok(())
}

/// True when a Google credential exists in the OS keychain.
pub(crate) fn has_token() -> bool {
    tokens::has_token()
}

/// A currently-valid access token, refreshing through the stored refresh
/// token when expired. Crate-private on purpose: raw tokens never leave core.
pub(crate) fn access_token(conn: &Connection) -> Result<String, CoreError> {
    let Some(stored) = tokens::load()? else {
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
    tokens::save(&updated)?;
    Ok(updated.access_token)
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
