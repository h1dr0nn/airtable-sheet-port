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
