mod commands;
mod db;
mod models;
mod queries;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Opens (or creates) the SQLite DB shared with the MCP sidecar and
            // applies schema + seed; see docs/ipc.md for the shared-state model.
            let state = db::DbState::init().map_err(std::io::Error::other)?;
            tauri::Manager::manage(app, state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::list_sources,
            commands::list_tables,
            commands::describe_table,
            commands::read_table,
            commands::list_permission_rules,
            commands::save_permission_rule,
            commands::delete_permission_rule,
            commands::list_changes,
            commands::approve_change,
            commands::reject_change,
            commands::list_audit_events,
            commands::token_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Airtable - Sheet Port");
}
