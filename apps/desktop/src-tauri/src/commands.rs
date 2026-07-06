//! Tauri command wrappers. Names, argument names, and JSON field names match
//! docs/ipc.md exactly. All broker logic lives in `sheet-port-core`; each
//! wrapper only locks the shared connection and delegates.

use std::path::PathBuf;
use std::sync::Mutex;

use sheet_port_core::rusqlite::Connection;
use sheet_port_core::types::{
    AppStatus, AuditEvent, DataSource, PendingChange, PermissionRuleRow, SavePermissionRule,
    TablePage, TableRef, TableSchema, TokenStatus,
};
use sheet_port_core::{audit, changes, db, heartbeat, mock_data, permissions, sources, vault};
use tauri::State;

/// Connection plus the resolved path, managed as Tauri state.
pub struct DbState {
    pub conn: Mutex<Connection>,
    pub path: PathBuf,
}

impl DbState {
    pub fn init() -> Result<Self, String> {
        let (conn, path) = db::open_default().map_err(|error| error.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
            path,
        })
    }
}

type Db<'a> = State<'a, DbState>;

fn lock_conn<'a>(state: &'a Db<'_>) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .conn
        .lock()
        .map_err(|_| "Database connection is unavailable (poisoned lock)".to_string())
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
pub fn list_tables(state: Db<'_>, source_id: String) -> Result<Vec<TableRef>, String> {
    let conn = lock_conn(&state)?;
    mock_data::list_tables(&conn, &source_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn describe_table(
    state: Db<'_>,
    source_id: String,
    table_id: String,
) -> Result<TableSchema, String> {
    let conn = lock_conn(&state)?;
    mock_data::describe_table(&conn, &source_id, &table_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn read_table(
    state: Db<'_>,
    source_id: String,
    table_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<TablePage, String> {
    let conn = lock_conn(&state)?;
    mock_data::read_table_page(&conn, &source_id, &table_id, limit, offset)
        .map_err(|error| error.to_string())
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
