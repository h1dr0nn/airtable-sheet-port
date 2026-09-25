//! Pure decision helpers only; nothing here starts or stops a real process.

use super::*;

const LOCAL_APP_DATA: &str = r"C:\Users\ducna1\AppData\Local";

fn heartbeat(pid: i64) -> SidecarHeartbeat {
    SidecarHeartbeat {
        pid,
        version: Some("2.2.1".into()),
        last_seen: "2026-09-25T00:00:00.000Z".into(),
        client_name: None,
        client_version: None,
        exe_path: None,
        parent_exe_path: None,
    }
}

#[test]
fn claude_desktop_paths_are_recognized() {
    for path in [
        r"C:\Users\ducna1\AppData\Local\AnthropicClaude\app-1.0.3218\claude.exe",
        r"C:\Users\ducna1\AppData\Local\AnthropicClaude\claude.exe",
        r"c:\users\DUCNA1\appdata\local\anthropicclaude\app-0.9.0\Claude.exe",
        "C:/Users/ducna1/AppData/Local/AnthropicClaude/app-1.0.0/claude.exe",
    ] {
        assert!(is_claude_desktop_exe(path, LOCAL_APP_DATA), "{path}");
    }
    assert!(is_claude_desktop_exe(
        r"C:\Users\ducna1\AppData\Local\AnthropicClaude\app-1.0.0\claude.exe",
        r"C:\Users\ducna1\AppData\Local\"
    ));
}

#[test]
fn other_claude_executables_are_never_claude_desktop() {
    for path in [
        // Claude Code CLI in a terminal.
        r"C:\Users\ducna1\.local\bin\claude.exe",
        // Claude Desktop's Code tab runs its own Claude Code copy.
        r"C:\Users\ducna1\AppData\Roaming\Claude\claude-code\2.1.0\claude.exe",
        // Other apps inside the AnthropicClaude folder, or look-alike folders.
        r"C:\Users\ducna1\AppData\Local\AnthropicClaude\app-1.0.0\Update.exe",
        r"C:\Users\ducna1\AppData\Local\AnthropicClaudeEvil\claude.exe",
        r"C:\Users\ducna1\AppData\Local\AnthropicClaude\..\Temp\claude.exe",
        r"D:\AppData\Local\AnthropicClaude\claude.exe",
        "",
    ] {
        assert!(!is_claude_desktop_exe(path, LOCAL_APP_DATA), "{path}");
    }
    assert!(!is_claude_desktop_exe(
        r"C:\Users\ducna1\AppData\Local\AnthropicClaude\claude.exe",
        ""
    ));
}

#[test]
fn sidecar_images_include_renamed_update_copies() {
    for path in [
        r"C:\Program Files\Airtable - Sheet Port\sheet-port-mcp.exe",
        r"D:\Projects\airtable-sheet-port\target\debug\sheet-port-mcp.exe",
        r"C:\Program Files\Airtable - Sheet Port\sheet-port-mcp.old-3.exe",
        "/Applications/Sheet Port.app/Contents/MacOS/sheet-port-mcp",
        "SHEET-PORT-MCP.EXE",
    ] {
        assert!(is_sidecar_image(path), "{path}");
    }
    for path in [
        r"C:\Windows\System32\notepad.exe",
        r"C:\x\sheet-port.exe",
        r"C:\x\sheet-port-mcp-evil.exe",
        r"C:\x\sheet-port-mcp.old-.exe",
        r"C:\x\sheet-port-mcp.old-x.exe",
        r"C:\sheet-port-mcp.exe\claude.exe",
    ] {
        assert!(!is_sidecar_image(path), "{path}");
    }
}

#[test]
fn stop_is_allowed_only_for_a_fresh_sidecar_row_and_image() {
    let image = r"D:\Projects\airtable-sheet-port\target\debug\sheet-port-mcp.exe";
    let row = heartbeat(4242);
    assert_eq!(
        validate_stop_target(4242, 1, Some(&row), Some(image)),
        Ok(4242)
    );
}

#[test]
fn stop_refuses_pids_without_a_fresh_heartbeat() {
    let image = "sheet-port-mcp.exe";
    assert_eq!(
        validate_stop_target(4242, 1, None, Some(image)),
        Err(StopRefusal::NotRunning)
    );
    let other = heartbeat(7);
    assert_eq!(
        validate_stop_target(4242, 1, Some(&other), Some(image)),
        Err(StopRefusal::NotRunning),
        "a row for another pid does not count"
    );
    assert_eq!(
        validate_stop_target(-5, 1, None, Some(image)),
        Err(StopRefusal::NotRunning)
    );
}

#[test]
fn stop_refuses_own_process_and_pid_zero() {
    let row = heartbeat(99);
    assert_eq!(
        validate_stop_target(99, 99, Some(&row), Some("sheet-port-mcp.exe")),
        Err(StopRefusal::OwnProcess)
    );
    let zero = heartbeat(0);
    assert_eq!(
        validate_stop_target(0, 99, Some(&zero), Some("sheet-port-mcp.exe")),
        Err(StopRefusal::OwnProcess)
    );
}

#[test]
fn stop_refuses_unknown_or_foreign_images() {
    let row = heartbeat(4242);
    assert_eq!(
        validate_stop_target(4242, 1, Some(&row), None),
        Err(StopRefusal::ImageUnknown)
    );
    let refusal = validate_stop_target(
        4242,
        1,
        Some(&row),
        Some(r"C:\Users\ducna1\AppData\Local\AnthropicClaude\app-1.0.0\claude.exe"),
    )
    .expect_err("claude.exe is not a sidecar");
    assert!(matches!(refusal, StopRefusal::NotSidecar(_)));
    assert!(refusal.to_string().contains("claude.exe"));
}

#[test]
fn file_name_handles_both_separators() {
    assert_eq!(file_name(r"C:\a\b\c.exe"), "c.exe");
    assert_eq!(file_name("/a/b/c"), "c");
    assert_eq!(file_name("plain"), "plain");
}

#[test]
fn reads_this_test_process_image() {
    // Read-only: inspects the test binary itself.
    let path = image_path(std::process::id()).expect("own image path");
    assert!(!path.is_empty());
}
