export type ScreenId = "dashboard" | "sources" | "tables" | "permissions" | "changes" | "audit";

export type NavItem = {
  id: ScreenId;
  label: string;
  screen: ScreenId;
};

// Nav is pure mono text + markers; the terminal language carries itself without icons.
export const NAV: readonly NavItem[] = [
  { id: "dashboard", label: "Dashboard", screen: "dashboard" },
  { id: "sources", label: "Data Sources", screen: "sources" },
  { id: "tables", label: "Tables", screen: "tables" },
  { id: "permissions", label: "Permissions", screen: "permissions" },
  { id: "changes", label: "Changes", screen: "changes" },
  { id: "audit", label: "Audit Log", screen: "audit" }
] as const;
