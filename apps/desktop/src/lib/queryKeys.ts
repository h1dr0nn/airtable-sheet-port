export const queryKeys = {
  appStatus: ["app-status"] as const,
  sources: ["sources"] as const,
  tablesRoot: ["tables"] as const,
  tables: (sourceId: string) => ["tables", sourceId] as const,
  tableSchema: (sourceId: string, tableId: string) => ["table-schema", sourceId, tableId] as const,
  tablePage: (sourceId: string, tableId: string, page: number) =>
    ["table-page", sourceId, tableId, page] as const,
  permissionRules: ["permission-rules"] as const,
  changesRoot: ["changes"] as const,
  changes: (status: string | null) => ["changes", status ?? "all"] as const,
  auditEvents: ["audit-events"] as const,
  auditEventsPaged: (pageSize: number) => ["audit-events", pageSize] as const,
  tokenStatus: ["token-status"] as const,
  googleConfig: ["google-config"] as const,
  googleAccounts: ["google-accounts"] as const,
  settings: ["settings"] as const,
  autostart: ["autostart"] as const,
  mcpConfig: ["mcp-config"] as const,
  mcpClients: ["mcp-clients"] as const,
  // Workbench
  workbenchTree: ["workbench-tree"] as const,
  sheetTabs: (itemId: string) => ["workbench-sheet-tabs", itemId] as const,
  sheet: (itemId: string, gid: string) => ["workbench-sheet", itemId, gid] as const
};
