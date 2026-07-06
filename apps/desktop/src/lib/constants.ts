export const APP_NAME = "Airtable - Sheet Port";
export const APP_AUTHOR = "h1dr0n";

export const TABLE_PAGE_SIZE = 25;
export const AUDIT_PAGE_SIZE = 100;
/** Smaller page size for the titlebar activity dropdown. */
export const AUDIT_DROPDOWN_PAGE_SIZE = 50;
export const APP_STATUS_REFETCH_MS = 5000;
export const DASHBOARD_AUDIT_COUNT = 8;
export const DASHBOARD_CHANGES_COUNT = 5;

/** Shown on the dashboard when the MCP server is not running. */
export const CLAUDE_DESKTOP_CONFIG_HINT = `{
  "mcpServers": {
    "sheet-port": {
      "command": "<repo>/target/release/sheet-port-mcp.exe",
      "args": []
    }
  }
}`;

/** Stdio launch command clients use to spawn the MCP sidecar. */
export const MCP_STDIO_COMMAND = "<repo>/target/release/sheet-port-mcp.exe";

/** Bounds enforced by the backend's set_mcp_port (docs/ipc.md). */
export const MCP_PORT_MIN = 1024;
export const MCP_PORT_MAX = 65_535;

/** Local HTTP endpoint clients connect to; {port} is the configured port. */
export const buildMcpHttpUrl = (port: number): string => `http://127.0.0.1:${port}/mcp`;

/**
 * Root data attributes CSS reads to apply the live font preferences. Keep in
 * sync with the selectors in styles.css.
 */
export const FONT_SCALE_ATTR = "data-font-scale";
export const FONT_FAMILY_ATTR = "data-font-family";
