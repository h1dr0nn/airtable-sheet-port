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
