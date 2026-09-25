//! Small OS process helpers shared by the sidecar and the desktop shell: the
//! image path of a PID, the sidecar's parent process, a name-filtered process
//! list, and a terminate that re-checks the image on the same handle it kills
//! through (so a recycled PID is never hit). Windows uses the Win32 toolhelp
//! and process APIs from `windows-sys`; other platforms use `/proc` or `ps`.
//!
//! The decisions (is this path a sidecar? a Claude Desktop process? may the
//! desktop stop this PID?) are pure functions so they are unit-tested without
//! touching real processes.

use std::fmt;

use crate::types::SidecarHeartbeat;

/// The sidecar executable's file stem on every platform.
pub const SIDECAR_EXE_STEM: &str = "sheet-port-mcp";

/// One process from [`processes_named`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: u32,
    /// The image file name as the OS reports it (e.g. "claude.exe").
    pub name: String,
    /// Full image path; `None` when the process cannot be opened (another
    /// user, elevated, or already gone).
    pub exe_path: Option<String>,
}

/// The last component of a Windows or POSIX path.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// True when `path` is a sheet-port-mcp executable: `sheet-port-mcp(.exe)`,
/// or `sheet-port-mcp.old-N.exe`, the name the Windows installer gives a copy
/// that was still running during an update. Case-insensitive.
pub fn is_sidecar_image(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    if stem == SIDECAR_EXE_STEM {
        return true;
    }
    match stem.strip_prefix(SIDECAR_EXE_STEM) {
        Some(rest) => rest
            .strip_prefix(".old-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        None => false,
    }
}

/// Lowercases and turns every `/` into `\` so Windows paths compare reliably.
fn normalize_windows_path(path: &str) -> String {
    path.replace('/', "\\").to_ascii_lowercase()
}

/// True when `exe_path` is a Claude Desktop process on Windows: a `claude.exe`
/// anywhere under `%LOCALAPPDATA%\AnthropicClaude\` (the Squirrel launcher
/// and the versioned `app-x.y.z\claude.exe`). Claude Code's `claude.exe` in
/// `%USERPROFILE%\.local\bin`, the Desktop Code tab's copy under
/// `%APPDATA%\Claude\claude-code\`, and every other path are rejected.
pub fn is_claude_desktop_exe(exe_path: &str, local_app_data: &str) -> bool {
    if local_app_data.trim().is_empty() {
        return false;
    }
    let path = normalize_windows_path(exe_path);
    let mut root = normalize_windows_path(local_app_data.trim());
    while root.ends_with('\\') {
        root.pop();
    }
    root.push_str("\\anthropicclaude\\");
    file_name(&path) == "claude.exe" && path.starts_with(&root) && !path.contains("\\..\\")
}

/// Why the desktop refuses to stop a PID (see [`validate_stop_target`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopRefusal {
    /// No fresh heartbeat row for the PID: not a running sidecar we know of.
    NotRunning,
    /// The PID is the calling process itself (or 0).
    OwnProcess,
    /// The process image could not be read (gone, or not ours to open).
    ImageUnknown,
    /// The process is not a sheet-port-mcp executable (e.g. a recycled PID).
    NotSidecar(String),
}

impl fmt::Display for StopRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning => write!(f, "No running MCP sidecar has this PID"),
            Self::OwnProcess => write!(f, "Refusing to stop this process"),
            Self::ImageUnknown => write!(f, "Could not verify the process behind this PID"),
            Self::NotSidecar(image) => write!(
                f,
                "Process is not an MCP sidecar ({}); refusing to stop it",
                file_name(image)
            ),
        }
    }
}

/// Decides whether the desktop may kill `pid`: it must have a fresh
/// heartbeat row, not be the caller, and run a sheet-port-mcp image. Never
/// allows an arbitrary PID.
pub fn validate_stop_target(
    pid: i64,
    own_pid: u32,
    fresh_heartbeat: Option<&SidecarHeartbeat>,
    image_path: Option<&str>,
) -> Result<u32, StopRefusal> {
    let pid = u32::try_from(pid).map_err(|_| StopRefusal::NotRunning)?;
    if pid == 0 || pid == own_pid {
        return Err(StopRefusal::OwnProcess);
    }
    match fresh_heartbeat {
        Some(row) if row.pid == i64::from(pid) => {}
        _ => return Err(StopRefusal::NotRunning),
    }
    let image = image_path.ok_or(StopRefusal::ImageUnknown)?;
    if !is_sidecar_image(image) {
        return Err(StopRefusal::NotSidecar(image.to_string()));
    }
    Ok(pid)
}

/// Full image path of `pid`, or `None` when it cannot be read.
pub fn image_path(pid: u32) -> Option<String> {
    platform::image_path(pid)
}

/// Image path of the process that spawned this one (the MCP client for a
/// stdio sidecar). `None` when unknown.
pub fn parent_exe_path() -> Option<String> {
    platform::parent_pid(std::process::id()).and_then(image_path)
}

/// Every process whose image file name equals `name` (case-insensitive),
/// with its full path when readable. Empty on platforms without an
/// enumeration backend (only Windows has one; the callers are Windows-only).
pub fn processes_named(name: &str) -> Vec<ProcessInfo> {
    platform::processes_named(name)
}

/// Terminates `pid` only if `accept` approves its image path, read from the
/// same handle used for the kill so a recycled PID is never terminated.
pub fn terminate_verified(pid: u32, accept: impl Fn(&str) -> bool) -> Result<(), String> {
    platform::terminate_verified(pid, &accept)
}

#[cfg(windows)]
mod platform {
    use super::ProcessInfo;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE,
    };

    /// Closes the wrapped handle on drop.
    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful Open*/Create* call and
            // is closed exactly once here.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn open(pid: u32, access: u32) -> Option<OwnedHandle> {
        // SAFETY: plain FFI call; a null return means failure.
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            None
        } else {
            Some(OwnedHandle(handle))
        }
    }

    fn image_of(handle: &OwnedHandle) -> Option<String> {
        let mut buffer = vec![0u16; 32_768];
        let mut size = buffer.len() as u32;
        // SAFETY: `buffer` holds `size` u16s and outlives the call.
        let ok = unsafe {
            QueryFullProcessImageNameW(handle.0, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size)
        };
        if ok == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }

    pub fn image_path(pid: u32) -> Option<String> {
        open(pid, PROCESS_QUERY_LIMITED_INFORMATION).and_then(|handle| image_of(&handle))
    }

    /// Walks a toolhelp snapshot, calling `visit` with (pid, parent, name).
    fn for_each_process(mut visit: impl FnMut(u32, u32, String)) {
        // SAFETY: plain FFI call; INVALID_HANDLE_VALUE means failure.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return;
        }
        let snapshot = OwnedHandle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        // SAFETY: `entry.dwSize` is set as the API requires.
        let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
        while more {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            visit(entry.th32ProcessID, entry.th32ParentProcessID, name);
            // SAFETY: same snapshot and entry as above.
            more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
        }
    }

    pub fn parent_pid(pid: u32) -> Option<u32> {
        let mut parent = None;
        for_each_process(|candidate, parent_pid, _| {
            if candidate == pid {
                parent = Some(parent_pid);
            }
        });
        parent.filter(|&parent| parent != 0)
    }

    pub fn processes_named(name: &str) -> Vec<ProcessInfo> {
        let mut found = Vec::new();
        for_each_process(|pid, parent_pid, exe_name| {
            if exe_name.eq_ignore_ascii_case(name) {
                found.push((pid, parent_pid, exe_name));
            }
        });
        found
            .into_iter()
            .map(|(pid, parent_pid, name)| ProcessInfo {
                pid,
                parent_pid,
                name,
                exe_path: image_path(pid),
            })
            .collect()
    }

    pub fn terminate_verified(pid: u32, accept: &dyn Fn(&str) -> bool) -> Result<(), String> {
        let handle = open(
            pid,
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
        )
        .ok_or_else(|| format!("Could not open process {pid}"))?;
        let image = image_of(&handle).ok_or_else(|| format!("Could not read process {pid}"))?;
        if !accept(&image) {
            return Err(format!("Process {pid} changed; refusing to stop it"));
        }
        // SAFETY: valid handle opened with PROCESS_TERMINATE.
        if unsafe { TerminateProcess(handle.0, 1) } == 0 {
            return Err(format!(
                "Could not stop process {pid}: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: valid handle opened with SYNCHRONIZE. Best effort wait so
        // the caller sees the process gone before it cleans up.
        unsafe {
            WaitForSingleObject(handle.0, 3000);
        }
        Ok(())
    }
}

#[cfg(unix)]
mod platform {
    use super::ProcessInfo;
    use std::process::Command;

    pub fn image_path(pid: u32) -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(path) = std::fs::read_link(format!("/proc/{pid}/exe")) {
                return Some(path.to_string_lossy().into_owned());
            }
        }
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .ok()?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (output.status.success() && !path.is_empty()).then_some(path)
    }

    pub fn parent_pid(pid: u32) -> Option<u32> {
        if pid == std::process::id() {
            return Some(std::os::unix::process::parent_id()).filter(|&ppid| ppid != 0);
        }
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "ppid="])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    pub fn processes_named(_name: &str) -> Vec<ProcessInfo> {
        Vec::new()
    }

    pub fn terminate_verified(pid: u32, accept: &dyn Fn(&str) -> bool) -> Result<(), String> {
        let image = image_path(pid).ok_or_else(|| format!("Could not read process {pid}"))?;
        if !accept(&image) {
            return Err(format!("Process {pid} changed; refusing to stop it"));
        }
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|error| format!("Could not stop process {pid}: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Could not stop process {pid}"))
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    use super::ProcessInfo;

    pub fn image_path(_pid: u32) -> Option<String> {
        None
    }
    pub fn parent_pid(_pid: u32) -> Option<u32> {
        None
    }
    pub fn processes_named(_name: &str) -> Vec<ProcessInfo> {
        Vec::new()
    }
    pub fn terminate_verified(_pid: u32, _accept: &dyn Fn(&str) -> bool) -> Result<(), String> {
        Err("Stopping processes is not supported on this platform".to_string())
    }
}

#[cfg(test)]
#[path = "processes_tests.rs"]
mod tests;
