import { cn, FOCUS_RING, StatusDot } from "@sheet-port/ui";
import {
  Database,
  GitPullRequest,
  LayoutDashboard,
  ScrollText,
  Settings as SettingsIcon,
  Table2,
  type LucideIcon
} from "lucide-react";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { APP_NAME } from "../lib/constants.js";
import { NAV, type ScreenId } from "../lib/nav.js";

const NAV_ICON_SIZE = 15;
const NAV_ICON_STROKE = 1.75;

const NAV_ICONS: Record<ScreenId, LucideIcon> = {
  dashboard: LayoutDashboard,
  sources: Database,
  tables: Table2,
  changes: GitPullRequest,
  audit: ScrollText,
  settings: SettingsIcon
};

type SidebarProps = {
  active: ScreenId;
  onNavigate: (screen: ScreenId) => void;
};

export function Sidebar({ active, onNavigate }: SidebarProps) {
  const { data: status } = useAppStatus();
  const pendingCount = status?.pendingCount ?? 0;
  const mcpRunning = status?.mcpRunning ?? false;

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-edge bg-bg">
      {/* Wordmark: the app identity lives here, not in the titlebar. */}
      <div className="flex items-center gap-2 px-6 pb-1 pt-4">
        <span aria-hidden className="h-2 w-2 shrink-0 rounded-[3px] bg-accent" />
        <h1 className="truncate text-[12.5px] font-semibold tracking-[-0.01em] text-ink">
          {APP_NAME}
        </h1>
      </div>
      <nav className="flex-1 space-y-0.5 overflow-y-auto px-3 py-4" aria-label="Main navigation">
        {NAV.map((item) => {
          const isActive = active === item.id;
          const Icon = NAV_ICONS[item.id];
          return (
            <button
              key={item.id}
              type="button"
              aria-current={isActive ? "page" : undefined}
              onClick={() => onNavigate(item.screen)}
              className={cn(
                "relative flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left",
                "text-[13px] font-medium transition-colors",
                FOCUS_RING,
                isActive
                  ? "bg-accent/[0.07] text-accent"
                  : "text-ink-muted hover:bg-surface hover:text-ink"
              )}
            >
              {/* Active marker: 3px rounded accent bar on the sidebar edge. */}
              {isActive ? (
                <span
                  aria-hidden
                  className="absolute -left-3 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-accent"
                />
              ) : null}
              <Icon
                size={NAV_ICON_SIZE}
                strokeWidth={NAV_ICON_STROKE}
                aria-hidden
                className={cn("shrink-0", isActive ? "text-accent" : "text-ink-faint")}
              />
              {item.label}
            </button>
          );
        })}
      </nav>

      <div className="border-t border-edge px-3 py-3">
        <button
          type="button"
          onClick={() => onNavigate("changes")}
          className={cn(
            "flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left",
            "text-[13px] font-medium text-ink-muted transition-colors hover:bg-surface hover:text-ink",
            FOCUS_RING
          )}
        >
          Pending approvals
          <span
            className={cn(
              "inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1.5",
              "text-[11px] font-semibold tabular-nums",
              pendingCount > 0 ? "bg-warning/15 text-warning" : "bg-edge/60 text-ink-muted"
            )}
          >
            {pendingCount}
          </span>
        </button>
        <div className="flex items-center gap-2 px-3 pb-1 pt-2 text-[12px]">
          <StatusDot status={mcpRunning ? "live" : "idle"} />
          <span className="text-ink-muted">MCP server</span>
          <span className={cn("ml-auto font-medium", mcpRunning ? "text-success" : "text-ink-muted")}>
            {mcpRunning ? "Running" : "Offline"}
          </span>
        </div>
      </div>
    </aside>
  );
}
