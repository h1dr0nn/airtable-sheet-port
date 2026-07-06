//! Tauri command wrappers. Names, argument names, and JSON field names match
//! docs/ipc.md exactly. All heavy lifting lives in `queries`; each wrapper
//! only locks the shared connection and delegates.

use tauri::State;

use crate::db::DbState;
use crate::models::{
    AppStatus, AuditEvent, DataSource, PendingChange, PermissionRuleRow, SavePermissionRule,
    TablePage, TableRef, TableSchema, TokenStatus,
};
use crate::queries::{self, ChangeDecision};

const KEYRING_SERVICE: &str = "sheet-port";
const KEYRING_USER_GOOGLE_SHEETS: &str = "google_sheets";
const KEYRING_USER_PROVIDER: &str = "provider";

type Db<'a> = State<'a, DbState>;

fn lock_conn<'a>(
    state: &'a Db<'_>,
) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, String> {
    state
        .conn
        .lock()
        .map_err(|_| "Database connection is unavailable (poisoned lock)".to_string())
}

#[tauri::command]
pub fn get_app_status(app: tauri::AppHandle, state: Db<'_>) -> Result<AppStatus, String> {
    let conn = lock_conn(&state)?;
    queries::app_status(
        &conn,
        app.package_info().version.to_string(),
        state.path.display().to_string(),
    )
}

#[tauri::command]
pub fn list_sources(state: Db<'_>) -> Result<Vec<DataSource>, String> {
    let conn = lock_conn(&state)?;
    queries::list_sources(&conn)
}

#[tauri::command]
pub fn list_tables(state: Db<'_>, source_id: String) -> Result<Vec<TableRef>, String> {
    let conn = lock_conn(&state)?;
    queries::list_tables(&conn, &source_id)
}

#[tauri::command]
pub fn describe_table(
    state: Db<'_>,
    source_id: String,
    table_id: String,
) -> Result<TableSchema, String> {
    let conn = lock_conn(&state)?;
    queries::describe_table(&conn, &source_id, &table_id)
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
    queries::read_table(&conn, &source_id, &table_id, limit, offset)
}

#[tauri::command]
pub fn list_permission_rules(state: Db<'_>) -> Result<Vec<PermissionRuleRow>, String> {
    let conn = lock_conn(&state)?;
    queries::list_permission_rules(&conn)
}

#[tauri::command]
pub fn save_permission_rule(
    state: Db<'_>,
    rule: SavePermissionRule,
) -> Result<PermissionRuleRow, String> {
    let conn = lock_conn(&state)?;
    queries::save_permission_rule(&conn, &rule)
}

#[tauri::command]
pub fn delete_permission_rule(state: Db<'_>, id: i64) -> Result<(), String> {
    let conn = lock_conn(&state)?;
    queries::delete_permission_rule(&conn, id)
}

#[tauri::command]
pub fn list_changes(state: Db<'_>, status: Option<String>) -> Result<Vec<PendingChange>, String> {
    let conn = lock_conn(&state)?;
    queries::list_changes(&conn, status.as_deref())
}

#[tauri::command]
pub fn approve_change(state: Db<'_>, change_id: String) -> Result<PendingChange, String> {
    let conn = lock_conn(&state)?;
    queries::decide_change(&conn, &change_id, ChangeDecision::Approve)
}

#[tauri::command]
pub fn reject_change(state: Db<'_>, change_id: String) -> Result<PendingChange, String> {
    let conn = lock_conn(&state)?;
    queries::decide_change(&conn, &change_id, ChangeDecision::Reject)
}

#[tauri::command]
pub fn list_audit_events(
    state: Db<'_>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AuditEvent>, String> {
    let conn = lock_conn(&state)?;
    queries::list_audit_events(&conn, limit, offset)
}

#[tauri::command]
pub fn token_status() -> Result<TokenStatus, String> {
    Ok(TokenStatus {
        google_sheets: keyring_entry_exists(KEYRING_USER_GOOGLE_SHEETS),
        provider: keyring_entry_exists(KEYRING_USER_PROVIDER),
    })
}

/// True only when a credential exists. Secrets themselves NEVER cross IPC;
/// unexpected keychain errors are logged to stderr and reported as "absent".
fn keyring_entry_exists(user: &str) -> bool {
    let entry = match keyring::Entry::new(KEYRING_SERVICE, user) {
        Ok(entry) => entry,
        Err(error) => {
            eprintln!("[sheet-port] keyring entry '{user}' unavailable: {error}");
            return false;
        }
    };
    match entry.get_password() {
        Ok(_) => true,
        Err(keyring::Error::NoEntry) => false,
        Err(error) => {
            eprintln!("[sheet-port] keyring read for '{user}' failed: {error}");
            false
        }
    }
}
