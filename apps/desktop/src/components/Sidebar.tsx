import { cn, StatusDot } from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { NAV, type ScreenId } from "../lib/nav.js";

const NAV_ICON_SIZE = 16;

type SidebarProps = {
  active: ScreenId;
  onNavigate: (screen: ScreenId) => void;
};

export function Sidebar({ active, onNavigate }: SidebarProps) {
  const { data: status } = useAppStatus();
  const pendingCount = status?.pendingCount ?? 0;
  const mcpRunning = status?.mcpRunning ?? false;

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-edge bg-surface">
      <nav className="flex-1 space-y-0.5 px-3 py-4" aria-label="Main navigation">
        <p className="mb-2 px-3 text-[11px] font-semibold uppercase tracking-widest text-ink-muted/70">
          Console
        </p>
        {NAV.map((item) => {
          const Icon = item.icon;
          const isActive = active === item.id;
          return (
            <button
              key={item.id}
              type="button"
              aria-current={isActive ? "page" : undefined}
              onClick={() => onNavigate(item.screen)}
              className={cn(
                "relative flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-left text-[13px] font-medium",
                "transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
                isActive
                  ? "bg-raised text-ink"
                  : "text-ink-muted hover:bg-raised/60 hover:text-ink"
              )}
            >
              {isActive ? (
                <span className="absolute -left-3 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-r-full bg-accent" />
              ) : null}
              <Icon size={NAV_ICON_SIZE} aria-hidden className={isActive ? "text-accent" : undefined} />
              {item.label}
            </button>
          );
        })}
      </nav>

      <div className="space-y-2 border-t border-edge px-3 py-3">
        <div className="flex items-center gap-2.5 rounded-md bg-raised/60 px-3 py-2">
          <StatusDot status={mcpRunning ? "live" : "idle"} />
          <div className="min-w-0">
            <p className="text-[11px] font-semibold uppercase tracking-widest text-ink-muted">MCP server</p>
            <p className={cn("text-xs font-medium", mcpRunning ? "text-accent" : "text-ink-muted")}>
              {mcpRunning ? "Running" : "Stopped"}
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={() => onNavigate("changes")}
          className={cn(
            "flex w-full items-center justify-between rounded-md px-3 py-2 text-xs font-medium transition-colors",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
            pendingCount > 0
              ? "bg-warning/10 text-warning hover:bg-warning/15"
              : "text-ink-muted hover:bg-raised/60 hover:text-ink"
          )}
        >
          Pending approvals
          <span
            className={cn(
              "inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1.5 font-mono text-[11px]",
              pendingCount > 0 ? "bg-warning/20 text-warning" : "bg-raised text-ink-muted"
            )}
          >
            {pendingCount}
          </span>
        </button>
      </div>
    </aside>
  );
}
