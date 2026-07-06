mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Opens (or creates) the SQLite DB shared with the MCP server and
            // applies schema + seed; see docs/ipc.md for the shared-state model.
            let state = commands::DbState::init().map_err(std::io::Error::other)?;
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
            commands::get_settings,
            commands::set_auto_approve,
            commands::reset_settings,
            commands::get_mcp_config,
            commands::set_mcp_transport,
            commands::set_mcp_port,
            commands::mcp_detect_clients,
            commands::mcp_configure_client,
            commands::mcp_unregister_client,
            commands::mcp_configure_all,
            commands::get_google_config,
            commands::set_google_client_id,
            commands::set_google_client_secret,
            commands::google_connect,
            commands::google_disconnect,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Airtable - Sheet Port");
}
