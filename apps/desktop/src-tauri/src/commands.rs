//! Tauri command wrappers. Names, argument names, and JSON field names match
//! docs/ipc.md exactly. All broker logic lives in `sheet-port-core`; each
//! wrapper only locks the shared connection and delegates. Commands that may
//! perform blocking connector HTTP (table reads, Google bridges) are async
//! and run on a blocking task so the main thread never stalls.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::json;
use sheet_port_core::connectors::ConnectorRegistry;
use sheet_port_core::constants::{
    AUDIT_CLEARED_ACTION, HEARTBEAT_STALE_MS, MCP_TRANSPORT_HTTP, MCP_TRANSPORT_STDIO,
    META_CONFIGURED_MCP_CLIENTS, META_MCP_PORT, META_MCP_TRANSPORT, META_UI_FONT_FAMILY,
    META_UI_FONT_SCALE, META_UI_LANGUAGE, READ_LIMIT_DEFAULT, READ_LIMIT_MAX, READ_LIMIT_MIN,
};
use sheet_port_core::db::McpTransport;
use sheet_port_core::mcp_clients::{DetectedClient, ServerSpec};
use sheet_port_core::rusqlite::Connection;
use sheet_port_core::types::{
    AppStatus, AuditActor, AuditEvent, DataSource, GridData, GridRow, PermissionRuleRow,
    ReadOptions, SavePermissionRule, SheetTab, TablePage, TableRef, TableSchema, TokenStatus,
};
use sheet_port_core::workbench::{self, WorkbenchFolder, WorkbenchItem, WorkbenchTree};
use sheet_port_core::{
    audit, db, google, heartbeat, mcp_clients, permissions, sources, vault, CoreError,
};
use tauri::{Manager, State};

const POISONED_LOCK_MESSAGE: &str = "Database connection is unavailable (poisoned lock)";

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

/// The desktop-managed MCP sidecar child process, started on the configured
/// transport (stdio or http). At most one child is tracked at a time: `Some`
/// while running, `None` when stopped. Agent-side stdio MCP clients spawn their
/// own separate sidecar, which is never tracked here.
#[derive(Default)]
pub struct ManagedSidecar {
    pub child: Arc<Mutex<Option<std::process::Child>>>,
}

type Sidecar<'a> = State<'a, ManagedSidecar>;

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
pub fn list_audit_events(
    state: Db<'_>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AuditEvent>, String> {
    let conn = lock_conn(&state)?;
    // The activity feed hides the `audit_cleared` trace, so a clear leaves
    // the feed empty instead of showing the clear as its newest entry.
    audit::list_activity(&conn, limit, offset).map_err(|error| error.to_string())
}

/// Clears the entire audit log, then records a single `audit_cleared` event so
/// the wipe itself leaves a trace in the audit trail. The activity feed
/// (`list_audit_events`) does not show that event, so it reads as empty.
#[tauri::command]
pub fn clear_audit_log(state: Db<'_>) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    audit::clear(&conn).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        AUDIT_CLEARED_ACTION,
        None,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn token_status(state: Db<'_>) -> Result<TokenStatus, String> {
    let conn = lock_conn(&state)?;
    vault::token_status(&conn).map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// App-managed settings (docs/ipc.md "Settings" section)
// ---------------------------------------------------------------------------

/// App-managed preferences stored in the shared `meta` table. Frontend-only
/// prefs (e.g. theme, kept in localStorage) are intentionally absent.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// UI font scale: "small" | "normal" | "large" (default "normal").
    pub font_scale: String,
    /// UI font family: "classic" | "modern" | "system" (default "modern").
    pub font_family: String,
    /// UI language: "en" | "vi" (default "en").
    /// Autostart is intentionally NOT here; it lives in the OS launcher, so the
    /// UI reads it via `get_autostart_enabled`.
    pub language: String,
}

const SETTINGS_UPDATED_ACTION: &str = "settings_updated";
const SETTINGS_RESET_ACTION: &str = "settings_reset";

#[tauri::command]
pub fn get_settings(state: Db<'_>) -> Result<AppSettings, String> {
    let conn = lock_conn(&state)?;
    let font_scale = db::get_ui_font_scale(&conn).map_err(|error| error.to_string())?;
    let font_family = db::get_ui_font_family(&conn).map_err(|error| error.to_string())?;
    let language = db::get_language(&conn).map_err(|error| error.to_string())?;
    Ok(AppSettings {
        font_scale,
        font_family,
        language,
    })
}

/// Sets the UI language ("en" | "vi"); rejects any other value. Audit event
/// `settings_updated` with the persisted value.
#[tauri::command]
pub fn set_language(state: Db<'_>, language: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    db::set_language(&conn, &language).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": META_UI_LANGUAGE, "value": language })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Sets the UI font scale ("small" | "normal" | "large"); rejects any other
/// value. Audit event `settings_updated` with the persisted value.
#[tauri::command]
pub fn set_font_scale(state: Db<'_>, scale: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    db::set_ui_font_scale(&conn, &scale).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": META_UI_FONT_SCALE, "value": scale })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Sets the UI font family ("classic" | "modern" | "system"); rejects any other
/// value. Audit event `settings_updated` with the persisted value.
#[tauri::command]
pub fn set_font_family(state: Db<'_>, family: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    db::set_ui_font_family(&conn, &family).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": META_UI_FONT_FAMILY, "value": family })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Resets app-managed preferences to their defaults. Prefs-only: deletes the
/// appearance (font scale/family) and language keys and does NOT touch Google
/// bridges, permission rules, sources, changes, or the audit log itself.
#[tauri::command]
pub fn reset_settings(state: Db<'_>) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    db::delete_meta(&conn, META_UI_FONT_SCALE).map_err(|error| error.to_string())?;
    db::delete_meta(&conn, META_UI_FONT_FAMILY).map_err(|error| error.to_string())?;
    db::delete_meta(&conn, META_UI_LANGUAGE).map_err(|error| error.to_string())?;
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
/// root's built sidecar under `target/release` or `target/debug`. Returns None
/// when the layout does not match (e.g. a bundled install), so the caller uses
/// the sibling path instead. Checking debug too lets `tauri dev` auto-start the
/// sidecar without a separate release build.
fn dev_workspace_release(exe_dir: &std::path::Path) -> Option<PathBuf> {
    // exe_dir = .../apps/desktop/src-tauri/target/<profile>
    let target_dir = exe_dir.parent()?; // .../target
    if target_dir.file_name()?.to_str()? != "target" {
        return None;
    }
    for profile in ["release", "debug"] {
        let candidate = target_dir.join(profile).join(MCP_SIDECAR_BIN);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
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

/// The MCP client ids the user has configured from this app, from meta.
/// A missing or malformed value reads as an empty list.
fn configured_client_ids(conn: &Connection) -> Vec<String> {
    db::get_meta(conn, META_CONFIGURED_MCP_CLIENTS)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
}

fn save_configured_client_ids(conn: &Connection, ids: &[String]) -> Result<(), String> {
    let raw = serde_json::to_string(ids).map_err(|error| error.to_string())?;
    db::set_meta(conn, META_CONFIGURED_MCP_CLIENTS, &raw).map_err(|error| error.to_string())
}

/// Remembers a client the user configured so app launches re-register it.
fn remember_configured_client(conn: &Connection, id: &str) -> Result<(), String> {
    let mut ids = configured_client_ids(conn);
    if !ids.iter().any(|existing| existing == id) {
        ids.push(id.to_string());
        save_configured_client_ids(conn, &ids)?;
    }
    Ok(())
}

fn forget_configured_client(conn: &Connection, id: &str) -> Result<(), String> {
    let ids: Vec<String> = configured_client_ids(conn)
        .into_iter()
        .filter(|existing| existing != id)
        .collect();
    save_configured_client_ids(conn, &ids)
}

/// Re-registers every remembered client on app launch. Self-healing for two
/// real failure modes: the sidecar path going stale after an app update, and
/// clients that rewrite their config file from memory on exit (Claude Desktop)
/// silently dropping our entry. Best-effort: per-client failures are logged
/// and never block startup.
pub fn reregister_mcp_clients(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<DbState>() else {
        return;
    };
    let Ok(conn) = state.conn.lock() else {
        return;
    };
    let ids = configured_client_ids(&conn);
    if ids.is_empty() {
        return;
    }
    let (spec, _bin, _exists) = match server_spec_for_config(&conn) {
        Ok(resolved) => resolved,
        Err(error) => {
            eprintln!("[sheet-port] client re-register skipped: {error}");
            return;
        }
    };
    for id in ids {
        match mcp_clients::configure_client(&id, &spec) {
            Ok(path) => {
                let _ = audit::record(
                    &conn,
                    AuditActor::System,
                    MCP_CLIENT_CONFIGURED_ACTION,
                    None,
                    None,
                    Some(&json!({
                        "client": id,
                        "configPath": path.to_string_lossy(),
                        "reregisteredOnLaunch": true,
                    })),
                );
            }
            Err(error) => {
                eprintln!("[sheet-port] re-register of MCP client {id} failed: {error}");
            }
        }
    }
}

/// Writes our server entry into a single client's config, transport-aware.
#[tauri::command]
pub fn mcp_configure_client(state: Db<'_>, id: String) -> Result<ConfigureResult, String> {
    let conn = lock_conn(&state)?;
    let (spec, bin, binary_exists) = server_spec_for_config(&conn)?;
    let path = mcp_clients::configure_client(&id, &spec).map_err(|error| error.to_string())?;
    remember_configured_client(&conn, &id)?;
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
    forget_configured_client(&conn, &id)?;
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
        remember_configured_client(&conn, &id)?;
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
// Desktop-managed MCP sidecar process control.
// The desktop starts a managed child on the configured transport (stdio or
// http) so "running" status works for both: the child keeps the heartbeat row
// fresh regardless of transport. The env overrides pin the child onto exactly
// the configured transport + port so it never drifts from the stored meta.
// stdio MCP clients additionally spawn their OWN sidecar; that one is not
// tracked here (only this desktop-managed child is).
// ---------------------------------------------------------------------------

/// Env var the sidecar reads to force its transport (mirrors the private const
/// in `crates/sheet-port-mcp/src/main.rs`). Kept in sync via docs/ipc.md.
const ENV_MCP_TRANSPORT: &str = "SHEET_PORT_MCP_TRANSPORT";
/// Env var the sidecar reads to force its HTTP port.
const ENV_MCP_PORT: &str = "SHEET_PORT_MCP_PORT";

const MCP_SERVER_STARTED_ACTION: &str = "mcp_server_started";
const MCP_SERVER_STOPPED_ACTION: &str = "mcp_server_stopped";

/// Applies the Windows flag that stops the console-subsystem sidecar from
/// flashing a terminal window when the GUI desktop process spawns it. No-op on
/// other platforms. Without this a black console blinks on every launch/start.
#[cfg(target_os = "windows")]
fn hide_child_console(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: the child gets no console and does not inherit ours.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_child_console(_command: &mut std::process::Command) {}

/// The result of a start/stop request: whether a managed child is running now
/// and its PID when one is.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

/// Starts the sidecar as a desktop-managed child on the CONFIGURED transport
/// (stdio or http), so "running" status works for both: the child keeps the
/// heartbeat row fresh either way. The child is pinned onto exactly the stored
/// transport + port via env overrides so it never drifts from the meta config.
/// No-op error when a managed child is already running (only one is allowed).
/// The resolved binary must exist; a missing release build is a clear error
/// rather than a silent failure.
#[tauri::command]
pub fn mcp_server_start(state: Db<'_>, sidecar: Sidecar<'_>) -> Result<SidecarStatus, String> {
    let mut guard = sidecar
        .child
        .lock()
        .map_err(|_| "Managed sidecar state is unavailable (poisoned lock)".to_string())?;

    // Reap an already-exited child so a crashed process is not mistaken for a
    // running one, then guard against a genuinely live child.
    if let Some(existing) = guard.as_mut() {
        match existing.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
            }
            Ok(None) => {
                return Err("The MCP server is already running".to_string());
            }
            Err(error) => {
                return Err(format!("Could not check the MCP server state: {error}"));
            }
        }
    }

    let config = {
        let conn = lock_conn(&state)?;
        db::get_mcp_config(&conn).map_err(|error| error.to_string())?
    };
    let transport = config.transport.as_str();
    let port = config.port;

    let bin = resolve_sidecar_bin();
    if !bin.exists() {
        return Err(format!(
            "The MCP server binary was not found at {}. Build the release sidecar first",
            bin.display()
        ));
    }

    // Pipe stdin and keep the Child (which owns the write end) alive so the
    // stdio transport never sees EOF and exits: a desktop-launched GUI process
    // has no console stdin, so an inherited/null stdin would close immediately
    // and the sidecar would stop heartbeating. stdout/stderr are discarded.
    let mut command = std::process::Command::new(&bin);
    command
        .env(ENV_MCP_TRANSPORT, transport)
        .env(ENV_MCP_PORT, port.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    hide_child_console(&mut command);
    let child = command
        .spawn()
        .map_err(|error| format!("Could not start the MCP server: {error}"))?;
    let pid = child.id();
    *guard = Some(child);

    // Audit outside the child lock is fine; the DB lock is independent.
    {
        let conn = lock_conn(&state)?;
        audit::record(
            &conn,
            AuditActor::User,
            MCP_SERVER_STARTED_ACTION,
            None,
            None,
            Some(&json!({ "pid": pid, "port": port, "transport": transport })),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(SidecarStatus {
        running: true,
        pid: Some(pid),
    })
}

/// Best-effort auto-start of the managed sidecar at app launch so the server is
/// running without the user clicking Start. Everything here is swallowed and
/// logged: a missing binary, a busy DB lock, or a spawn failure must never block
/// startup. A sidecar that is already running (fresh heartbeat) is left alone.
pub fn auto_start_managed_sidecar(app: &tauri::AppHandle) {
    use tauri::Manager;

    let (Some(sidecar), Some(state)) = (
        app.try_state::<ManagedSidecar>(),
        app.try_state::<DbState>(),
    ) else {
        return;
    };
    let Ok(mut guard) = sidecar.child.lock() else {
        return;
    };
    // Already-managed child still alive: nothing to do.
    if let Some(existing) = guard.as_mut() {
        match existing.try_wait() {
            Ok(Some(_)) => *guard = None, // exited: fall through and relaunch
            Ok(None) => return,           // running
            Err(_) => return,
        }
    }

    let config = match state.conn.lock() {
        Ok(conn) => match db::get_mcp_config(&conn) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("[sheet-port] auto-start: could not read MCP config: {error}");
                return;
            }
        },
        Err(_) => return,
    };

    let bin = resolve_sidecar_bin();
    if !bin.exists() {
        eprintln!(
            "[sheet-port] auto-start skipped: sidecar binary not found at {}",
            bin.display()
        );
        return;
    }

    let mut command = std::process::Command::new(&bin);
    command
        .env(ENV_MCP_TRANSPORT, config.transport.as_str())
        .env(ENV_MCP_PORT, config.port.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    hide_child_console(&mut command);
    match command.spawn() {
        Ok(child) => *guard = Some(child),
        Err(error) => eprintln!("[sheet-port] auto-start failed to spawn sidecar: {error}"),
    }
}

/// Stops the desktop-managed sidecar child if one is running. Idempotent: no
/// managed child is not an error (the UI can call it safely).
#[tauri::command]
pub fn mcp_server_stop(state: Db<'_>, sidecar: Sidecar<'_>) -> Result<SidecarStatus, String> {
    let mut guard = sidecar
        .child
        .lock()
        .map_err(|_| "Managed sidecar state is unavailable (poisoned lock)".to_string())?;

    let stopped_pid = match guard.take() {
        Some(mut child) => {
            let pid = child.id();
            // Best-effort terminate + reap so the OS process does not linger.
            let _ = child.kill();
            let _ = child.wait();
            Some(pid)
        }
        None => None,
    };

    if let Some(pid) = stopped_pid {
        let conn = lock_conn(&state)?;
        audit::record(
            &conn,
            AuditActor::User,
            MCP_SERVER_STOPPED_ACTION,
            None,
            None,
            Some(&json!({ "pid": pid })),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(SidecarStatus {
        running: false,
        pid: None,
    })
}

// ---------------------------------------------------------------------------
// Google account linking (docs/ipc.md "Google Sheets account" section)
// ---------------------------------------------------------------------------

const GOOGLE_BRIDGE_ADDED_ACTION: &str = "google_bridge_added";
const GOOGLE_BRIDGE_REMOVED_ACTION: &str = "google_bridge_removed";
const GOOGLE_BRIDGE_TESTED_ACTION: &str = "google_bridge_tested";

/// Runs a bridge call on a blocking task with its OWN SQLite connection. A
/// bridge call waits on Apps Script (up to the HTTP timeout), and holding the
/// shared mutex that long would freeze every other command (status polling
/// included).
async fn with_own_conn_blocking<T, F>(state: &Db<'_>, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, CoreError> + Send + 'static,
{
    let db_path = state.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_at(&db_path).map_err(|error| error.to_string())?;
        task(&conn).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Background task failed: {error}"))?
}

/// Every connected Google account (one per bridge), ordered by source id.
#[tauri::command]
pub fn google_list_accounts(state: Db<'_>) -> Result<Vec<google::GoogleAccount>, String> {
    let conn = lock_conn(&state)?;
    google::list_accounts(&conn).map_err(|error| error.to_string())
}

/// Adds (or replaces) an Apps Script bridge from its web app URL and secret.
/// Fetches a token to learn the account email, so it runs off the async
/// runtime. The secret never reaches the audit log.
#[tauri::command]
pub async fn google_add_bridge(
    state: Db<'_>,
    url: String,
    secret: String,
) -> Result<google::GoogleAccount, String> {
    with_own_conn_blocking(&state, move |conn| {
        let account = google::add_bridge(conn, &url, &secret)?;
        audit::record(
            conn,
            AuditActor::User,
            GOOGLE_BRIDGE_ADDED_ACTION,
            Some(&account.source_id),
            None,
            Some(&json!({
                "email": account.email,
                "deploymentId": account.deployment_id,
            })),
        )?;
        Ok(account)
    })
    .await
}

/// Removes one bridge by its "google-sheets:{accountKey}" source id (its
/// keychain credential, source row, and cached routes). Idempotent.
#[tauri::command]
pub fn google_remove_bridge(state: Db<'_>, source_id: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    google::remove_bridge(&conn, &source_id).map_err(|error| error.to_string())?;
    audit::record(
        &conn,
        AuditActor::User,
        GOOGLE_BRIDGE_REMOVED_ACTION,
        Some(&source_id),
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Forces a fresh token fetch from one bridge. Blocks on the network, so it
/// runs off the async runtime. The audit event records the outcome either way.
#[tauri::command]
pub async fn google_test_bridge(
    state: Db<'_>,
    source_id: String,
) -> Result<google::GoogleAccount, String> {
    with_own_conn_blocking(&state, move |conn| {
        let result = google::test_bridge(conn, &source_id);
        let metadata = match &result {
            Ok(account) => json!({ "ok": true, "email": account.email }),
            Err(error) => json!({ "ok": false, "error": error.to_string() }),
        };
        // Best effort: a failed audit write must not hide the test result.
        if let Err(error) = audit::record(
            conn,
            AuditActor::User,
            GOOGLE_BRIDGE_TESTED_ACTION,
            Some(&source_id),
            None,
            Some(&metadata),
        ) {
            eprintln!("[sheet-port] could not audit the bridge test: {error}");
        }
        result
    })
    .await
}

// ---------------------------------------------------------------------------
// Window + launch at login. Closing the window quits the app (no tray or
// background mode); MCP clients spawn their own stdio sidecar, so they keep
// working after the app exits.
// ---------------------------------------------------------------------------

/// The main window label, matching `tauri.conf.json` app.windows[0].
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Brings the main window back into view and gives it focus. Used by the
/// single-instance handler when the app is launched a second time.
pub fn show_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Whether the app is registered to launch at login.
#[tauri::command]
pub fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|error| format!("Could not read the autostart state: {error}"))
}

/// Enables or disables launch at login. Audit event `settings_updated`.
#[tauri::command]
pub fn set_autostart_enabled(
    app: tauri::AppHandle,
    enabled: bool,
    state: Db<'_>,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|error| format!("Could not enable autostart: {error}"))?;
    } else {
        manager
            .disable()
            .map_err(|error| format!("Could not disable autostart: {error}"))?;
    }
    let conn = lock_conn(&state)?;
    audit::record(
        &conn,
        AuditActor::User,
        SETTINGS_UPDATED_ACTION,
        None,
        None,
        Some(&json!({ "key": "autostart_enabled", "enabled": enabled })),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Workbench (docs/ipc.md "Workbench"). Folder/item CRUD are quick DB ops and
// run synchronously; the sheet-tab, grid read, and grid write commands hit the
// connector (Google network) so they run on a blocking task like read_table.
// Grid writes are DIRECT (no pending-change flow): they audit as actor=user at
// the command boundary.
// ---------------------------------------------------------------------------

const WORKBENCH_CELL_UPDATED_ACTION: &str = "workbench_cell_updated";
const WORKBENCH_ROW_APPENDED_ACTION: &str = "workbench_row_appended";

/// The new 0-based data row index returned by `append_workbench_row`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendRowResult {
    pub row_index: i64,
}

/// Builds `{spreadsheetId}:{gid}` - the connector tableId that names the exact
/// sheet tab of a Workbench item.
fn table_id_for(item: &WorkbenchItem, gid: &str) -> String {
    format!("{}:{}", item.spreadsheet_id, gid)
}

#[tauri::command]
pub fn workbench_tree(state: Db<'_>) -> Result<WorkbenchTree, String> {
    let conn = lock_conn(&state)?;
    workbench::tree(&conn).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_workbench_folder(state: Db<'_>, name: String) -> Result<WorkbenchFolder, String> {
    let conn = lock_conn(&state)?;
    workbench::create_folder(&conn, &name).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_workbench_folder(state: Db<'_>, id: String, name: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    workbench::rename_folder(&conn, &id, &name).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_workbench_folder(state: Db<'_>, id: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    workbench::delete_folder(&conn, &id).map_err(|error| error.to_string())
}

/// Resolves the pasted URL/id via the connected Google account and adds it.
/// Runs on a blocking task because it fetches the spreadsheet title.
#[tauri::command]
pub async fn add_workbench_spreadsheet(
    state: Db<'_>,
    folder_id: Option<String>,
    url_or_id: String,
) -> Result<WorkbenchItem, String> {
    with_conn_blocking(&state, move |conn, registry| {
        workbench::add_spreadsheet(conn, registry, folder_id.as_deref(), &url_or_id)
    })
    .await
}

#[tauri::command]
pub fn remove_workbench_item(state: Db<'_>, id: String) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    workbench::remove_item(&conn, &id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn move_workbench_item(
    state: Db<'_>,
    id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    workbench::move_item(&conn, &id, folder_id.as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_workbench_sheet_tabs(
    state: Db<'_>,
    item_id: String,
) -> Result<Vec<SheetTab>, String> {
    with_conn_blocking(&state, move |conn, registry| {
        let item = workbench::get_item(conn, &item_id)?;
        registry.list_sheet_tabs(conn, &item.source_id, &item.spreadsheet_id)
    })
    .await
}

#[tauri::command]
pub async fn read_workbench_sheet(
    state: Db<'_>,
    item_id: String,
    gid: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<GridData, String> {
    with_conn_blocking(&state, move |conn, registry| {
        let item = workbench::get_item(conn, &item_id)?;
        registry.read_grid(
            conn,
            &item.source_id,
            &table_id_for(&item, &gid),
            limit,
            offset,
        )
    })
    .await
}

/// Writes one cell directly and audits it as a user action.
#[tauri::command]
pub async fn update_workbench_cell(
    state: Db<'_>,
    item_id: String,
    gid: String,
    row_index: i64,
    column_id: String,
    value: String,
) -> Result<(), String> {
    with_conn_blocking(&state, move |conn, registry| {
        let item = workbench::get_item(conn, &item_id)?;
        registry.write_cell(
            conn,
            &item.source_id,
            &table_id_for(&item, &gid),
            row_index,
            &column_id,
            &value,
        )?;
        audit::record(
            conn,
            AuditActor::User,
            WORKBENCH_CELL_UPDATED_ACTION,
            Some(&item.source_id),
            Some(&item.spreadsheet_id),
            Some(&json!({
                "itemId": item.id,
                "gid": gid,
                "rowIndex": row_index,
                "columnId": column_id,
            })),
        )?;
        Ok(())
    })
    .await
}

/// Appends a row directly, returning its new index, and audits it as a user
/// action.
#[tauri::command]
pub async fn append_workbench_row(
    state: Db<'_>,
    item_id: String,
    gid: String,
    values: GridRow,
) -> Result<AppendRowResult, String> {
    with_conn_blocking(&state, move |conn, registry| {
        let item = workbench::get_item(conn, &item_id)?;
        let row_index =
            registry.append_grid_row(conn, &item.source_id, &table_id_for(&item, &gid), &values)?;
        audit::record(
            conn,
            AuditActor::User,
            WORKBENCH_ROW_APPENDED_ACTION,
            Some(&item.source_id),
            Some(&item.spreadsheet_id),
            Some(&json!({ "itemId": item.id, "gid": gid, "rowIndex": row_index })),
        )?;
        Ok(AppendRowResult { row_index })
    })
    .await
}
