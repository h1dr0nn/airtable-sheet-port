//! MCP client auto-configuration.
//!
//! Registers (or removes) this app's MCP server inside the config files of
//! locally installed MCP clients - Claude Desktop, Claude Code, Cursor, and
//! so on - modelled on MCP-for-Unity / adb-compass. Everything here is
//! data-driven: [`registry`] lists the known clients with their per-OS config
//! path and the JSON shape they expect, and the three public functions
//! ([`detect_clients`], [`configure_client`], [`unregister_client`]) walk that
//! registry so adding a new client is a one-line change, never new logic.
//!
//! Writes are explicit and reversible: [`configure_client`] merges a single
//! entry keyed by [`MCP_CLIENT_SERVER_NAME`] into the client's server map and
//! preserves every other server; [`unregister_client`] removes only that one
//! entry. Parent directories are created but no other file content is touched.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::constants::{MCP_CLIENT_HTTP_PATH, MCP_CLIENT_SERVER_NAME};
use crate::error::CoreError;

/// The command + args (stdio) or url (http) this app should be launched with,
/// resolved by the caller (the desktop app knows the sidecar binary path and
/// the configured transport). Passed to [`configure_client`] and written into
/// each client's config in the shape that client expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerSpec {
    /// stdio launch: the client spawns `command` with `args` and speaks
    /// JSON-RPC over its stdio. The default for every client.
    Stdio { command: String, args: Vec<String> },
    /// Remote launch over the loopback HTTP transport. `url` is the full
    /// `http://127.0.0.1:{port}{path}` endpoint the sidecar serves.
    Http { url: String },
}

impl ServerSpec {
    /// Builds the HTTP spec url from a loopback port, using the shared MCP
    /// endpoint path so it always matches what the sidecar serves.
    pub fn http_for_port(port: u16) -> Self {
        Self::Http {
            url: format!("http://127.0.0.1:{port}{MCP_CLIENT_HTTP_PATH}"),
        }
    }
}

/// How a client's config JSON names the map that holds server entries, and
/// how a single entry is shaped. Most clients use the `mcpServers` object with
/// Claude Desktop's `{command,args}` / `{type:"sse",url}` entry shape; the
/// enum leaves room to add divergent shapes (e.g. VSCode's `servers` key)
/// without branching in the read/write code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigShape {
    /// `{ "mcpServers": { "<name>": <entry> } }`. Entry is `{command,args}`
    /// for stdio or `{type:"sse",url}` for http - the de-facto standard shared
    /// by Claude Desktop, Claude Code, Cursor, Windsurf, and Cline.
    McpServers,
}

impl ConfigShape {
    /// The top-level object key that holds the server map for this shape.
    fn servers_key(self) -> &'static str {
        match self {
            Self::McpServers => "mcpServers",
        }
    }

    /// Serializes a [`ServerSpec`] into this shape's per-entry JSON value.
    fn entry_value(self, spec: &ServerSpec) -> Value {
        match self {
            // `type: "sse"` is the http entry form the mcpServers clients read.
            Self::McpServers => match spec {
                ServerSpec::Stdio { command, args } => json!({
                    "command": command,
                    "args": args,
                }),
                ServerSpec::Http { url } => json!({
                    "type": "sse",
                    "url": url,
                }),
            },
        }
    }
}

/// A known MCP client and where/how to write its config. `config_path`
/// resolves the per-OS file location from environment variables (the same
/// `APPDATA` / `HOME` scheme [`crate::db`] uses); `None` means we cannot
/// locate it on this platform, so the client is reported `detectable = false`.
struct ClientDef {
    id: &'static str,
    display_name: &'static str,
    shape: ConfigShape,
    /// `false` for clients whose config path we cannot yet resolve reliably;
    /// they still appear in [`detect_clients`] (so the UI can list them) but
    /// `configure_client` / `unregister_client` refuse with a clear error.
    detectable: bool,
    config_path: fn() -> Option<PathBuf>,
}

/// One detected client's state for the UI: whether its config directory exists
/// (`installed`) and whether our server entry is already present
/// (`configured`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedClient {
    pub id: String,
    pub display_name: String,
    /// The client appears installed: its config directory exists on disk.
    pub installed: bool,
    /// Our server entry is present in the client's config file.
    pub configured: bool,
    /// We know how to locate this client's config on this OS. When false the
    /// UI should not offer to configure it.
    pub detectable: bool,
}

// --- Per-OS config-path resolvers ------------------------------------------
// Kept as small standalone fns (one per client) so the registry stays a flat
// data table. All read env vars directly rather than pulling in `dirs`, which
// core does not otherwise depend on.

fn home() -> Option<PathBuf> {
    // Windows has no HOME by default; USERPROFILE is the reliable home there.
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn windows_appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Claude Desktop: `%APPDATA%/Claude` (Windows),
/// `~/Library/Application Support/Claude` (macOS), `~/.config/Claude` (Linux).
fn claude_desktop_path() -> Option<PathBuf> {
    let dir = if cfg!(target_os = "windows") {
        windows_appdata()?.join("Claude")
    } else if cfg!(target_os = "macos") {
        home()?
            .join("Library")
            .join("Application Support")
            .join("Claude")
    } else {
        home()?.join(".config").join("Claude")
    };
    Some(dir.join("claude_desktop_config.json"))
}

/// Claude Code CLI: the global `~/.claude.json` (same on every OS) whose
/// top-level `mcpServers` object holds user-scoped servers.
fn claude_code_path() -> Option<PathBuf> {
    Some(home()?.join(".claude.json"))
}

/// Cursor: `~/.cursor/mcp.json` (global scope, same on every OS).
fn cursor_path() -> Option<PathBuf> {
    Some(home()?.join(".cursor").join("mcp.json"))
}

/// Windsurf: `~/.codeium/windsurf/mcp_config.json` (same on every OS).
fn windsurf_path() -> Option<PathBuf> {
    Some(
        home()?
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json"),
    )
}

/// Cline (VSCode extension `saoudrizwan.claude-dev`): its settings file lives
/// in VSCode's per-user globalStorage. Only the stable VSCode build's path is
/// resolved; Insiders / VSCodium / other forks use different roots and are not
/// covered here.
fn cline_path() -> Option<PathBuf> {
    let user_dir = if cfg!(target_os = "windows") {
        windows_appdata()?.join("Code").join("User")
    } else if cfg!(target_os = "macos") {
        home()?
            .join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
    } else {
        home()?.join(".config").join("Code").join("User")
    };
    Some(
        user_dir
            .join("globalStorage")
            .join("saoudrizwan.claude-dev")
            .join("settings")
            .join("cline_mcp_settings.json"),
    )
}

/// The data-driven client registry. Order is the order the UI lists them.
///
/// TODO(mcp-clients): VSCode Copilot is intentionally `detectable = false`.
/// Its workspace form is `.vscode/mcp.json` (requires a concrete project root
/// we do not have here) and it uses a different `{ "servers": { ... } }` shape
/// than the `mcpServers` clients. Add a `ConfigShape::Servers` variant plus a
/// project-root-aware path before enabling it - do not point it at a guessed
/// user-level path.
fn registry() -> Vec<ClientDef> {
    vec![
        ClientDef {
            id: "claude-desktop",
            display_name: "Claude Desktop",
            shape: ConfigShape::McpServers,
            detectable: true,
            config_path: claude_desktop_path,
        },
        ClientDef {
            id: "claude-code",
            display_name: "Claude Code",
            shape: ConfigShape::McpServers,
            detectable: true,
            config_path: claude_code_path,
        },
        ClientDef {
            id: "cursor",
            display_name: "Cursor",
            shape: ConfigShape::McpServers,
            detectable: true,
            config_path: cursor_path,
        },
        ClientDef {
            id: "windsurf",
            display_name: "Windsurf",
            shape: ConfigShape::McpServers,
            detectable: true,
            config_path: windsurf_path,
        },
        ClientDef {
            id: "cline",
            display_name: "Cline (VS Code)",
            shape: ConfigShape::McpServers,
            detectable: true,
            config_path: cline_path,
        },
        ClientDef {
            id: "vscode-copilot",
            display_name: "VS Code (Copilot)",
            shape: ConfigShape::McpServers,
            // See registry() TODO: path + shape not settled, so never written.
            detectable: false,
            config_path: || None,
        },
    ]
}

fn find_client(id: &str) -> Result<ClientDef, CoreError> {
    registry()
        .into_iter()
        .find(|client| client.id == id)
        .ok_or_else(|| CoreError::NotFound(format!("Unknown MCP client '{id}'")))
}

/// The resolved config path for a client that we know how to locate. Errors if
/// the client is not detectable or its path cannot be resolved on this OS.
fn resolve_writable_path(client: &ClientDef) -> Result<PathBuf, CoreError> {
    if !client.detectable {
        return Err(CoreError::Unsupported(format!(
            "Configuring '{}' is not supported yet",
            client.id
        )));
    }
    (client.config_path)().ok_or_else(|| {
        CoreError::Storage(format!(
            "Could not resolve the config path for '{}' on this platform",
            client.id
        ))
    })
}

/// Reads a client's config file into a JSON object. A missing file yields an
/// empty object (first-time configure); a present-but-malformed file is an
/// error rather than being silently overwritten, so we never clobber a config
/// we cannot understand.
fn read_config_object(path: &std::path::Path) -> Result<Map<String, Value>, CoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Map::new()),
        Ok(text) => {
            let value: Value = serde_json::from_str(&text).map_err(|error| {
                CoreError::Storage(format!(
                    "Config file {} is not valid JSON: {error}",
                    path.display()
                ))
            })?;
            match value {
                Value::Object(map) => Ok(map),
                _ => Err(CoreError::Storage(format!(
                    "Config file {} is not a JSON object",
                    path.display()
                ))),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(CoreError::Storage(format!(
            "Could not read config file {}: {error}",
            path.display()
        ))),
    }
}

/// Serializes and writes a config object back, creating parent directories.
/// Pretty-printed with a trailing newline so hand-edited files stay tidy.
fn write_config_object(
    path: &std::path::Path,
    object: Map<String, Value>,
) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            CoreError::Storage(format!(
                "Could not create config directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(object)).map_err(|error| {
        CoreError::Storage(format!(
            "Could not encode config for {}: {error}",
            path.display()
        ))
    })?;
    text.push('\n');
    std::fs::write(path, text).map_err(|error| {
        CoreError::Storage(format!(
            "Could not write config file {}: {error}",
            path.display()
        ))
    })
}

/// Borrows (creating if absent) the server map object under the shape's key.
/// Errors if the key exists but is not an object, so we never overwrite a
/// value the user put there.
fn servers_map<'a>(
    root: &'a mut Map<String, Value>,
    shape: ConfigShape,
    path: &std::path::Path,
) -> Result<&'a mut Map<String, Value>, CoreError> {
    let key = shape.servers_key();
    let entry = root
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    entry.as_object_mut().ok_or_else(|| {
        CoreError::Storage(format!(
            "Config file {} has a non-object '{key}'",
            path.display()
        ))
    })
}

/// Whether a client's config file already contains our server entry. A missing
/// or empty file (or a read error surfaced by [`read_config_object`]) means
/// not configured.
fn is_configured(client: &ClientDef) -> bool {
    let Some(path) = (client.config_path)() else {
        return false;
    };
    let Ok(root) = read_config_object(&path) else {
        return false;
    };
    root.get(client.shape.servers_key())
        .and_then(Value::as_object)
        .map(|servers| servers.contains_key(MCP_CLIENT_SERVER_NAME))
        .unwrap_or(false)
}

/// Detects every known client. `installed` is true when the client's config
/// directory exists on disk (best-effort proxy for "the client is present");
/// `configured` is true when our entry is already in its config file.
pub fn detect_clients() -> Vec<DetectedClient> {
    registry()
        .into_iter()
        .map(|client| {
            let path = (client.config_path)();
            let installed = path
                .as_deref()
                .and_then(std::path::Path::parent)
                .map(std::path::Path::is_dir)
                .unwrap_or(false);
            let configured = is_configured(&client);
            DetectedClient {
                id: client.id.to_string(),
                display_name: client.display_name.to_string(),
                installed,
                configured,
                detectable: client.detectable,
            }
        })
        .collect()
}

/// Merges our server entry into the config file at `path` (shape `shape`),
/// preserving every other server and all unrelated top-level keys. Creates the
/// file and parent directories on first configure. Pure over the filesystem
/// (no env/registry lookup) so it is unit-testable with temp paths.
fn merge_entry_into(
    path: &std::path::Path,
    shape: ConfigShape,
    spec: &ServerSpec,
) -> Result<(), CoreError> {
    let mut root = read_config_object(path)?;
    let servers = servers_map(&mut root, shape, path)?;
    servers.insert(MCP_CLIENT_SERVER_NAME.to_string(), shape.entry_value(spec));
    write_config_object(path, root)
}

/// Removes only our server entry from the config file at `path`. Idempotent:
/// a missing file, missing server map, or already-absent entry is a no-op
/// success. Returns whether a file was actually rewritten. Pure over the
/// filesystem so it is unit-testable with temp paths.
fn remove_entry_from(path: &std::path::Path, shape: ConfigShape) -> Result<bool, CoreError> {
    if !path.exists() {
        return Ok(false);
    }
    let mut root = read_config_object(path)?;
    let key = shape.servers_key();
    let removed = root
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .map(|servers| servers.remove(MCP_CLIENT_SERVER_NAME).is_some())
        .unwrap_or(false);
    if !removed {
        return Ok(false);
    }
    write_config_object(path, root)?;
    Ok(true)
}

/// Merges our server entry (named [`MCP_CLIENT_SERVER_NAME`]) into `id`'s
/// config, preserving every other server and all unrelated top-level keys.
/// Creates the file and parent directories on first configure. Returns the
/// path written so callers can audit/report it.
pub fn configure_client(id: &str, spec: &ServerSpec) -> Result<PathBuf, CoreError> {
    let client = find_client(id)?;
    let path = resolve_writable_path(&client)?;
    merge_entry_into(&path, client.shape, spec)?;
    Ok(path)
}

/// Removes only our server entry from `id`'s config, leaving every other
/// server and top-level key untouched. Idempotent: a missing file, missing
/// server map, or already-absent entry is a no-op success. Returns the path if
/// a file was rewritten, `None` when there was nothing to remove.
pub fn unregister_client(id: &str) -> Result<Option<PathBuf>, CoreError> {
    let client = find_client(id)?;
    let path = resolve_writable_path(&client)?;
    if remove_entry_from(&path, client.shape)? {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
#[path = "mcp_clients_tests.rs"]
mod tests;
