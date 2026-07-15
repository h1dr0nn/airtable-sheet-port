import {
  cn,
  FOCUS_RING,
  StatusDot,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@sheet-port/ui";
import {
  ArrowUpCircle,
  Database,
  GitPullRequest,
  LayoutDashboard,
  Settings as SettingsIcon,
  Table2,
  type LucideIcon,
} from "lucide-react";
import type { ReactNode } from "react";
import { useAppStatus } from "../hooks/useAppStatus.js";
import type { UpdateState } from "../hooks/useUpdate.js";
import { useTranslation, type TFunction } from "../i18n/useTranslation.js";
import { NAV, type ScreenId } from "../lib/nav.js";

const NAV_ICON_SIZE = 15;
const NAV_ICON_STROKE = 1.75;

// Rail widths. Expanded matches the prior w-56; collapsed is an icon-only rail.
const SIDEBAR_EXPANDED_CLASS = "w-56";
const SIDEBAR_COLLAPSED_CLASS = "w-14";

const NAV_ICONS: Record<ScreenId, LucideIcon> = {
  dashboard: LayoutDashboard,
  sources: Database,
  tables: Table2,
  changes: GitPullRequest,
  settings: SettingsIcon,
};

/** Wraps a rail control in a tooltip only while collapsed, since labels are
 * hidden then and the tooltip is the sole affordance. Expanded labels are
 * always visible, so no tooltip is needed. */
function RailTooltip({
  collapsed,
  label,
  children,
}: {
  collapsed: boolean;
  label: string;
  children: ReactNode;
}) {
  if (!collapsed) {
    return <>{children}</>;
  }
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

type SidebarProps = {
  active: ScreenId;
  onNavigate: (screen: ScreenId) => void;
  update: UpdateState;
  collapsed: boolean;
};

export function Sidebar({
  active,
  onNavigate,
  update,
  collapsed,
}: SidebarProps) {
  const { data: status } = useAppStatus();
  const { t } = useTranslation();
  const pendingCount = status?.pendingCount ?? 0;
  const mcpRunning = status?.mcpRunning ?? false;

  return (
    <aside
      className={cn(
        // min-w-0 so the declared rail width wins over the flex min-content size
        // of the (clipped) wordmark; overflow-hidden then hides the overflow.
        "flex min-w-0 shrink-0 flex-col overflow-hidden border-r border-edge bg-bg",
        "motion-safe:transition-[width] motion-safe:duration-[var(--dur-normal)]",
        "motion-safe:ease-[var(--ease-emphasized)]",
        collapsed ? SIDEBAR_COLLAPSED_CLASS : SIDEBAR_EXPANDED_CLASS,
      )}
    >
      <nav
        className="flex flex-1 flex-col overflow-y-auto px-3 py-4"
        aria-label="Main navigation"
      >
        <div className="space-y-0.5">
          {NAV.map((item) => {
            const isActive = active === item.id;
            const Icon = NAV_ICONS[item.id];
            const label = t(item.labelKey);
            return (
              <RailTooltip key={item.id} collapsed={collapsed} label={label}>
                <button
                  type="button"
                  aria-current={isActive ? "page" : undefined}
                  onClick={() => onNavigate(item.screen)}
                  className={cn(
                    "relative flex w-full items-center rounded-lg py-2 text-left",
                    "text-[13px] font-medium transition-colors",
                    collapsed ? "justify-center px-0" : "gap-2.5 px-3",
                    FOCUS_RING,
                    isActive
                      ? "bg-accent/[0.07] text-accent"
                      : "text-ink-muted hover:bg-surface hover:text-ink",
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
                    className={cn(
                      "shrink-0",
                      isActive ? "text-accent" : "text-ink-faint",
                    )}
                  />
                  {collapsed ? (
                    <span className="sr-only">{label}</span>
                  ) : (
                    <span className="truncate">{label}</span>
                  )}
                </button>
              </RailTooltip>
            );
          })}
        </div>

        {/* Update prompt lives at the bottom of the nav (mt-auto) so it fills the
            empty space below the menu instead of a separate footer box. */}
        {update.available ? (
          <div className="mt-auto pt-3">
            <UpdateCard update={update} collapsed={collapsed} t={t} />
          </div>
        ) : null}
      </nav>

      <div
        className={cn("border-t border-edge py-3", collapsed ? "px-2" : "px-3")}
      >
        <StatusCluster
          pendingCount={pendingCount}
          mcpRunning={mcpRunning}
          onNavigate={onNavigate}
          collapsed={collapsed}
          t={t}
        />
      </div>
    </aside>
  );
}

type StatusClusterProps = {
  pendingCount: number;
  mcpRunning: boolean;
  onNavigate: (screen: ScreenId) => void;
  collapsed: boolean;
  t: TFunction;
};

/** Default bottom cluster: pending approvals shortcut + MCP heartbeat status.
 * Collapsed condenses both into icon+dot rail controls with tooltips. */
function StatusCluster({
  pendingCount,
  mcpRunning,
  onNavigate,
  collapsed,
  t,
}: StatusClusterProps) {
  const mcpStatusLabel = mcpRunning ? t("common.running") : t("common.offline");
  if (collapsed) {
    const mcpLabel = `${t("dashboard.mcpServer")}: ${mcpStatusLabel}`;
    const pendingLabel =
      pendingCount > 0
        ? `${t("dashboard.pendingApprovals")}: ${pendingCount}`
        : t("dashboard.nothingWaiting");
    return (
      <div className="flex flex-col items-center gap-1">
        <RailTooltip collapsed label={pendingLabel}>
          <button
            type="button"
            aria-label={pendingLabel}
            onClick={() => onNavigate("changes")}
            className={cn(
              "relative flex h-9 w-9 items-center justify-center rounded-lg",
              "text-ink-muted transition-colors hover:bg-surface hover:text-ink",
              FOCUS_RING,
            )}
          >
            <GitPullRequest
              size={15}
              strokeWidth={NAV_ICON_STROKE}
              aria-hidden
            />
            {pendingCount > 0 ? (
              <span
                aria-hidden
                className="absolute right-1 top-1 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-warning/15 px-1 text-[10px] font-semibold tabular-nums text-warning"
              >
                {pendingCount}
              </span>
            ) : null}
          </button>
        </RailTooltip>
        <RailTooltip collapsed label={mcpLabel}>
          <div
            className="flex h-9 w-9 items-center justify-center rounded-lg"
            aria-label={mcpLabel}
            role="img"
          >
            <StatusDot status={mcpRunning ? "live" : "idle"} />
          </div>
        </RailTooltip>
      </div>
    );
  }

  return (
    <>
      <button
        type="button"
        onClick={() => onNavigate("changes")}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left",
          "text-[13px] font-medium text-ink-muted transition-colors hover:bg-surface hover:text-ink",
          FOCUS_RING,
        )}
      >
        {t("dashboard.pendingApprovals")}
        <span
          className={cn(
            "inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1.5",
            "text-[11px] font-semibold tabular-nums",
            pendingCount > 0
              ? "bg-warning/15 text-warning"
              : "bg-edge/60 text-ink-muted",
          )}
        >
          {pendingCount}
        </span>
      </button>
      <div className="flex items-center gap-2 px-3 pb-1 pt-2 text-[12px]">
        <StatusDot status={mcpRunning ? "live" : "idle"} />
        <span className="text-ink-muted">{t("dashboard.mcpServer")}</span>
        <span
          className={cn(
            "ml-auto font-medium",
            mcpRunning ? "text-success" : "text-ink-muted",
          )}
        >
          {mcpStatusLabel}
        </span>
      </div>
    </>
  );
}

/** Accent-tinted notification shown in place of the status cluster when a newer
 * version is available. The button drives update.install() (download + relaunch).
 * Collapsed shrinks to a single accent icon button that still triggers install. */
function UpdateCard({
  update,
  collapsed,
  t,
}: {
  update: UpdateState;
  collapsed: boolean;
  t: TFunction;
}) {
  if (collapsed) {
    const label = update.version
      ? t("sidebar.updateAvailableVersion", { version: update.version })
      : t("sidebar.updateAvailable");
    return (
      <div className="flex justify-center">
        <RailTooltip
          collapsed
          label={update.downloading ? t("sidebar.downloadingUpdate") : label}
        >
          <button
            type="button"
            aria-label={label}
            onClick={() => void update.install()}
            disabled={update.downloading}
            className={cn(
              "flex h-9 w-9 items-center justify-center rounded-lg",
              "border border-accent/30 bg-accent/[0.07] text-accent transition-colors",
              "hover:bg-accent/15 disabled:pointer-events-none disabled:opacity-60",
              FOCUS_RING,
            )}
          >
            <ArrowUpCircle size={16} strokeWidth={1.75} aria-hidden />
          </button>
        </RailTooltip>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-accent/30 bg-accent/[0.07] p-3">
      <div className="flex items-center gap-2">
        <ArrowUpCircle
          size={15}
          strokeWidth={1.75}
          aria-hidden
          className="shrink-0 text-accent"
        />
        <div className="min-w-0">
          <p className="truncate text-[12.5px] font-semibold text-accent">
            {t("sidebar.updateAvailable")}
          </p>
          {update.version ? (
            <p className="truncate text-[11px] text-ink-muted">
              v{update.version}
            </p>
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
          FOCUS_RING,
        )}
      >
        {update.downloading ? t("sidebar.downloading") : t("sidebar.update")}
      </button>
    </div>
  );
}
