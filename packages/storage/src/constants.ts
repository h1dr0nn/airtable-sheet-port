/** The sidecar refreshes its own mcp_heartbeat row this often. */
export const HEARTBEAT_INTERVAL_MS = 10000;

/** A heartbeat row older than this is treated as a dead process. */
export const HEARTBEAT_STALE_MS = 30000;

/** Maximum rows returned when listing pending changes. */
export const CHANGE_LIST_LIMIT = 200;
