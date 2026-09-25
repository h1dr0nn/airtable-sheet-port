//! Detects and restarts Claude Desktop so it respawns its MCP sidecar from
//! the current install (docs/ipc.md `claude_desktop_restart`).
//!
//! Windows: Claude Desktop is every `claude.exe` under
//! `%LOCALAPPDATA%\AnthropicClaude\` (see
//! [`sheet_port_core::processes::is_claude_desktop_exe`]). Claude Code's
//! `claude.exe` in `%USERPROFILE%\.local\bin` or any other path is never
//! touched. A restart asks the process tree to close (`taskkill` without
//! `/F`), force-stops the Claude Desktop processes that are still alive after
//! a short wait (re-verifying each image path on the handle it kills
//! through), then relaunches through the Squirrel launcher
//! `%LOCALAPPDATA%\AnthropicClaude\claude.exe`, which starts the newest app
//! version.
//!
//! macOS: `osascript -e 'quit app "Claude"'`, wait, then `open -a Claude`.
//! Other platforms have no Claude Desktop: never running, restart errors.

#[cfg(any(windows, target_os = "macos"))]
use std::time::{Duration, Instant};

/// How long a graceful close gets before the remaining processes are forced.
#[cfg(any(windows, target_os = "macos"))]
const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(5);
/// How long to wait for forced processes to disappear.
#[cfg(any(windows, target_os = "macos"))]
const FORCE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(any(windows, target_os = "macos"))]
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Polls `done` until it returns true or `timeout` passes.
#[cfg(any(windows, target_os = "macos"))]
fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if done() {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(windows)]
mod platform {
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    use sheet_port_core::processes::{self, ProcessInfo};

    use super::{wait_until, FORCE_TIMEOUT, GRACEFUL_TIMEOUT};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    fn local_app_data() -> Option<String> {
        std::env::var("LOCALAPPDATA")
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    /// Running Claude Desktop processes (verified by image path).
    fn desktop_processes(local_app_data: &str) -> Vec<ProcessInfo> {
        processes::processes_named("claude.exe")
            .into_iter()
            .filter(|process| {
                process
                    .exe_path
                    .as_deref()
                    .is_some_and(|path| processes::is_claude_desktop_exe(path, local_app_data))
            })
            .collect()
    }

    pub fn is_running() -> bool {
        local_app_data().is_some_and(|dir| !desktop_processes(&dir).is_empty())
    }

    pub fn restart() -> Result<(), String> {
        let dir = local_app_data().ok_or("LOCALAPPDATA is not set")?;
        let running = desktop_processes(&dir);
        if running.is_empty() {
            return Err("Claude Desktop is not running".to_string());
        }
        let fallback_exe = running.iter().find_map(|process| process.exe_path.clone());

        // Graceful: ask each tree root (a Claude Desktop process whose parent
        // is not one) to close, like clicking the window's close button.
        let pids: Vec<u32> = running.iter().map(|process| process.pid).collect();
        for root in running
            .iter()
            .filter(|process| !pids.contains(&process.parent_pid))
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &root.pid.to_string(), "/T"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }

        if !wait_until(GRACEFUL_TIMEOUT, || desktop_processes(&dir).is_empty()) {
            // Force only what is still a Claude Desktop process, re-checking
            // the image on the handle used for the kill.
            for process in desktop_processes(&dir) {
                let _ = processes::terminate_verified(process.pid, |image| {
                    processes::is_claude_desktop_exe(image, &dir)
                });
            }
            if !wait_until(FORCE_TIMEOUT, || desktop_processes(&dir).is_empty()) {
                return Err("Claude Desktop did not quit".to_string());
            }
        }

        let launcher = PathBuf::from(&dir)
            .join("AnthropicClaude")
            .join("claude.exe");
        let exe = if launcher.exists() {
            launcher
        } else {
            fallback_exe
                .map(PathBuf::from)
                .ok_or("Could not find the Claude Desktop executable to relaunch")?
        };
        Command::new(&exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not relaunch Claude Desktop: {error}"))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::process::{Command, Stdio};

    use super::{wait_until, FORCE_TIMEOUT, GRACEFUL_TIMEOUT};

    const APP_NAME: &str = "Claude";

    pub fn is_running() -> bool {
        Command::new("pgrep")
            .args(["-x", APP_NAME])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    pub fn restart() -> Result<(), String> {
        if !is_running() {
            return Err("Claude Desktop is not running".to_string());
        }
        Command::new("osascript")
            .args(["-e", &format!("quit app \"{APP_NAME}\"")])
            .status()
            .map_err(|error| format!("Could not quit Claude Desktop: {error}"))?;
        if !wait_until(GRACEFUL_TIMEOUT + FORCE_TIMEOUT, || !is_running()) {
            return Err("Claude Desktop did not quit".to_string());
        }
        let status = Command::new("open")
            .args(["-a", APP_NAME])
            .status()
            .map_err(|error| format!("Could not relaunch Claude Desktop: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("Could not relaunch Claude Desktop".to_string())
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    pub fn is_running() -> bool {
        false
    }

    pub fn restart() -> Result<(), String> {
        Err("Claude Desktop is not available on this platform".to_string())
    }
}

/// Whether Claude Desktop is running right now (cheap enough to poll).
pub fn is_running() -> bool {
    platform::is_running()
}

/// Quits and relaunches Claude Desktop. Blocking; call off the main thread.
pub fn restart() -> Result<(), String> {
    platform::restart()
}
