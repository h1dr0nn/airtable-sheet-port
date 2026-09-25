mod claude_desktop;
mod commands;

use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single-instance MUST be the first plugin so a second launch is
        // intercepted before any other setup runs; it focuses the existing
        // window instead of spinning up a duplicate process.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            commands::show_main_window(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Opens external links (the Guide's script.google.com link) in the
        // system browser; capabilities scope it to the allowed URLs.
        .plugin(tauri_plugin_opener::init())
        // Persist and restore window position/size/maximized across restarts.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Launch-at-login toggle exposed in Settings (LaunchAgent on macOS).
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            remove_renamed_sidecars();
            // Opens (or creates) the SQLite DB shared with the MCP server and
            // applies schema + seed; see docs/ipc.md for the shared-state model.
            let state = commands::DbState::init().map_err(std::io::Error::other)?;
            app.manage(state);
            app.manage(commands::ManagedSidecar::default());
            // Auto-start the managed sidecar so the server is running on launch
            // without the user clicking Start. Best-effort; never blocks startup.
            commands::auto_start_managed_sidecar(app.handle());
            // Re-register the remembered MCP clients so their configs always
            // point at the current sidecar (heals stale paths after updates and
            // entries dropped by clients that rewrite their config on exit).
            commands::reregister_mcp_clients(app.handle());
            Ok(())
        })
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::list_sources,
            commands::list_tables,
            commands::describe_table,
            commands::read_table,
            commands::list_permission_rules,
            commands::save_permission_rule,
            commands::delete_permission_rule,
            commands::list_audit_events,
            commands::clear_audit_log,
            commands::token_status,
            commands::get_settings,
            commands::set_font_scale,
            commands::set_font_family,
            commands::set_language,
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
            commands::mcp_stop_sidecar,
            commands::claude_desktop_restart,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
            commands::google_list_accounts,
            commands::google_add_bridge,
            commands::google_remove_bridge,
            commands::google_test_bridge,
            commands::workbench_tree,
            commands::create_workbench_folder,
            commands::rename_workbench_folder,
            commands::delete_workbench_folder,
            commands::add_workbench_spreadsheet,
            commands::remove_workbench_item,
            commands::move_workbench_item,
            commands::list_workbench_sheet_tabs,
            commands::read_workbench_sheet,
            commands::update_workbench_cell,
            commands::append_workbench_row,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Airtable - Sheet Port");
}

/// Closing the window quits the app: there is no tray or background mode. MCP
/// clients spawn their own stdio sidecar, so they keep working after the app
/// exits. The desktop-managed HTTP sidecar is a child of this process and is
/// killed on Destroyed so no orphan server lingers.
fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    if matches!(event, WindowEvent::Destroyed) {
        kill_managed_sidecar(window.app_handle());
    }
}

/// Best-effort terminate + reap of the desktop-managed sidecar child when the
/// main window is destroyed (which is how the app quits).
fn kill_managed_sidecar(app: &tauri::AppHandle) {
    if let Some(sidecar) = app.try_state::<commands::ManagedSidecar>() {
        if let Ok(mut guard) = sidecar.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Deletes `sheet-port-mcp.old-*.exe` next to the app. The NSIS installer
/// renames a sidecar that an MCP client still runs to that name so the new one
/// can be written (windows/hooks.nsh). Best-effort: a copy still in use fails
/// to delete and is retried on the next launch.
#[cfg(windows)]
fn remove_renamed_sidecars() {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
    else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("sheet-port-mcp.old-") && name.ends_with(".exe") {
            if let Err(error) = std::fs::remove_file(entry.path()) {
                eprintln!("[sheet-port] could not remove {name}: {error}");
            }
        }
    }
}

#[cfg(not(windows))]
fn remove_renamed_sidecars() {}
