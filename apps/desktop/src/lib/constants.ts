export const TABLE_PAGE_SIZE = 25;
export const AUDIT_PAGE_SIZE = 100;
export const APP_STATUS_REFETCH_MS = 5000;
export const DASHBOARD_AUDIT_COUNT = 8;
export const DASHBOARD_CHANGES_COUNT = 5;

/** Shown on the dashboard when the MCP server is not running. */
export const CLAUDE_DESKTOP_CONFIG_HINT = `{
  "mcpServers": {
    "sheet-port": {
      "command": "node",
      "args": ["<repo>/apps/mcp-server/dist/index.js"]
    }
  }
}`;
