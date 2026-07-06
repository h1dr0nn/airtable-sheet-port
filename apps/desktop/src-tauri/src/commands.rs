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
    META_GOOGLE_CLIENT_ID, READ_LIMIT_DEFAULT, READ_LIMIT_MAX, READ_LIMIT_MIN,
};
use sheet_port_core::rusqlite::Connection;
use sheet_port_core::types::{
    AppStatus, AuditActor, AuditEvent, DataSource, PendingChange, PermissionRuleRow, ReadOptions,
    SavePermissionRule, TablePage, TableRef, TableSchema, TokenStatus,
};
use sheet_port_core::{
    audit, changes, db, google, heartbeat, permissions, sources, vault, CoreError,
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
// Google account linking (docs/ipc.md "Google Sheets account" section)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleConfig {
    pub client_id: Option<String>,
    pub connected_email: Option<String>,
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
        let email = google::connect(&conn, &client_id).map_err(|error| error.to_string())?;
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
