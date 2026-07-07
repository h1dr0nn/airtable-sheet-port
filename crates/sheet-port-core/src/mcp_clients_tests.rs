//! Unit tests for the MCP client config merge/preserve + unregister round-trip.
//! Exercise the pure filesystem helpers with temp files so no env var or real
//! client config is touched.

use super::*;
use serde_json::{json, Value};

fn temp_config_path() -> std::path::PathBuf {
    std::env::temp_dir()
        .join("sheet-port-mcp-clients-tests")
        .join(format!("{}.json", uuid::Uuid::new_v4()))
}

fn temp_toml_path() -> std::path::PathBuf {
    std::env::temp_dir()
        .join("sheet-port-mcp-clients-tests")
        .join(format!("{}.toml", uuid::Uuid::new_v4()))
}

fn read_toml_string(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).expect("config file should exist")
}

fn read_json(path: &std::path::Path) -> Value {
    let text = std::fs::read_to_string(path).expect("config file should exist");
    serde_json::from_str(&text).expect("config should be valid JSON")
}

fn stdio_spec() -> ServerSpec {
    ServerSpec::Stdio {
        command: "/opt/sheet-port-mcp".to_string(),
        args: vec!["--stdio".to_string()],
    }
}

#[test]
fn merge_creates_file_and_parent_dirs_with_stdio_entry() {
    let path = temp_config_path();
    assert!(!path.exists());

    merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec()).expect("merge");

    let root = read_json(&path);
    let entry = &root["mcpServers"][MCP_CLIENT_SERVER_NAME];
    assert_eq!(entry["command"], json!("/opt/sheet-port-mcp"));
    assert_eq!(entry["args"], json!(["--stdio"]));
}

#[test]
fn merge_preserves_other_servers_and_top_level_keys() {
    let path = temp_config_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        json!({
            "mcpServers": {
                "other-server": { "command": "other", "args": ["--x"] }
            },
            "someOtherSetting": true
        })
        .to_string(),
    )
    .unwrap();

    merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec()).expect("merge");

    let root = read_json(&path);
    // Our entry landed...
    assert!(root["mcpServers"][MCP_CLIENT_SERVER_NAME].is_object());
    // ...and the pre-existing server + unrelated key are untouched.
    assert_eq!(
        root["mcpServers"]["other-server"]["command"],
        json!("other")
    );
    assert_eq!(root["someOtherSetting"], json!(true));
}

#[test]
fn merge_is_idempotent_and_overwrites_only_our_entry() {
    let path = temp_config_path();
    merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec()).expect("first");
    // Re-run with a different spec: only our entry changes, no duplication.
    merge_entry_into(
        &path,
        ConfigShape::McpServers,
        &ServerSpec::http_for_port(4319),
    )
    .expect("second");

    let root = read_json(&path);
    let servers = root["mcpServers"].as_object().unwrap();
    assert_eq!(servers.len(), 1, "no duplicate entries");
    let entry = &servers[MCP_CLIENT_SERVER_NAME];
    assert_eq!(entry["type"], json!("sse"));
    assert_eq!(entry["url"], json!("http://127.0.0.1:4319/mcp"));
}

#[test]
fn configure_then_unregister_round_trips_back_to_original() {
    let path = temp_config_path();
    let original = json!({
        "mcpServers": { "keep-me": { "command": "keep" } }
    });
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, original.to_string()).unwrap();

    merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec()).expect("configure");
    assert!(read_json(&path)["mcpServers"][MCP_CLIENT_SERVER_NAME].is_object());

    let removed = remove_entry_from(&path, ConfigShape::McpServers).expect("unregister");
    assert!(removed, "our entry was present and removed");

    let root = read_json(&path);
    assert!(
        root["mcpServers"][MCP_CLIENT_SERVER_NAME].is_null(),
        "our entry is gone"
    );
    assert_eq!(
        root["mcpServers"]["keep-me"]["command"],
        json!("keep"),
        "other server preserved through the round trip"
    );
}

#[test]
fn remove_is_noop_when_file_missing_or_entry_absent() {
    let missing = temp_config_path();
    assert!(!remove_entry_from(&missing, ConfigShape::McpServers).expect("missing file"));

    let path = temp_config_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, json!({ "mcpServers": { "x": {} } }).to_string()).unwrap();
    assert!(
        !remove_entry_from(&path, ConfigShape::McpServers).expect("entry absent"),
        "removing a not-present entry is a no-op"
    );
    // The untouched foreign entry is still there.
    assert!(read_json(&path)["mcpServers"]["x"].is_object());
}

#[test]
fn malformed_config_is_rejected_not_clobbered() {
    let path = temp_config_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not valid json").unwrap();

    let result = merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec());
    assert!(result.is_err(), "malformed JSON must error");
    // File left as-is, not overwritten.
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{ not valid json",
        "malformed file preserved"
    );
}

#[test]
fn http_spec_builds_loopback_url_with_shared_path() {
    match ServerSpec::http_for_port(5000) {
        ServerSpec::Http { url } => assert_eq!(url, "http://127.0.0.1:5000/mcp"),
        other => panic!("expected http spec, got {other:?}"),
    }
}

#[test]
fn registry_ids_are_unique_and_detect_covers_them_all() {
    let clients = detect_clients();
    let ids: Vec<&str> = clients.iter().map(|c| c.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "client ids must be unique");
    // The known confident clients are present.
    for expected in [
        "claude-desktop",
        "claude-code",
        "cursor",
        "windsurf",
        "cline",
    ] {
        assert!(ids.contains(&expected), "missing client {expected}");
    }
}

#[test]
fn configure_unknown_client_is_not_found() {
    let error = configure_client("does-not-exist", &stdio_spec()).unwrap_err();
    assert!(matches!(error, CoreError::NotFound(_)));
}

#[test]
fn configure_undetectable_client_is_unsupported() {
    // vscode-copilot is registered but detectable = false.
    let error = configure_client("vscode-copilot", &stdio_spec()).unwrap_err();
    assert!(matches!(error, CoreError::Unsupported(_)));
}

#[test]
fn registry_covers_antigravity_and_codex() {
    let clients = detect_clients();
    let ids: Vec<&str> = clients.iter().map(|c| c.id.as_str()).collect();
    for expected in ["antigravity-2", "antigravity-ide", "codex"] {
        assert!(ids.contains(&expected), "missing client {expected}");
    }
    // Detectable clients resolve their paths on every OS via home().
    for client in &clients {
        if client.id.starts_with("antigravity") || client.id == "codex" {
            assert!(client.detectable, "{} should be detectable", client.id);
        }
    }
}

// --- Antigravity JSON (mcpServers shape) -----------------------------------

#[test]
fn antigravity_merge_creates_mcp_servers_entry() {
    let path = temp_config_path();
    // Antigravity shares Windsurf's mcpServers shape.
    merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec()).expect("merge");

    let root = read_json(&path);
    let entry = &root["mcpServers"][MCP_CLIENT_SERVER_NAME];
    assert_eq!(entry["command"], json!("/opt/sheet-port-mcp"));
    assert_eq!(entry["args"], json!(["--stdio"]));
}

#[test]
fn antigravity_merge_preserves_other_servers_then_unregister_leaves_them() {
    let path = temp_config_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        json!({
            "mcpServers": {
                "other-server": { "command": "other", "args": ["--x"] }
            }
        })
        .to_string(),
    )
    .unwrap();

    merge_entry_into(&path, ConfigShape::McpServers, &stdio_spec()).expect("merge");
    assert!(read_json(&path)["mcpServers"][MCP_CLIENT_SERVER_NAME].is_object());

    let removed = remove_entry_from(&path, ConfigShape::McpServers).expect("unregister");
    assert!(removed, "our entry was present and removed");

    let root = read_json(&path);
    assert!(
        root["mcpServers"][MCP_CLIENT_SERVER_NAME].is_null(),
        "our entry is gone"
    );
    assert_eq!(
        root["mcpServers"]["other-server"]["command"],
        json!("other"),
        "pre-existing server preserved"
    );
}

// --- Codex TOML (mcp_servers tables) ---------------------------------------

#[test]
fn toml_merge_creates_stdio_table() {
    let path = temp_toml_path();
    assert!(!path.exists());

    merge_entry_into(&path, ConfigShape::Toml, &stdio_spec()).expect("merge");

    let text = read_toml_string(&path);
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .expect("valid TOML output");
    let entry = &doc["mcp_servers"][MCP_CLIENT_SERVER_NAME];
    assert_eq!(
        entry["command"].as_str(),
        Some("/opt/sheet-port-mcp"),
        "command written"
    );
    let args = entry["args"].as_array().expect("args array");
    assert_eq!(args.len(), 1);
    assert_eq!(args.get(0).and_then(|v| v.as_str()), Some("--stdio"));
    // Renders as a sub-table header, not an inline parent table.
    assert!(
        text.contains("[mcp_servers.airtable-sheet-port]"),
        "sub-table header present, got:\n{text}"
    );
}

#[test]
fn toml_merge_writes_url_for_http_spec() {
    let path = temp_toml_path();
    merge_entry_into(&path, ConfigShape::Toml, &ServerSpec::http_for_port(4319)).expect("merge");

    let text = read_toml_string(&path);
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .expect("valid TOML output");
    let entry = &doc["mcp_servers"][MCP_CLIENT_SERVER_NAME];
    assert_eq!(
        entry["url"].as_str(),
        Some("http://127.0.0.1:4319/mcp"),
        "http url written"
    );
    assert!(entry.get("command").is_none(), "no stdio command for http");
}

#[test]
fn toml_merge_preserves_other_table_top_level_keys_and_comments() {
    let path = temp_toml_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = "\
# top of file comment
model = \"o3\"
approval_policy = \"on-request\"

# another server the user configured by hand
[mcp_servers.other]
command = \"other-cmd\"
args = [\"--flag\"]
";
    std::fs::write(&path, original).unwrap();

    merge_entry_into(&path, ConfigShape::Toml, &stdio_spec()).expect("merge");

    let text = read_toml_string(&path);
    // Top-level keys and both comments survive.
    assert!(text.contains("# top of file comment"), "top comment kept");
    assert!(text.contains("model = \"o3\""), "top-level key kept");
    assert!(
        text.contains("approval_policy = \"on-request\""),
        "second top-level key kept"
    );
    assert!(
        text.contains("# another server the user configured by hand"),
        "inline comment kept"
    );
    // The unrelated server table is intact.
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .expect("valid TOML output");
    assert_eq!(
        doc["mcp_servers"]["other"]["command"].as_str(),
        Some("other-cmd"),
        "unrelated server table preserved"
    );
    // And ours landed alongside it.
    assert_eq!(
        doc["mcp_servers"][MCP_CLIENT_SERVER_NAME]["command"].as_str(),
        Some("/opt/sheet-port-mcp"),
        "our table added"
    );
}

#[test]
fn toml_unregister_removes_only_our_table() {
    let path = temp_toml_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "\
model = \"o3\"

[mcp_servers.other]
command = \"other-cmd\"
",
    )
    .unwrap();

    merge_entry_into(&path, ConfigShape::Toml, &stdio_spec()).expect("configure");
    assert!(toml_is_configured(&path), "ours present after configure");

    let removed = remove_entry_from(&path, ConfigShape::Toml).expect("unregister");
    assert!(removed, "our table was present and removed");

    let text = read_toml_string(&path);
    let doc = text
        .parse::<toml_edit::DocumentMut>()
        .expect("valid TOML output");
    assert!(
        doc["mcp_servers"].get(MCP_CLIENT_SERVER_NAME).is_none(),
        "our table gone"
    );
    assert_eq!(
        doc["mcp_servers"]["other"]["command"].as_str(),
        Some("other-cmd"),
        "unrelated table survives"
    );
    assert!(text.contains("model = \"o3\""), "top-level key survives");
}

#[test]
fn toml_remove_is_noop_when_file_missing_or_entry_absent() {
    let missing = temp_toml_path();
    assert!(!remove_entry_from(&missing, ConfigShape::Toml).expect("missing file"));

    let path = temp_toml_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "[mcp_servers.other]\ncommand = \"x\"\n").unwrap();
    assert!(
        !remove_entry_from(&path, ConfigShape::Toml).expect("entry absent"),
        "removing a not-present table is a no-op"
    );
    // The foreign table is untouched.
    assert!(read_toml_string(&path).contains("[mcp_servers.other]"));
}

#[test]
fn toml_is_configured_reflects_presence() {
    let path = temp_toml_path();
    // Missing file -> not configured.
    assert!(!toml_is_configured(&path));

    merge_entry_into(&path, ConfigShape::Toml, &stdio_spec()).expect("configure");
    assert!(toml_is_configured(&path), "true once our table exists");

    remove_entry_from(&path, ConfigShape::Toml).expect("unregister");
    assert!(!toml_is_configured(&path), "false once our table removed");
}

#[test]
fn toml_malformed_config_is_rejected_not_clobbered() {
    let path = temp_toml_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bad = "this is = = not valid toml [[[";
    std::fs::write(&path, bad).unwrap();

    let result = merge_entry_into(&path, ConfigShape::Toml, &stdio_spec());
    assert!(result.is_err(), "malformed TOML must error");
    assert_eq!(
        read_toml_string(&path),
        bad,
        "malformed file left untouched"
    );
}
