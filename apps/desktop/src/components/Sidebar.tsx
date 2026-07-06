import { cn, FOCUS_RING, StatusDot } from "@sheet-port/ui";
import {
  ArrowUpCircle,
  Database,
  GitPullRequest,
  LayoutDashboard,
  Settings as SettingsIcon,
  Table2,
  type LucideIcon
} from "lucide-react";
import { useAppStatus } from "../hooks/useAppStatus.js";
import type { UpdateState } from "../hooks/useUpdate.js";
import { APP_NAME } from "../lib/constants.js";
import { NAV, type ScreenId } from "../lib/nav.js";

const NAV_ICON_SIZE = 15;
const NAV_ICON_STROKE = 1.75;

const NAV_ICONS: Record<ScreenId, LucideIcon> = {
  dashboard: LayoutDashboard,
  sources: Database,
  tables: Table2,
  changes: GitPullRequest,
  settings: SettingsIcon
};

type SidebarProps = {
  active: ScreenId;
  onNavigate: (screen: ScreenId) => void;
  update: UpdateState;
};

export function Sidebar({ active, onNavigate, update }: SidebarProps) {
  const { data: status } = useAppStatus();
  const pendingCount = status?.pendingCount ?? 0;
  const mcpRunning = status?.mcpRunning ?? false;

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-edge bg-bg">
      {/* Wordmark: text only, the app identity lives here, not in the titlebar. */}
      <div className="px-6 pb-1 pt-4">
        <h1 className="truncate text-[11.5px] font-semibold uppercase tracking-wide text-ink-muted">
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
        {update.available ? (
          <UpdateCard update={update} />
        ) : (
          <StatusCluster
            pendingCount={pendingCount}
            mcpRunning={mcpRunning}
            onNavigate={onNavigate}
          />
        )}
      </div>
    </aside>
  );
}

type StatusClusterProps = {
  pendingCount: number;
  mcpRunning: boolean;
  onNavigate: (screen: ScreenId) => void;
};

/** Default bottom cluster: pending approvals shortcut + MCP heartbeat status. */
function StatusCluster({ pendingCount, mcpRunning, onNavigate }: StatusClusterProps) {
  return (
    <>
      <button
        type="button"
        onClick={() => onNavigate("changes")}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left",
          "text-[13px] font-medium text-ink-muted transition-colors hover:bg-surface hover:text-ink",
          FOCUS_RING
        )}
      >
        Pending Approvals
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
        <span className="text-ink-muted">MCP Server</span>
        <span className={cn("ml-auto font-medium", mcpRunning ? "text-success" : "text-ink-muted")}>
          {mcpRunning ? "Running" : "Offline"}
        </span>
      </div>
    </>
  );
}

/** Accent-tinted notification shown in place of the status cluster when a newer
 * version is available. The button drives update.install() (download + relaunch). */
function UpdateCard({ update }: { update: UpdateState }) {
  return (
    <div className="rounded-lg border border-accent/30 bg-accent/[0.07] p-3">
      <div className="flex items-center gap-2">
        <ArrowUpCircle size={15} strokeWidth={1.75} aria-hidden className="shrink-0 text-accent" />
        <div className="min-w-0">
          <p className="truncate text-[12.5px] font-semibold text-accent">Update Available</p>
          {update.version ? (
            <p className="truncate text-[11px] text-ink-muted">v{update.version}</p>
          ) : null}
        </div>
      </div>
      <button
        type="button"
        onClick={() => void update.install()}
        disabled={update.downloading}
        className={cn(
          "mt-2.5 flex w-full items-center justify-center rounded-md px-3 py-1.5",
          "bg-accent text-[12px] font-semibold text-accent-ink transition-colors hover:bg-accent-hover",
          "disabled:pointer-events-none disabled:opacity-60",
          FOCUS_RING
        )}
      >
        {update.downloading ? "Downloading..." : "Update"}
      </button>
    </div>
  );
}
