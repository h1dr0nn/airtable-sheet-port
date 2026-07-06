#[tauri::command]
fn mcp_status() -> &'static str {
    // TODO: manage the Node MCP sidecar lifecycle from Tauri.
    "stdio-ready"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![mcp_status])
        .run(tauri::generate_context!())
        .expect("error while running Sheet Port");
}
