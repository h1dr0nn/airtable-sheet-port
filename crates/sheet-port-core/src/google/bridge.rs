//! Apps Script bridge client. A bridge is a web app the user deploys (execute
//! as the user, access "Anyone"); a POST with `{"secret": ...}` answers with a
//! short-lived Google OAuth access token and the signed-in email. Google
//! replies to the POST with a 302 to script.googleusercontent.com, which the
//! reqwest default redirect policy follows as a GET without the body (the
//! RFC 7231 POST-to-GET rewrite). The secret and the token never appear in
//! error messages.

use serde_json::{json, Value};
use url::Url;

use super::http_client;
use crate::error::CoreError;

/// The only host a bridge URL may point at.
const BRIDGE_HOST: &str = "script.google.com";

/// Token lifetime assumed when the bridge omits `expiresInSec`; below the
/// one-hour lifetime Google issues so the cache never outlives the token.
const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 3000;

/// Longest bridge-reported error text quoted back to the caller.
const BRIDGE_ERROR_MAX_CHARS: usize = 200;

/// Shown when the bridge answers with something other than JSON, which is what
/// Google does (a sign-in page) when the deployment is not public.
pub(crate) const NOT_JSON_MESSAGE: &str =
    "Bridge did not return JSON; deploy the web app with access 'Anyone'";

/// Shown when the bridge rejects the shared secret.
pub(crate) const UNAUTHORIZED_MESSAGE: &str =
    "The bridge rejected the secret; check it matches the SECRET in the Apps Script project";

/// A validated bridge location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BridgeUrl {
    /// The Apps Script deployment id.
    pub deployment_id: String,
    /// The canonical URL rebuilt from the parsed parts (no query, fragment,
    /// credentials, or port), so only a fixed Google endpoint is ever called.
    pub url: String,
}

/// What a bridge returns on success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BridgeToken {
    /// Google OAuth access token for the signed-in user.
    pub access_token: String,
    /// The email of the Google account the web app executes as.
    pub email: String,
    /// Remaining token lifetime in seconds.
    pub expires_in_secs: i64,
}

/// Validates a web app URL and extracts its deployment id. Accepts
/// `https://script.google.com/macros/s/{id}/exec` and the Workspace form
/// `https://script.google.com/a/macros/{domain}/s/{id}/exec`.
pub(crate) fn parse_bridge_url(input: &str) -> Result<BridgeUrl, CoreError> {
    let trimmed = input.trim();
    let parsed = Url::parse(trimmed).map_err(|_| invalid_url())?;
    if parsed.scheme() != "https" {
        return Err(CoreError::InvalidInput(
            "The bridge URL must start with https://".to_string(),
        ));
    }
    if parsed.host_str() != Some(BRIDGE_HOST) || parsed.port().is_some() {
        return Err(CoreError::InvalidInput(format!(
            "The bridge URL must be on {BRIDGE_HOST}"
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(invalid_url());
    }
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|segments| segments.collect())
        .unwrap_or_default();
    let (domain, deployment_id) = match segments.as_slice() {
        ["macros", "s", id, "exec"] => (None, *id),
        ["a", "macros", domain, "s", id, "exec"] if is_domain(domain) => (Some(*domain), *id),
        _ => return Err(invalid_url()),
    };
    if !is_deployment_id(deployment_id) {
        return Err(CoreError::InvalidInput(
            "The deployment id in the bridge URL may only contain letters, digits, '-' and '_'"
                .to_string(),
        ));
    }
    let url = match domain {
        Some(domain) => {
            format!("https://{BRIDGE_HOST}/a/macros/{domain}/s/{deployment_id}/exec")
        }
        None => format!("https://{BRIDGE_HOST}/macros/s/{deployment_id}/exec"),
    };
    Ok(BridgeUrl {
        deployment_id: deployment_id.to_string(),
        url,
    })
}

/// The shared wording for a URL that is not a web app `/exec` URL.
fn invalid_url() -> CoreError {
    CoreError::InvalidInput(format!(
        "Not an Apps Script web app URL. Copy the URL ending in /exec from Deploy > Manage deployments, e.g. https://{BRIDGE_HOST}/macros/s/DEPLOYMENT_ID/exec"
    ))
}

/// Non-empty `[A-Za-z0-9_-]`.
fn is_deployment_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

/// A plausible Workspace domain: non-empty `[A-Za-z0-9.-]`.
fn is_domain(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '.'
        })
}

/// Asks the bridge at `url` (already validated) for a fresh access token.
pub(crate) fn fetch_token(url: &str, secret: &str) -> Result<BridgeToken, CoreError> {
    let response = http_client()?
        .post(url)
        .json(&json!({ "secret": secret }))
        .send()
        .map_err(|error| {
            // The URL is stripped: after the redirect it is the
            // script.googleusercontent.com echo URL, whose user_content_key
            // serves the token reply to anyone who fetches it. The body (the
            // secret) is never part of the error.
            CoreError::Storage(format!(
                "Could not reach the Apps Script bridge: {}",
                error.without_url()
            ))
        })?;
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    parse_bridge_response(status, &body)
}

/// Interprets a bridge reply. Pure so the mapping is testable offline.
pub(crate) fn parse_bridge_response(status: u16, body: &str) -> Result<BridgeToken, CoreError> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Err(match status {
            404 => CoreError::NotFound(
                "The bridge deployment was not found; check the web app URL".to_string(),
            ),
            _ => CoreError::InvalidInput(NOT_JSON_MESSAGE.to_string()),
        });
    };
    if value["ok"].as_bool() != Some(true) {
        let reported = value["error"].as_str().unwrap_or("unknown error");
        if reported == "unauthorized" {
            return Err(CoreError::PermissionDenied(
                UNAUTHORIZED_MESSAGE.to_string(),
            ));
        }
        let snippet: String = reported.chars().take(BRIDGE_ERROR_MAX_CHARS).collect();
        return Err(CoreError::Storage(format!(
            "The Apps Script bridge reported an error: {snippet}"
        )));
    }
    let access_token = non_empty_string(&value, "accessToken")?;
    let email = non_empty_string(&value, "email")?;
    let expires_in_secs = value["expiresInSec"]
        .as_i64()
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_TOKEN_LIFETIME_SECS);
    Ok(BridgeToken {
        access_token,
        email,
        expires_in_secs,
    })
}

/// A required, non-blank string field of the bridge reply.
fn non_empty_string(value: &Value, field: &str) -> Result<String, CoreError> {
    value[field]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            CoreError::Storage(format!(
                "The Apps Script bridge reply is missing '{field}'; update the bridge script"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_consumer_web_app_url() {
        let parsed =
            parse_bridge_url(" https://script.google.com/macros/s/AKfy-cb_123/exec ").expect("ok");
        assert_eq!(parsed.deployment_id, "AKfy-cb_123");
        assert_eq!(
            parsed.url,
            "https://script.google.com/macros/s/AKfy-cb_123/exec"
        );
    }

    #[test]
    fn parses_the_workspace_web_app_url() {
        let parsed =
            parse_bridge_url("https://script.google.com/a/macros/corp.example.com/s/AKfy123/exec")
                .expect("ok");
        assert_eq!(parsed.deployment_id, "AKfy123");
        assert_eq!(
            parsed.url,
            "https://script.google.com/a/macros/corp.example.com/s/AKfy123/exec"
        );
    }

    #[test]
    fn canonical_url_drops_query_and_fragment() {
        let parsed = parse_bridge_url("https://script.google.com/macros/s/AKfy123/exec?x=1#frag")
            .expect("ok");
        assert_eq!(
            parsed.url,
            "https://script.google.com/macros/s/AKfy123/exec"
        );
    }

    #[test]
    fn rejects_urls_that_are_not_web_app_exec_urls() {
        for input in [
            "https://evil.example.com/macros/s/AKfy123/exec",
            "https://script.google.com.evil.com/macros/s/AKfy123/exec",
            "http://script.google.com/macros/s/AKfy123/exec",
            "https://script.google.com/macros/s/AKfy123",
            "https://script.google.com/macros/s/AKfy123/dev",
            "https://script.google.com/macros/s//exec",
            "https://script.google.com/macros/s/AK%20fy/exec",
            "https://script.google.com/macros/s/AK.fy/exec",
            "https://script.google.com:8443/macros/s/AKfy123/exec",
            "https://user:pw@script.google.com/macros/s/AKfy123/exec",
            "not a url",
            "",
        ] {
            let error = parse_bridge_url(input).expect_err(input);
            assert!(matches!(error, CoreError::InvalidInput(_)), "{input}");
        }
    }

    #[test]
    fn http_and_wrong_host_get_specific_messages() {
        assert_eq!(
            parse_bridge_url("http://script.google.com/macros/s/AKfy123/exec")
                .expect_err("http")
                .to_string(),
            "The bridge URL must start with https://"
        );
        assert_eq!(
            parse_bridge_url("https://example.com/macros/s/AKfy123/exec")
                .expect_err("host")
                .to_string(),
            "The bridge URL must be on script.google.com"
        );
    }

    #[test]
    fn transport_errors_do_not_quote_the_request_url() {
        // Nothing listens on port 1, so this fails fast without the network.
        let error = fetch_token(
            "https://127.0.0.1:1/macros/echo?user_content_key=KEY123&lib=L",
            "s3cret",
        )
        .expect_err("unreachable");
        let message = error.to_string();
        assert!(
            message.starts_with("Could not reach the Apps Script bridge"),
            "{message}"
        );
        assert!(!message.contains("KEY123"), "{message}");
        assert!(!message.contains("127.0.0.1"), "{message}");
        assert!(!message.contains("s3cret"), "{message}");
    }

    #[test]
    fn parses_a_successful_reply() {
        let token = parse_bridge_response(
            200,
            r#"{"ok":true,"accessToken":"ya29.x","email":"a@b.com","expiresInSec":3000}"#,
        )
        .expect("ok");
        assert_eq!(
            token,
            BridgeToken {
                access_token: "ya29.x".to_string(),
                email: "a@b.com".to_string(),
                expires_in_secs: 3000,
            }
        );
    }

    #[test]
    fn missing_lifetime_falls_back_to_the_default() {
        let token =
            parse_bridge_response(200, r#"{"ok":true,"accessToken":"t","email":"a@b.com"}"#)
                .expect("ok");
        assert_eq!(token.expires_in_secs, DEFAULT_TOKEN_LIFETIME_SECS);
    }

    #[test]
    fn maps_bridge_failures_to_clear_errors() {
        let unauthorized =
            parse_bridge_response(200, r#"{"ok":false,"error":"unauthorized"}"#).expect_err("401");
        assert!(matches!(unauthorized, CoreError::PermissionDenied(_)));
        assert_eq!(unauthorized.to_string(), UNAUTHORIZED_MESSAGE);

        let html =
            parse_bridge_response(200, "<!doctype html><title>Sign in</title>").expect_err("html");
        assert_eq!(html.to_string(), NOT_JSON_MESSAGE);

        let missing = parse_bridge_response(404, "<html>Not Found</html>").expect_err("404");
        assert!(matches!(missing, CoreError::NotFound(_)));

        let no_token =
            parse_bridge_response(200, r#"{"ok":true,"email":"a@b.com"}"#).expect_err("no token");
        assert!(no_token.to_string().contains("'accessToken'"));

        let other =
            parse_bridge_response(200, r#"{"ok":false,"error":"boom"}"#).expect_err("other");
        assert_eq!(
            other.to_string(),
            "The Apps Script bridge reported an error: boom"
        );
    }
}
