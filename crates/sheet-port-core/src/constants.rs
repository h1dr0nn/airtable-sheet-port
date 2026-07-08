//! Contract constants shared by the desktop app and the MCP server. Values
//! must match docs/mcp-tools.md and docs/ipc.md.

/// Update previews touching more than this many records are treated as
/// `bulk_update` when permission rules are evaluated.
pub const BULK_UPDATE_THRESHOLD: usize = 20;

/// The MCP server refreshes its own mcp_heartbeat row this often.
pub const HEARTBEAT_INTERVAL_MS: i64 = 10_000;

/// A heartbeat row older than this is treated as a dead process.
pub const HEARTBEAT_STALE_MS: i64 = 30_000;

/// read_table paging: default page size and the allowed clamp range.
pub const READ_LIMIT_DEFAULT: i64 = 100;
pub const READ_LIMIT_MIN: i64 = 1;
pub const READ_LIMIT_MAX: i64 = 500;

/// find_records query strings longer than this are rejected.
pub const FIND_QUERY_MAX_LEN: usize = 200;

/// Cap for find_records results so agents never receive unbounded payloads.
pub const FIND_RECORDS_LIMIT: usize = 100;

/// Maximum records/patches accepted per append or update preview call.
pub const WRITE_BATCH_MAX: usize = 100;

/// Maximum cell-format operations (and column-width entries) accepted per
/// preview_format_table call, bounding a single formatting change.
pub const FORMAT_OPS_MAX: usize = 100;

/// Allowed font-size range (points) for a cell-format operation.
pub const FONT_SIZE_MIN: i64 = 1;
pub const FONT_SIZE_MAX: i64 = 400;

/// Allowed column-width range (pixels) for a column-width override.
pub const COLUMN_WIDTH_MIN: i64 = 2;
pub const COLUMN_WIDTH_MAX: i64 = 2000;

/// Maximum rows or columns a formatting change may freeze.
pub const FREEZE_MAX: i64 = 100;

/// Maximum rows returned when listing pending changes.
pub const CHANGE_LIST_LIMIT: i64 = 200;

/// Audit log paging: default page size and maximum.
pub const AUDIT_LIMIT_DEFAULT: i64 = 100;
pub const AUDIT_LIMIT_MAX: i64 = 500;

/// Meta key storing the Google OAuth desktop client id. Written from the
/// desktop app settings (db::set_meta) and read by the Google connect and
/// token-refresh flows.
pub const META_GOOGLE_CLIENT_ID: &str = "google_client_id";

/// Meta key for the auto-approve-writes opt-in. Absent (or any value other
/// than "1") means off: agent writes still require human confirmation. Value
/// "1" bypasses the confirmation gate at commit time (see docs/security.md).
pub const META_AUTO_APPROVE_WRITES: &str = "auto_approve_writes";

/// Meta value that turns a boolean opt-in setting on.
pub const META_FLAG_ON: &str = "1";

/// Meta key for the UI font scale appearance preference. Absent means the
/// default ("normal"). See docs/ipc.md "Settings".
pub const META_UI_FONT_SCALE: &str = "ui_font_scale";

/// Allowed `ui_font_scale` values; the first is the default when the key is
/// absent or invalid.
pub const UI_FONT_SCALE_SMALL: &str = "small";
pub const UI_FONT_SCALE_NORMAL: &str = "normal";
pub const UI_FONT_SCALE_LARGE: &str = "large";
pub const UI_FONT_SCALE_VALUES: [&str; 3] = [
    UI_FONT_SCALE_NORMAL,
    UI_FONT_SCALE_SMALL,
    UI_FONT_SCALE_LARGE,
];
pub const UI_FONT_SCALE_DEFAULT: &str = UI_FONT_SCALE_NORMAL;

/// Meta key for the UI font family appearance preference. Absent means the
/// default ("modern"). See docs/ipc.md "Settings".
pub const META_UI_FONT_FAMILY: &str = "ui_font_family";

/// Allowed `ui_font_family` values; the first is the default when the key is
/// absent or invalid.
pub const UI_FONT_FAMILY_CLASSIC: &str = "classic";
pub const UI_FONT_FAMILY_MODERN: &str = "modern";
pub const UI_FONT_FAMILY_SYSTEM: &str = "system";
pub const UI_FONT_FAMILY_VALUES: [&str; 3] = [
    UI_FONT_FAMILY_MODERN,
    UI_FONT_FAMILY_CLASSIC,
    UI_FONT_FAMILY_SYSTEM,
];
pub const UI_FONT_FAMILY_DEFAULT: &str = UI_FONT_FAMILY_MODERN;

/// Meta key for the UI language preference. Absent means the default ("en").
/// See docs/ipc.md "Settings".
pub const META_UI_LANGUAGE: &str = "ui_language";

/// Allowed `ui_language` values; the first is the default when the key is
/// absent or invalid.
pub const UI_LANGUAGE_EN: &str = "en";
pub const UI_LANGUAGE_VI: &str = "vi";
pub const UI_LANGUAGE_VALUES: [&str; 2] = [UI_LANGUAGE_EN, UI_LANGUAGE_VI];
pub const UI_LANGUAGE_DEFAULT: &str = UI_LANGUAGE_EN;

/// Meta key selecting the MCP sidecar transport. Absent or "stdio" keeps the
/// default stdio transport; "http" serves the same tools over a loopback HTTP
/// endpoint. See docs/architecture.md and docs/security.md.
pub const META_MCP_TRANSPORT: &str = "mcp_transport";

/// Meta value for the default stdio transport (also the implicit default when
/// META_MCP_TRANSPORT is absent).
pub const MCP_TRANSPORT_STDIO: &str = "stdio";

/// Meta value selecting the loopback HTTP transport.
pub const MCP_TRANSPORT_HTTP: &str = "http";

/// Meta key for the port the HTTP transport binds on 127.0.0.1. Only read when
/// the transport is "http". Stored as a decimal string.
pub const META_MCP_PORT: &str = "mcp_port";

/// Default HTTP transport port when META_MCP_PORT is absent or invalid.
pub const MCP_PORT_DEFAULT: u16 = 4319;

/// Lowest port the HTTP transport accepts. Below 1024 are privileged ports.
pub const MCP_PORT_MIN: u16 = 1024;

/// Highest port the HTTP transport accepts (top of the u16 range).
pub const MCP_PORT_MAX: u16 = 65535;

/// The server-entry name this app writes into every MCP client's config
/// (the key inside their `mcpServers` object). Stable so re-configuring a
/// client overwrites our own entry and never touches the user's other
/// servers, and so `unregister_client` can find exactly what to remove.
pub const MCP_CLIENT_SERVER_NAME: &str = "airtable-sheet-port";

/// Meta key selecting what happens when the user closes the main window.
/// Absent means the default ("ask"). See docs/development.md "Run in
/// background".
pub const META_CLOSE_BEHAVIOR: &str = "close_behavior";

/// Allowed `close_behavior` values; the first is the default when the key is
/// absent or invalid. "ask" prompts each time, "tray" hides to the system
/// tray, "quit" exits the app.
pub const CLOSE_BEHAVIOR_ASK: &str = "ask";
pub const CLOSE_BEHAVIOR_TRAY: &str = "tray";
pub const CLOSE_BEHAVIOR_QUIT: &str = "quit";
pub const CLOSE_BEHAVIOR_VALUES: [&str; 3] =
    [CLOSE_BEHAVIOR_ASK, CLOSE_BEHAVIOR_TRAY, CLOSE_BEHAVIOR_QUIT];
pub const CLOSE_BEHAVIOR_DEFAULT: &str = CLOSE_BEHAVIOR_ASK;

/// The single MCP endpoint path the HTTP transport serves (mirrors
/// `sheet-port-mcp` http::MCP_HTTP_PATH). Used to build the advertised
/// `http://127.0.0.1:{port}{MCP_CLIENT_HTTP_PATH}` url when configuring a
/// client for the HTTP transport.
pub const MCP_CLIENT_HTTP_PATH: &str = "/mcp";
