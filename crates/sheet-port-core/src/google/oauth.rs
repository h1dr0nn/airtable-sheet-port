//! OAuth 2.0 authorization code flow with PKCE (RFC 7636) for a desktop app:
//! no client secret, loopback redirect on an ephemeral 127.0.0.1 port, and a
//! single-purpose HTTP responder that shows a small success page. Only the
//! parent module drives this; tokens leave through [`super::tokens`].

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use super::tokens::{expiry_from_now, TokenSet};
use super::{error_snippet, http_client, TOKEN_EXPIRED_MESSAGE};
use crate::error::CoreError;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const USERINFO_ENDPOINT: &str = "https://openidconnect.googleapis.com/v1/userinfo";

/// Sheets read/write + Drive metadata for spreadsheet discovery, plus
/// openid/email so the connected account can be labelled in the UI.
const OAUTH_SCOPES: &str = "openid email \
    https://www.googleapis.com/auth/spreadsheets \
    https://www.googleapis.com/auth/drive.metadata.readonly";

const LOOPBACK_HOST: &str = "127.0.0.1";
const CALLBACK_PATH: &str = "/callback";
/// How long the loopback server waits for the browser redirect.
const CALLBACK_TIMEOUT_SECS: u64 = 300;
const ACCEPT_POLL_INTERVAL_MS: u64 = 200;
const STREAM_READ_TIMEOUT_SECS: u64 = 10;
/// Safety cap while draining request headers from the browser.
const MAX_HEADER_LINES: usize = 100;

const SUCCESS_HTML: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>Airtable - Sheet Port</title></head>\
<body style=\"font-family:system-ui,sans-serif;margin:4rem auto;max-width:32rem;text-align:center\">\
<h1>Google Sheets connected</h1>\
<p>You can close this tab and return to Airtable - Sheet Port.</p></body></html>";

const NOT_FOUND_HTML: &str = "<!doctype html><html><body>Not found</body></html>";

/// Raw token endpoint response (authorization-code exchange and refresh).
#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub expires_in: i64,
}

/// A started PKCE flow: the loopback listener is bound and the consent URL is
/// ready to open in the system browser.
pub(crate) struct AuthFlow {
    listener: TcpListener,
    client_id: String,
    redirect_uri: String,
    verifier: String,
    state: String,
    consent_url: String,
}

impl AuthFlow {
    pub(crate) fn start(client_id: &str) -> Result<Self, CoreError> {
        let listener = TcpListener::bind((LOOPBACK_HOST, 0)).map_err(|error| {
            CoreError::Storage(format!(
                "Could not bind a loopback port for the Google sign-in callback: {error}"
            ))
        })?;
        let port = listener
            .local_addr()
            .map_err(|error| {
                CoreError::Storage(format!("Could not read the loopback port: {error}"))
            })?
            .port();
        let redirect_uri = format!("http://{LOOPBACK_HOST}:{port}{CALLBACK_PATH}");
        let verifier = generate_verifier();
        let challenge = challenge_for(&verifier);
        let state = uuid::Uuid::new_v4().simple().to_string();
        let consent_url = build_consent_url(client_id, &redirect_uri, &challenge, &state)?;
        Ok(Self {
            listener,
            client_id: client_id.to_string(),
            redirect_uri,
            verifier,
            state,
            consent_url,
        })
    }

    pub(crate) fn consent_url(&self) -> &str {
        &self.consent_url
    }

    /// Blocks until the browser redirects back (or the timeout hits), then
    /// exchanges the authorization code for tokens. The browser tab is kept
    /// waiting: the caller decides the final page via the returned responder,
    /// so a failure after the redirect never shows a false success page.
    pub(crate) fn wait_for_tokens(self) -> Result<(TokenSet, CallbackResponder), CoreError> {
        let (code, stream) = wait_for_callback(&self.listener, &self.state)?;
        let responder = CallbackResponder::new(stream);
        let response =
            match exchange_code(&self.client_id, &code, &self.verifier, &self.redirect_uri) {
                Ok(response) => response,
                Err(error) => {
                    responder.fail(&error.to_string());
                    return Err(error);
                }
            };
        Ok((
            TokenSet {
                access_token: response.access_token,
                refresh_token: response.refresh_token,
                expires_at: expiry_from_now(response.expires_in),
            },
            responder,
        ))
    }
}

/// Holds the browser's callback connection until the connect flow finishes,
/// then renders the real outcome. Dropping it unanswered shows a generic
/// failure page instead of leaving the tab hanging.
pub(crate) struct CallbackResponder {
    stream: Option<TcpStream>,
}

impl CallbackResponder {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream: Some(stream),
        }
    }

    pub(crate) fn succeed(mut self) {
        if let Some(mut stream) = self.stream.take() {
            write_response(&mut stream, "200 OK", SUCCESS_HTML);
        }
    }

    pub(crate) fn fail(mut self, message: &str) {
        if let Some(mut stream) = self.stream.take() {
            write_response(&mut stream, "200 OK", &error_html(message));
        }
    }
}

impl Drop for CallbackResponder {
    fn drop(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            write_response(
                &mut stream,
                "200 OK",
                &error_html("The connection attempt did not finish."),
            );
        }
    }
}

/// 64 unreserved characters from two UUIDv4s (~244 bits of entropy), inside
/// the RFC 7636 43..=128 length window.
pub(crate) fn generate_verifier() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// S256 code challenge: BASE64URL-ENCODE(SHA256(ASCII(verifier))), no padding.
pub(crate) fn challenge_for(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

pub(crate) fn build_consent_url(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> Result<String, CoreError> {
    let url = Url::parse_with_params(
        AUTH_ENDPOINT,
        &[
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("scope", OAUTH_SCOPES),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
            // offline + consent so Google issues a refresh token to the
            // desktop app.
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .map_err(|error| CoreError::Storage(format!("Could not build Google consent URL: {error}")))?;
    Ok(url.into())
}

/// Accept-loop with deadline: serves 404 to stray requests (favicon and the
/// like) and returns the authorization code from the /callback redirect along
/// with the still-open browser connection so the final page can be deferred.
fn wait_for_callback(
    listener: &TcpListener,
    expected_state: &str,
) -> Result<(String, TcpStream), CoreError> {
    listener.set_nonblocking(true).map_err(|error| {
        CoreError::Storage(format!(
            "Could not configure the callback listener: {error}"
        ))
    })?;
    let deadline = Instant::now() + Duration::from_secs(CALLBACK_TIMEOUT_SECS);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(callback) = handle_request(stream, expected_state)? {
                    return Ok(callback);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(CoreError::Conflict(
                        "Timed out waiting for the Google sign-in to finish in the browser"
                            .to_string(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(ACCEPT_POLL_INTERVAL_MS));
            }
            Err(error) => {
                return Err(CoreError::Storage(format!(
                    "Google sign-in callback server failed: {error}"
                )))
            }
        }
    }
}

/// Handles one browser request. Ok(Some((code, stream))) when the callback
/// carried a valid authorization code (the response is deferred to the
/// caller); Ok(None) for unrelated requests (keep waiting); Err for a
/// denied/invalid authorization.
fn handle_request(
    mut stream: TcpStream,
    expected_state: &str,
) -> Result<Option<(String, TcpStream)>, CoreError> {
    // The accepted socket may inherit non-blocking mode from the listener on
    // some platforms; reads below must block (with a timeout).
    if let Err(error) = stream
        .set_nonblocking(false)
        .and_then(|()| stream.set_read_timeout(Some(Duration::from_secs(STREAM_READ_TIMEOUT_SECS))))
    {
        eprintln!("[sheet-port] OAuth callback socket setup failed: {error}");
        return Ok(None);
    }

    let Some(target) = read_request_target(&stream) else {
        return Ok(None);
    };
    if target != CALLBACK_PATH && !target.starts_with(&format!("{CALLBACK_PATH}?")) {
        write_response(&mut stream, "404 Not Found", NOT_FOUND_HTML);
        return Ok(None);
    }

    let Ok(parsed) = Url::parse(&format!("http://{LOOPBACK_HOST}{target}")) else {
        write_response(&mut stream, "400 Bad Request", NOT_FOUND_HTML);
        return Ok(None);
    };
    let mut code = None;
    let mut state = None;
    let mut error_param = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error_param = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(reason) = error_param {
        write_response(
            &mut stream,
            "200 OK",
            &error_html("Google authorization was not completed."),
        );
        return Err(CoreError::PermissionDenied(format!(
            "Google authorization failed: {reason}"
        )));
    }
    if state.as_deref() != Some(expected_state) {
        write_response(
            &mut stream,
            "200 OK",
            &error_html("This sign-in attempt could not be verified."),
        );
        return Err(CoreError::PermissionDenied(
            "Google authorization state mismatch, start the connection again".to_string(),
        ));
    }
    match code {
        Some(code) if !code.is_empty() => Ok(Some((code, stream))),
        _ => {
            write_response(
                &mut stream,
                "200 OK",
                &error_html("Google did not return an authorization code."),
            );
            Err(CoreError::InvalidInput(
                "Google authorization callback had no code parameter".to_string(),
            ))
        }
    }
}

/// Reads the request line ("GET /callback?... HTTP/1.1"), drains the headers
/// so the browser sees a clean close, and returns the request target.
fn read_request_target(stream: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return None;
    }
    let target = request_line.split_whitespace().nth(1)?.to_string();
    for _ in 0..MAX_HEADER_LINES {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Some(target)
}

fn error_html(message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>Airtable - Sheet Port</title></head>\
<body style=\"font-family:system-ui,sans-serif;margin:4rem auto;max-width:32rem;text-align:center\">\
<h1>Sign-in not completed</h1><p>{message} You can close this tab and try again \
from the Airtable - Sheet Port desktop app.</p></body></html>"
    )
}

/// Best-effort response write: at this point the flow outcome is already
/// decided, so render failures are logged instead of failing the connect.
fn write_response(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(error) = stream
        .write_all(response.as_bytes())
        .and_then(|()| stream.flush())
    {
        eprintln!("[sheet-port] could not write OAuth callback response: {error}");
    }
}

/// Exchanges the authorization code using the PKCE verifier; desktop clients
/// send no client secret.
pub(crate) fn exchange_code(
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, CoreError> {
    request_tokens(
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ],
        |message| {
            CoreError::PermissionDenied(format!(
                "Google authorization code exchange failed: {message}"
            ))
        },
    )
}

pub(crate) fn refresh_access_token(
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse, CoreError> {
    request_tokens(
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ],
        |_| CoreError::PermissionDenied(TOKEN_EXPIRED_MESSAGE.to_string()),
    )
}

fn request_tokens(
    params: &[(&str, &str)],
    auth_failure: impl FnOnce(String) -> CoreError,
) -> Result<TokenResponse, CoreError> {
    let response = http_client()?
        .post(TOKEN_ENDPOINT)
        .form(params)
        .send()
        .map_err(|error| {
            CoreError::Storage(format!(
                "Could not reach the Google token endpoint: {error}"
            ))
        })?;
    let status = response.status();
    let body = response.text().map_err(|error| {
        CoreError::Storage(format!("Could not read the Google token response: {error}"))
    })?;
    if status.is_success() {
        return serde_json::from_str(&body).map_err(|error| {
            CoreError::Storage(format!("Google token response was not valid JSON: {error}"))
        });
    }
    let message = error_snippet(&body);
    if status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::UNAUTHORIZED {
        Err(auth_failure(message))
    } else {
        Err(CoreError::Storage(format!(
            "Google token endpoint returned {status}: {message}"
        )))
    }
}

/// The signed-in account's email via the OpenID userinfo endpoint (requires
/// the openid + email scopes requested at consent).
pub(crate) fn fetch_user_email(access_token: &str) -> Result<String, CoreError> {
    let body = super::get_json(access_token, USERINFO_ENDPOINT)?;
    body["email"].as_str().map(str::to_string).ok_or_else(|| {
        CoreError::Storage("Google userinfo response did not include an email".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn challenge_matches_the_rfc_7636_appendix_b_vector() {
        // https://www.rfc-editor.org/rfc/rfc7636#appendix-B
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            challenge_for(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifiers_are_valid_and_unique() {
        let first = generate_verifier();
        let second = generate_verifier();
        assert_eq!(first.len(), 64, "inside the RFC 7636 43..=128 window");
        assert!(
            first.chars().all(|c| c.is_ascii_alphanumeric()),
            "only unreserved characters: {first}"
        );
        assert_ne!(first, second, "verifiers must be random");
    }

    #[test]
    fn consent_url_carries_pkce_scopes_and_offline_access() {
        let url = build_consent_url(
            "client-123.apps.googleusercontent.com",
            "http://127.0.0.1:49152/callback",
            "challenge-abc",
            "state-xyz",
        )
        .expect("build url");
        let parsed = Url::parse(&url).expect("parse url");
        assert_eq!(parsed.host_str(), Some("accounts.google.com"));
        assert_eq!(parsed.path(), "/o/oauth2/v2/auth");

        let query: HashMap<String, String> = parsed
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        assert_eq!(
            query.get("client_id").map(String::as_str),
            Some("client-123.apps.googleusercontent.com")
        );
        assert_eq!(
            query.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:49152/callback")
        );
        assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            query.get("code_challenge").map(String::as_str),
            Some("challenge-abc")
        );
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(query.get("state").map(String::as_str), Some("state-xyz"));
        assert_eq!(
            query.get("access_type").map(String::as_str),
            Some("offline")
        );
        assert_eq!(query.get("prompt").map(String::as_str), Some("consent"));

        let scope = query.get("scope").expect("scope present");
        assert!(scope.contains("https://www.googleapis.com/auth/spreadsheets"));
        assert!(scope.contains("https://www.googleapis.com/auth/drive.metadata.readonly"));
        assert!(scope.contains("openid"));
        assert!(scope.contains("email"));
    }

    #[test]
    fn auth_flow_binds_an_ephemeral_loopback_redirect() {
        let flow = AuthFlow::start("client-123").expect("start flow");
        let parsed = Url::parse(flow.consent_url()).expect("parse consent url");
        let query: HashMap<String, String> = parsed
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        let redirect = query.get("redirect_uri").expect("redirect uri");
        let redirect_url = Url::parse(redirect).expect("parse redirect");
        assert_eq!(redirect_url.host_str(), Some(LOOPBACK_HOST));
        assert_eq!(redirect_url.path(), CALLBACK_PATH);
        assert!(redirect_url.port().is_some(), "ephemeral port bound");
    }
}
