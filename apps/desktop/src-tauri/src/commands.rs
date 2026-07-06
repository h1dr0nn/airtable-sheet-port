//! Tauri command wrappers. Names, argument names, and JSON field names match
//! docs/ipc.md exactly. All broker logic lives in `sheet-port-core`; each
//! wrapper only locks the shared connection and delegates. Commands that may
//! perform blocking connector HTTP (table reads, Google connect) are async
//! and run on a blocking task so the main thread never stalls.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::json;
use sheet_port_core::connectors::ConnectorRegistry;
use sheet_port_core::constants::{
    HEARTBEAT_STALE_MS, MCP_TRANSPORT_HTTP, MCP_TRANSPORT_STDIO, META_AUTO_APPROVE_WRITES,
    META_FLAG_ON, META_GOOGLE_CLIENT_ID, META_MCP_PORT, META_MCP_TRANSPORT, READ_LIMIT_DEFAULT,
    READ_LIMIT_MAX, READ_LIMIT_MIN,
};
use sheet_port_core::db::McpTransport;
use sheet_port_core::mcp_clients::{DetectedClient, ServerSpec};
use sheet_port_core::rusqlite::Connection;
use sheet_port_core::types::{
    AppStatus, AuditActor, AuditEvent, DataSource, PendingChange, PermissionRuleRow, ReadOptions,
    SavePermissionRule, TablePage, TableRef, TableSchema, TokenStatus,
};
use sheet_port_core::{
    audit, changes, db, google, heartbeat, mcp_clients, permissions, sources, vault, CoreError,
};
use tauri::State;

const POISONED_LOCK_MESSAGE: &str = "Database connection is unavailable (poisoned lock)";
const CLIENT_ID_MISSING_MESSAGE: &str =
    "Google client ID is not configured. Set it in the desktop app settings";

/// Connection, connector registry, and the resolved DB path, managed as Tauri
/// state. `Arc` so async commands can move clones onto blocking tasks.
pub struct DbState {
    pub conn: Arc<Mutex<Connection>>,
    pub registry: Arc<ConnectorRegistry>,
    pub path: PathBuf,
}

impl DbState {
    pub fn init() -> Result<Self, String> {
        let (conn, path) = db::open_default().map_err(|error| error.to_string())?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            registry: Arc::new(ConnectorRegistry::with_default_connectors()),
            path,
        })
    }
}

type Db<'a> = State<'a, DbState>;

fn lock_conn<'a>(state: &'a Db<'_>) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .conn
        .lock()
        .map_err(|_| POISONED_LOCK_MESSAGE.to_string())
}

/// Runs `task` on a blocking task with the shared connection and the
/// connector registry. Used by commands that may perform connector HTTP so
/// the Tauri main thread never blocks on the network.
async fn with_conn_blocking<T, F>(state: &Db<'_>, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Connection, &ConnectorRegistry) -> Result<T, CoreError> + Send + 'static,
{
    let conn = Arc::clone(&state.conn);
    let registry = Arc::clone(&state.registry);
    tauri::async_runtime::spawn_blocking(move || {
        let guard = conn.lock().map_err(|_| POISONED_LOCK_MESSAGE.to_string())?;
        task(&guard, &registry).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Background task failed: {error}"))?
}

#[tauri::command]
pub fn get_app_status(app: tauri::AppHandle, state: Db<'_>) -> Result<AppStatus, String> {
    let conn = lock_conn(&state)?;
    heartbeat::app_status(
        &conn,
        app.package_info().version.to_string(),
        state.path.display().to_string(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_sources(state: Db<'_>) -> Result<Vec<DataSource>, String> {
    let conn = lock_conn(&state)?;
    sources::list(&conn).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_tables(state: Db<'_>, source_id: String) -> Result<Vec<TableRef>, String> {
    with_conn_blocking(&state, move |conn, registry| {
        // Unknown sourceId stays Ok([]) per docs/ipc.md.
        if sources::get_kind(conn, &source_id)?.is_none() {
            return Ok(Vec::new());
        }
        registry.list_tables(conn, &source_id)
    })
    .await
}

#[tauri::command]
pub async fn describe_table(
    state: Db<'_>,
    source_id: String,
    table_id: String,
) -> Result<TableSchema, String> {
    with_conn_blocking(&state, move |conn, registry| {
        registry.describe_table(conn, &source_id, &table_id)
    })
    .await
}

#[tauri::command]
pub async fn read_table(
    state: Db<'_>,
    source_id: String,
    table_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<TablePage, String> {
    with_conn_blocking(&state, move |conn, registry| {
        // One unbounded connector read keeps `total` exact for every
        // connector (docs/ipc.md); the page window is sliced locally with
        // the documented bounds (default 100, clamp 1..=500, offset >= 0).
        let all = registry.read_table(conn, &source_id, &table_id, ReadOptions::default())?;
        let total = all.len() as i64;
        let limit = limit
            .unwrap_or(READ_LIMIT_DEFAULT)
            .clamp(READ_LIMIT_MIN, READ_LIMIT_MAX);
        let offset = offset.unwrap_or(0).max(0);
        let records = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(TablePage { records, total })
    })
    .await
}

#[tauri::command]
pub fn list_permission_rules(state: Db<'_>) -> Result<Vec<PermissionRuleRow>, String> {
    let conn = lock_conn(&state)?;
    permissions::list_rules(&conn).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_permission_rule(
    state: Db<'_>,
    rule: SavePermissionRule,
) -> Result<PermissionRuleRow, String> {
    let conn = lock_conn(&state)?;
    permissions::save_rule(&conn, &rule).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_permission_rule(state: Db<'_>, id: i64) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    permissions::delete_rule(&conn, id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_changes(state: Db<'_>, status: Option<String>) -> Result<Vec<PendingChange>, String> {
    let conn = lock_conn(&state)?;
    changes::list_changes(&conn, status.as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn approve_change(state: Db<'_>, change_id: String) -> Result<PendingChange, String> {
    let conn = lock_conn(&state)?;
    changes::decide_change(&conn, &change_id, changes::ChangeDecision::Approve)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reject_change(state: Db<'_>, change_id: String) -> Result<PendingChange, String> {
    let conn = lock_conn(&state)?;
    changes::decide_change(&conn, &change_id, changes::ChangeDecision::Reject)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_audit_events(
    state: Db<'_>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AuditEvent>, String> {
    let conn = lock_conn(&state)?;
    audit::list(&conn, limit, offset).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn token_status() -> Result<TokenStatus, String> {
    Ok(vault::token_status())
}

// ---------------------------------------------------------------------------
// App-managed settings (docs/ipc.md "Settings" section)
// ---------------------------------------------------------------------------

/// App-managed preferences stored in the shared `meta` table. Frontend-only
/// prefs (e.g. theme, kept in localStorage) are intentionally absent.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Auto-approve agent writes, bypassing the confirmation gate at commit
    /// time. Off (meta key absent or not "1") by default.
    pub auto_approve_writes: bool,
}

const SETTINGS_UPDATED_ACTION: &str = "settings_updated";
const SETTINGS_RESET_ACTION: &str = "settings_reset";

#[tauri::command]
pub fn get_settings(state: Db<'_>) -> Result<AppSettings, String> {
    let conn = lock_conn(&state)?;
    let auto_approve_writes = db::get_meta(&conn, META_AUTO_APPROVE_WRITES)
        .map_err(|error| error.to_string())?
        .as_deref()
        == Some(META_FLAG_ON);
    Ok(AppSettings {
        auto_approve_writes,
    })
}

/// Enables or disables auto-approve. Enabling writes meta "1"; disabling
/// deletes the key so it reads back as the absent default.
#[tauri::command]
pub fn set_auto_approve(state: Db<'_>, enabled: bool) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    if enabled {
        db::set_meta(&conn, META_AUTO_APPROVE_WRITES, META_FLAG_ON)
            .map_err(|error| error.to_string())?;
    } else {
        db::delete_meta(&conn, META_AUTO_APPROVE_WRITES).map_err(|error| error.to_string())?;
    }
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": META_AUTO_APPROVE_WRITES, "enabled": enabled })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Resets app-managed preferences to their defaults. Prefs-only: deletes the
/// auto-approve key and does NOT touch Google tokens, the client id/secret,
/// permission rules, sources, changes, or the audit log itself.
#[tauri::command]
pub fn reset_settings(state: Db<'_>) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    db::delete_meta(&conn, META_AUTO_APPROVE_WRITES).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_RESET_ACTION,
        None,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP transport config (docs/architecture.md, docs/security.md)
// ---------------------------------------------------------------------------

/// MCP sidecar transport settings plus live status. `transport`/`port` are the
/// persisted config; `running`/`boundPort` reflect the current sidecar. The
/// sidecar reads config once at startup, so a change only takes effect after it
/// restarts - the UI should say so.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigView {
    /// "stdio" | "http".
    pub transport: String,
    /// Configured HTTP port (only meaningful when transport is "http").
    pub port: u16,
    /// Whether a fresh sidecar heartbeat exists right now.
    pub running: bool,
    /// The port the running sidecar is reachable on. Only set when the sidecar
    /// is running AND the configured transport is "http"; the desktop cannot
    /// observe the sidecar's actual bound port across the DB, so it reports the
    /// configured port (which equals the bound port unless the config changed
    /// without a restart). Null for stdio or when not running.
    pub bound_port: Option<u16>,
}

/// Maps the "http" | anything-else meta string onto the canonical value.
fn transport_meta_string(transport: McpTransport) -> String {
    transport.as_str().to_string()
}

#[tauri::command]
pub fn get_mcp_config(state: Db<'_>) -> Result<McpConfigView, String> {
    let conn = lock_conn(&state)?;
    let config = db::get_mcp_config(&conn).map_err(|error| error.to_string())?;
    let running = heartbeat::status(&conn, HEARTBEAT_STALE_MS)
        .map_err(|error| error.to_string())?
        .running;
    // boundPort is only meaningful for a running HTTP sidecar.
    let bound_port = match (config.transport, running) {
        (McpTransport::Http, true) => Some(config.port),
        _ => None,
    };
    Ok(McpConfigView {
        transport: transport_meta_string(config.transport),
        port: config.port,
        running,
        bound_port,
    })
}

/// Persists the transport choice. Accepts only "stdio" | "http". Changing it
/// requires a sidecar restart to take effect (the command only writes config).
#[tauri::command]
pub fn set_mcp_transport(state: Db<'_>, transport: String) -> Result<(), String> {
    let parsed = match transport.as_str() {
        MCP_TRANSPORT_STDIO => McpTransport::Stdio,
        MCP_TRANSPORT_HTTP => McpTransport::Http,
        other => {
            return Err(format!(
                "transport must be \"{MCP_TRANSPORT_STDIO}\" or \"{MCP_TRANSPORT_HTTP}\", got \"{other}\""
            ))
        }
    };
    let conn = lock_conn(&state)?;
    db::set_mcp_transport(&conn, parsed).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": META_MCP_TRANSPORT, "transport": parsed.as_str() })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Persists the HTTP port after range validation (1024-65535). Changing it
/// requires a sidecar restart to take effect.
#[tauri::command]
pub fn set_mcp_port(state: Db<'_>, port: u16) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    db::set_mcp_port(&conn, port).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": META_MCP_PORT, "port": port })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP client auto-configuration (docs/development.md "MCP client config")
// ---------------------------------------------------------------------------

const MCP_CLIENT_CONFIGURED_ACTION: &str = "mcp_client_configured";
const MCP_CLIENT_UNREGISTERED_ACTION: &str = "mcp_client_unregistered";

/// The sidecar binary file name (with the platform executable extension).
#[cfg(target_os = "windows")]
const MCP_SIDECAR_BIN: &str = "sheet-port-mcp.exe";
#[cfg(not(target_os = "windows"))]
const MCP_SIDECAR_BIN: &str = "sheet-port-mcp";

/// Resolves the sheet-port-mcp binary the clients should launch. Prefers a
/// sibling of the running desktop executable (the bundled layout), then falls
/// back to the workspace `target/release/<bin>` used during development. The
/// path is returned even when the file does not exist yet so we can still
/// write config pointing at where the release build will land.
fn resolve_sidecar_bin() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(MCP_SIDECAR_BIN);
            if sibling.exists() {
                return sibling;
            }
            // Dev fallback: apps/desktop/src-tauri/target/<profile>/<exe> ->
            // walk up to the workspace root and into target/release.
            if let Some(workspace_release) = dev_workspace_release(dir) {
                return workspace_release;
            }
            // Last resort: advertise the expected sibling path anyway.
            return sibling;
        }
    }
    PathBuf::from(MCP_SIDECAR_BIN)
}

/// From a Tauri `target/<profile>` executable directory, finds the workspace
/// root's `target/release/<bin>`. Returns None when the layout does not match
/// (e.g. a bundled install), so the caller uses the sibling path instead.
fn dev_workspace_release(exe_dir: &std::path::Path) -> Option<PathBuf> {
    // exe_dir = .../apps/desktop/src-tauri/target/<profile>
    let target_dir = exe_dir.parent()?; // .../target
    if target_dir.file_name()?.to_str()? != "target" {
        return None;
    }
    let candidate = target_dir.join("release").join(MCP_SIDECAR_BIN);
    candidate.exists().then_some(candidate)
}

/// Builds the [`ServerSpec`] for the configured transport: an http url when the
/// transport is "http", otherwise a stdio launch of the resolved sidecar
/// binary. Returns the spec plus the sidecar path (for auditing) and whether
/// that binary exists yet.
fn server_spec_for_config(conn: &Connection) -> Result<(ServerSpec, PathBuf, bool), String> {
    let config = db::get_mcp_config(conn).map_err(|error| error.to_string())?;
    let bin = resolve_sidecar_bin();
    let exists = bin.exists();
    let spec = match config.transport {
        McpTransport::Http => ServerSpec::http_for_port(config.port),
        McpTransport::Stdio => ServerSpec::Stdio {
            command: bin.to_string_lossy().into_owned(),
            args: Vec::new(),
        },
    };
    Ok((spec, bin, exists))
}

/// One client's post-configure result: the id, the config file written, and
/// whether the sidecar binary the entry points at exists yet.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureResult {
    pub id: String,
    pub config_path: String,
    /// False when the release sidecar is not built yet; the entry still points
    /// at the expected path so the UI can prompt the user to build it.
    pub binary_exists: bool,
}

#[tauri::command]
pub fn mcp_detect_clients(state: Db<'_>) -> Result<Vec<DetectedClient>, String> {
    // No DB access needed, but keep the state arg for a uniform command shape.
    let _ = state;
    Ok(mcp_clients::detect_clients())
}

/// Writes our server entry into a single client's config, transport-aware.
#[tauri::command]
pub fn mcp_configure_client(state: Db<'_>, id: String) -> Result<ConfigureResult, String> {
    let conn = lock_conn(&state)?;
    let (spec, bin, binary_exists) = server_spec_for_config(&conn)?;
    let path = mcp_clients::configure_client(&id, &spec).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        MCP_CLIENT_CONFIGURED_ACTION,
        None,
        None,
        Some(&json!({
            "client": id,
            "configPath": path.to_string_lossy(),
            "binary": bin.to_string_lossy(),
            "binaryExists": binary_exists,
        })),
    )
    .map_err(|error| error.to_string())?;
    Ok(ConfigureResult {
        id,
        config_path: path.to_string_lossy().into_owned(),
        binary_exists,
    })
}

/// Removes only our server entry from a single client's config.
#[tauri::command]
pub fn mcp_unregister_client(state: Db<'_>, id: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    let removed = mcp_clients::unregister_client(&id).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        MCP_CLIENT_UNREGISTERED_ACTION,
        None,
        None,
        Some(&json!({
            "client": id,
            "configPath": removed.as_ref().map(|path| path.to_string_lossy()),
        })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Configures every detected, installed, detectable client in one call. Skips
/// clients that are not installed or not detectable; returns the results for
/// the ones actually written. A per-client failure aborts and surfaces its
/// error rather than leaving a partial-with-no-signal state.
#[tauri::command]
pub fn mcp_configure_all(state: Db<'_>) -> Result<Vec<ConfigureResult>, String> {
    let conn = lock_conn(&state)?;
    let (spec, bin, binary_exists) = server_spec_for_config(&conn)?;
    let targets: Vec<String> = mcp_clients::detect_clients()
        .into_iter()
        .filter(|client| client.installed && client.detectable)
        .map(|client| client.id)
        .collect();

    let mut results = Vec::with_capacity(targets.len());
    for id in targets {
        let path = mcp_clients::configure_client(&id, &spec).map_err(|error| error.to_string())?;
        audit::record(
            &conn,
            AuditActor::User,
            MCP_CLIENT_CONFIGURED_ACTION,
            None,
            None,
            Some(&json!({
                "client": id,
                "configPath": path.to_string_lossy(),
                "binary": bin.to_string_lossy(),
                "binaryExists": binary_exists,
            })),
        )
        .map_err(|error| error.to_string())?;
        results.push(ConfigureResult {
            id,
            config_path: path.to_string_lossy().into_owned(),
            binary_exists,
        });
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Google account linking (docs/ipc.md "Google Sheets account" section)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleConfig {
    pub client_id: Option<String>,
    pub connected_email: Option<String>,
    /// The secret itself never crosses IPC; the UI only needs presence.
    pub has_client_secret: bool,
}

#[derive(Serialize)]
pub struct GoogleConnectResult {
    pub email: String,
}

/// "Google Sheets (user@example.com)" -> "user@example.com"; any other
/// shape falls back to the raw source name so the UI still shows something.
fn email_from_source_name(name: &str) -> String {
    name.rfind('(')
        .and_then(|start| name[start + 1..].strip_suffix(')'))
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

#[tauri::command]
pub fn get_google_config(state: Db<'_>) -> Result<GoogleConfig, String> {
    let conn = lock_conn(&state)?;
    let client_id =
        db::get_meta(&conn, META_GOOGLE_CLIENT_ID).map_err(|error| error.to_string())?;
    let connected_email = sources::list(&conn)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| source.id == google::GOOGLE_SOURCE_ID)
        .map(|source| email_from_source_name(&source.name));
    Ok(GoogleConfig {
        client_id,
        connected_email,
        has_client_secret: google::has_client_secret().unwrap_or(false),
    })
}

#[tauri::command]
pub fn set_google_client_id(state: Db<'_>, client_id: String) -> Result<(), String> {
    let trimmed = client_id.trim();
    if trimmed.is_empty() {
        return Err("Google client ID must not be empty".to_string());
    }
    let conn = lock_conn(&state)?;
    db::set_meta(&conn, META_GOOGLE_CLIENT_ID, trimmed).map_err(|error| error.to_string())?;
    // The id itself is not audited: it is configuration, not an event detail.
    audit::record(
        &conn,
        AuditActor::User,
        "settings_updated",
        None,
        None,
        Some(&json!({ "key": META_GOOGLE_CLIENT_ID })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Runs the interactive OAuth flow. Blocks (on a blocking task) until the
/// user finishes or abandons the browser consent, so it uses its OWN SQLite
/// connection: holding the shared mutex for minutes would freeze every other
/// command (status polling included).
#[tauri::command]
pub async fn google_connect(state: Db<'_>) -> Result<GoogleConnectResult, String> {
    let db_path = state.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_at(&db_path).map_err(|error| error.to_string())?;
        let client_id = db::get_meta(&conn, META_GOOGLE_CLIENT_ID)
            .map_err(|error| error.to_string())?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CLIENT_ID_MISSING_MESSAGE.to_string())?;
        let email = match google::connect(&conn, &client_id) {
            Ok(email) => email,
            Err(error) => {
                let message = error.to_string();
                // Best-effort: a dismissed toast must not be the only trace of
                // a failed connect, so the audit log keeps the reason.
                let _ = audit::record(
                    &conn,
                    AuditActor::User,
                    "google_connect_failed",
                    Some(google::GOOGLE_SOURCE_ID),
                    None,
                    Some(&json!({ "error": message })),
                );
                return Err(message);
            }
        };
        audit::record(
            &conn,
            AuditActor::User,
            "google_connected",
            Some(google::GOOGLE_SOURCE_ID),
            None,
            Some(&json!({ "email": email })),
        )
        .map_err(|error| error.to_string())?;
        Ok(GoogleConnectResult { email })
    })
    .await
    .map_err(|error| format!("Background task failed: {error}"))?
}

/// Stores the Google OAuth client secret in the OS keychain (empty clears
/// it). Google requires it on token exchange even for desktop-type clients.
#[tauri::command]
pub fn set_google_client_secret(state: Db<'_>, client_secret: String) -> Result<(), String> {
    google::set_client_secret(&client_secret).map_err(|error| error.to_string())?;
    let conn = lock_conn(&state)?;
    audit::record(
        &conn,
        AuditActor::User,
        "settings_updated",
        None,
        None,
        Some(&json!({ "key": "google_client_secret" })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn google_disconnect(state: Db<'_>) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    google::disconnect(&conn).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        "google_disconnected",
        Some(google::GOOGLE_SOURCE_ID),
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}
