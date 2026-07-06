export type ScreenId =
  | "dashboard"
  | "sources"
  | "tables"
  | "changes"
  | "settings";

export type NavItem = {
  id: ScreenId;
  label: string;
  screen: ScreenId;
};

// Pure data; the sidebar maps ids to icons so nav stays framework-free.
export const NAV: readonly NavItem[] = [
  { id: "dashboard", label: "Dashboard", screen: "dashboard" },
  { id: "sources", label: "Data Sources", screen: "sources" },
  { id: "tables", label: "Tables", screen: "tables" },
  { id: "changes", label: "Changes", screen: "changes" },
  { id: "settings", label: "Settings", screen: "settings" }
] as const;
