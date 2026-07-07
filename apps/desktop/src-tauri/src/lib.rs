mod commands;

use sheet_port_core::constants::CLOSE_BEHAVIOR_QUIT;
use sheet_port_core::constants::CLOSE_BEHAVIOR_TRAY;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Emitted to the frontend when the user closes the window and the configured
/// close behavior is "ask": the frontend shows the choice modal, then calls
/// `window_hide_to_tray` or `window_quit`.
const EVENT_CLOSE_REQUESTED: &str = "close-requested";

/// Tray menu item ids.
const TRAY_SHOW_ID: &str = "show-window";
const TRAY_QUIT_ID: &str = "quit";

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
        // Persist and restore window position/size/maximized across restarts.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Launch-at-login toggle exposed in Settings (LaunchAgent on macOS).
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
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
            app.manage(state);
            app.manage(commands::ManagedSidecar::default());
            build_tray(app.handle())?;
            // Auto-start the managed sidecar so the server is running on launch
            // without the user clicking Start. Best-effort; never blocks startup.
            commands::auto_start_managed_sidecar(app.handle());
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
            commands::list_changes,
            commands::approve_change,
            commands::reject_change,
            commands::list_audit_events,
            commands::clear_audit_log,
            commands::token_status,
            commands::get_settings,
            commands::set_auto_approve,
            commands::set_font_scale,
            commands::set_font_family,
            commands::set_language,
            commands::set_close_behavior,
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
            commands::window_hide_to_tray,
            commands::window_quit,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
            commands::get_google_config,
            commands::set_google_client_id,
            commands::set_google_client_secret,
            commands::google_connect,
            commands::google_list_accounts,
            commands::google_disconnect,
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

/// Builds the system tray icon (app icon) with a "Show Window" + "Quit" menu.
/// Left-clicking the tray or choosing "Show Window" restores the window; "Quit"
/// exits the app.
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id(TRAY_SHOW_ID, "Show Window").build(app)?;
    let quit = MenuItemBuilder::with_id(TRAY_QUIT_ID, "Quit").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW_ID => commands::show_main_window(app),
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                commands::show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Handles window events: the managed sidecar is killed on Destroyed so no
/// orphan MCP server lingers, and CloseRequested is routed by the configured
/// close behavior (quit / minimize to tray / ask the user).
fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    match event {
        WindowEvent::Destroyed => kill_managed_sidecar(window.app_handle()),
        WindowEvent::CloseRequested { api, .. } => {
            let behavior = resolve_close_behavior(window.app_handle());
            match behavior.as_str() {
                // Real close: let the window (and app) shut down.
                CLOSE_BEHAVIOR_QUIT => {}
                // Stay resident in the tray: keep the process, hide the window.
                CLOSE_BEHAVIOR_TRAY => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Ask: keep the window alive and let the frontend decide.
                _ => {
                    api.prevent_close();
                    let _ = window.emit(EVENT_CLOSE_REQUESTED, ());
                }
            }
        }
        _ => {}
    }
}

/// Reads the persisted close behavior, defaulting to "ask" if the DB is
/// unavailable so a locked connection never forces an unexpected quit.
fn resolve_close_behavior(app: &tauri::AppHandle) -> String {
    use sheet_port_core::constants::CLOSE_BEHAVIOR_DEFAULT;
    let Some(state) = app.try_state::<commands::DbState>() else {
        return CLOSE_BEHAVIOR_DEFAULT.to_string();
    };
    let Ok(conn) = state.conn.lock() else {
        return CLOSE_BEHAVIOR_DEFAULT.to_string();
    };
    sheet_port_core::db::get_close_behavior(&conn)
        .unwrap_or_else(|_| CLOSE_BEHAVIOR_DEFAULT.to_string())
}

/// Best-effort terminate + reap of the desktop-managed sidecar child, shared by
/// the window Destroyed path and the tray/frontend Quit path (via Destroyed).
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
