import { cn, StatusDot } from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { NAV, type ScreenId } from "../lib/nav.js";

const FALLBACK_REV = "0.1.0";

type SidebarProps = {
  active: ScreenId;
  onNavigate: (screen: ScreenId) => void;
};

export function Sidebar({ active, onNavigate }: SidebarProps) {
  const { data: status } = useAppStatus();
  const pendingCount = status?.pendingCount ?? 0;
  const mcpRunning = status?.mcpRunning ?? false;
  const rev = status?.appVersion ?? FALLBACK_REV;

  return (
    <aside className="flex w-56 shrink-0 flex-col bg-bg">
      <nav className="flex-1 overflow-y-auto py-6" aria-label="Main navigation">
        <p className="mb-3 px-5 font-mono text-[10px] uppercase tracking-[0.15em] text-ink-muted">
          /// Console
        </p>
        {NAV.map((item) => {
          const isActive = active === item.id;
          return (
            <button
              key={item.id}
              type="button"
              aria-current={isActive ? "page" : undefined}
              onClick={() => onNavigate(item.screen)}
              className={cn(
                "relative flex w-full items-center gap-2 px-5 py-2 text-left",
                "font-mono text-[11px] uppercase tracking-[0.08em] transition-colors",
                "focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-2 focus-visible:outline-hazard",
                isActive ? "text-ink" : "text-ink-muted hover:text-ink"
              )}
            >
              {/* Active strike marker: 2px hazard bar on the compartment edge. */}
              {isActive ? <span aria-hidden className="absolute left-0 top-0 h-full w-0.5 bg-hazard" /> : null}
              <span aria-hidden className={cn("w-2 shrink-0", isActive ? "text-hazard" : "opacity-0")}>
                {">"}
              </span>
              {item.label}
            </button>
          );
        })}
      </nav>

      <div className="border-t border-edge px-5 py-3">
        <p className="font-mono text-[10px] font-bold uppercase tracking-[0.1em] text-ink-muted">
          [ MCP Server ]
        </p>
        <p className="mt-1.5 flex items-center gap-2">
          <StatusDot status={mcpRunning ? "live" : "idle"} />
          <span
            className={cn(
              "font-mono text-[11px] font-bold uppercase tracking-[0.08em]",
              mcpRunning ? "text-signal" : "text-ink-muted"
            )}
          >
            {mcpRunning ? "Running" : "Offline"}
          </span>
        </p>
      </div>

      <button
        type="button"
        onClick={() => onNavigate("changes")}
        className={cn(
          "flex w-full items-center justify-between border-t border-edge px-5 py-3 text-left transition-colors",
          "font-mono text-[11px] uppercase tracking-[0.08em]",
          "focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-2 focus-visible:outline-hazard",
          pendingCount > 0 ? "text-ink hover:text-hazard" : "text-ink-muted hover:text-ink"
        )}
      >
        Pending
        <span
          className={cn(
            "inline-flex h-5 min-w-5 items-center justify-center border px-1.5 font-mono text-[11px] tabular-nums",
            pendingCount > 0 ? "border-hazard text-hazard" : "border-edge text-ink-muted"
          )}
        >
          {pendingCount}
        </span>
      </button>

      <footer className="border-t border-edge px-5 py-3">
        <p className="font-mono text-[9px] uppercase tracking-[0.08em] text-ink-muted">
          Sheet-Port(R) Rev {rev} / Local-Only
        </p>
      </footer>
    </aside>
  );
}
