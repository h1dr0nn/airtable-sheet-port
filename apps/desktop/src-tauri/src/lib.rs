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
            // One-time migration of a pre-multi-account Google connection into
            // the keyed scheme. Best-effort: a keychain hiccup here must not
            // block startup, so failures are logged and swallowed.
            if let Ok(conn) = state.conn.lock() {
                if let Err(error) = sheet_port_core::google::migrate_legacy_account(&conn) {
                    eprintln!("[sheet-port] Google account migration failed: {error}");
                }
            }
            tauri::Manager::manage(app, state);
            tauri::Manager::manage(app, commands::ManagedSidecar::default());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Kill the desktop-managed sidecar child when the app window closes
            // so no orphan MCP server lingers after exit.
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(sidecar) = tauri::Manager::try_state::<commands::ManagedSidecar>(window)
                {
                    if let Ok(mut guard) = sidecar.child.lock() {
                        if let Some(mut child) = guard.take() {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                }
            }
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
            commands::set_font_scale,
            commands::set_font_family,
            commands::reset_settings,
            commands::get_mcp_config,
            commands::set_mcp_transport,
            commands::set_mcp_port,
            commands::mcp_detect_clients,
            commands::mcp_configure_client,
            commands::mcp_unregister_client,
            commands::mcp_configure_all,
            commands::mcp_server_start,
            commands::mcp_server_stop,
            commands::get_google_config,
            commands::set_google_client_id,
            commands::set_google_client_secret,
            commands::google_connect,
            commands::google_list_accounts,
            commands::google_disconnect,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Airtable - Sheet Port");
}
