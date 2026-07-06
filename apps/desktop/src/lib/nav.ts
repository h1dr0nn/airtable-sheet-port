import {
  Database,
  GitPullRequestArrow,
  LayoutDashboard,
  ScrollText,
  ShieldCheck,
  Table2,
  type LucideIcon
} from "lucide-react";

export type ScreenId = "dashboard" | "sources" | "tables" | "permissions" | "changes" | "audit";

export type NavItem = {
  id: ScreenId;
  label: string;
  icon: LucideIcon;
  screen: ScreenId;
};

export const NAV: readonly NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, screen: "dashboard" },
  { id: "sources", label: "Data Sources", icon: Database, screen: "sources" },
  { id: "tables", label: "Tables", icon: Table2, screen: "tables" },
  { id: "permissions", label: "Permissions", icon: ShieldCheck, screen: "permissions" },
  { id: "changes", label: "Changes", icon: GitPullRequestArrow, screen: "changes" },
  { id: "audit", label: "Audit Log", icon: ScrollText, screen: "audit" }
] as const;
