import { useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  cn,
  Skeleton,
  StatusDot,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { useAuditEvents } from "../hooks/useAuditEvents.js";
import { useRestartClaudeDesktop, useStopSidecar } from "../hooks/useMcp.js";
import { useSources } from "../hooks/useSources.js";
import { useTokenStatus } from "../hooks/useTokenStatus.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { DASHBOARD_AUDIT_COUNT } from "../lib/constants.js";
import type { SidecarHeartbeat } from "../lib/ipc.js";
import {
  isDevBuild,
  isSidecarOutdated,
  normalizeVersion,
  outdatedSidecars,
  outdatedVersionLabel,
  sidecarClient,
  type SidecarClient
} from "../lib/sidecars.js";
import type { ScreenId } from "../lib/nav.js";
import { ConfirmDialog } from "../components/ConfirmDialog.js";
import { CopyButton } from "../components/CopyButton.js";
import { RelativeTime } from "../components/RelativeTime.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const ACTOR_VARIANTS: Record<"agent" | "user" | "system", BadgeVariant> = {
  agent: "accent",
  user: "default",
  system: "muted"
};

type StatCardProps = {
  label: string;
  action?: ReactNode;
  className?: string;
  children: ReactNode;
};

function StatCard({ label, action, className, children }: StatCardProps) {
  return (
    <section
      className={cn(
        "flex flex-col rounded-card border border-edge bg-raised p-5 shadow-card",
        className
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <p className="overline-label">{label}</p>
        {action}
      </div>
      <div className="mt-3 flex-1">{children}</div>
    </section>
  );
}

// The sidecar card spans the first row of the 2-column md grid so the three
// stat cards leave no gap; at xl all three sit side by side.
const MCP_CARD_SPAN = "md:col-span-2 xl:col-span-1";

type SidecarListProps = {
  sidecars: SidecarHeartbeat[];
  appVersion: string;
  bundledSidecarPath: string | null;
  managedSidecarPid: number | null;
};

/** The translated row label for a sidecar's client (see lib/sidecars.ts). */
function useClientLabel(): (client: SidecarClient) => string {
  const { t } = useTranslation();
  return (client) => {
    switch (client.kind) {
      case "named":
        return client.label;
      case "app":
        return t("dashboard.clientApp");
      case "pending":
        return t("dashboard.clientPending");
      case "unknownOlder":
        return t("dashboard.clientUnknownOlder");
    }
  };
}

/** Tooltip body: the raw clientInfo plus the sidecar and parent executables. */
function SidecarDetails({ sidecar }: { sidecar: SidecarHeartbeat }) {
  const { t } = useTranslation();
  const lines: string[] = [];
  if (sidecar.clientName) {
    const raw = sidecar.clientVersion
      ? `${sidecar.clientName} ${sidecar.clientVersion}`
      : sidecar.clientName;
    lines.push(t("dashboard.sidecarRawClient", { name: raw }));
  }
  if (sidecar.exePath) {
    lines.push(t("dashboard.sidecarExePath", { path: sidecar.exePath }));
  }
  if (sidecar.parentExePath) {
    lines.push(t("dashboard.sidecarParentPath", { path: sidecar.parentExePath }));
  }
  if (lines.length === 0) {
    lines.push(t("dashboard.sidecarNoDetails"));
  }
  return (
    <div className="max-w-md space-y-0.5 break-all font-mono text-[11.5px]">
      {lines.map((line) => (
        <p key={line}>{line}</p>
      ))}
    </div>
  );
}

/**
 * One line per running sidecar ("Claude Code · PID 1234 · v2.2.1"), plus a
 * restart warning when any of them runs a version other than the app's (see
 * lib/sidecars.ts). Outdated rows get a Stop button behind a confirmation.
 */
function SidecarList({
  sidecars,
  appVersion,
  bundledSidecarPath,
  managedSidecarPid
}: SidecarListProps) {
  const { t } = useTranslation();
  const clientLabel = useClientLabel();
  const stopSidecar = useStopSidecar();
  // The target outlives `stopOpen` so the dialog text stays put while it closes.
  const [stopTarget, setStopTarget] = useState<SidecarHeartbeat | null>(null);
  const [stopOpen, setStopOpen] = useState(false);
  const outdated = outdatedSidecars(sidecars, appVersion);
  const oldVersion = outdatedVersionLabel(outdated);
  const current = normalizeVersion(appVersion);
  const versionLabel = (sidecar: SidecarHeartbeat) =>
    sidecar.version ? `v${normalizeVersion(sidecar.version)}` : t("dashboard.sidecarVersionUnknown");

  return (
    <>
      <ul className="mt-2 space-y-1.5 font-mono text-[12px] text-ink-muted">
        {sidecars.map((sidecar) => {
          const label = clientLabel(sidecarClient(sidecar, managedSidecarPid));
          const sidecarOutdated = isSidecarOutdated(sidecar, appVersion);
          return (
            <li key={sidecar.pid}>
              <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="cursor-default font-sans text-[12.5px] font-medium text-ink">
                      {label}
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>
                    <SidecarDetails sidecar={sidecar} />
                  </TooltipContent>
                </Tooltip>
                {isDevBuild(sidecar, bundledSidecarPath) ? (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="inline-flex">
                        <Badge variant="muted" className="font-sans">
                          {t("dashboard.devBuildBadge")}
                        </Badge>
                      </span>
                    </TooltipTrigger>
                    <TooltipContent className="max-w-md break-all font-mono">
                      {sidecar.exePath}
                    </TooltipContent>
                  </Tooltip>
                ) : null}
                {sidecarOutdated ? (
                  <Badge variant="warning" className="font-sans">
                    {t("dashboard.sidecarOutdatedBadge")}
                  </Badge>
                ) : null}
                {sidecarOutdated ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    className="ml-auto h-6 px-2 font-sans text-[12px]"
                    aria-label={t("dashboard.stopSidecarAria", { pid: sidecar.pid })}
                    disabled={stopSidecar.isPending}
                    onClick={() => {
                      setStopTarget(sidecar);
                      setStopOpen(true);
                    }}
                  >
                    {t("dashboard.stopSidecar")}
                  </Button>
                ) : null}
              </div>
              <p>
                PID <span className="text-ink">{sidecar.pid}</span>
                {" · "}
                <span className="text-ink">{versionLabel(sidecar)}</span>
                {" · heartbeat "}
                <RelativeTime iso={sidecar.lastSeen} className="text-ink" />
              </p>
            </li>
          );
        })}
      </ul>
      <ConfirmDialog
        open={stopOpen}
        onOpenChange={setStopOpen}
        title={t("dashboard.stopSidecarTitle")}
        description={
          stopTarget
            ? t("dashboard.stopSidecarDescription", {
                client: clientLabel(sidecarClient(stopTarget, managedSidecarPid)),
                pid: stopTarget.pid,
                version: versionLabel(stopTarget)
              })
            : ""
        }
        confirmLabel={t("dashboard.stopSidecar")}
        isPending={stopSidecar.isPending}
        onConfirm={() => {
          if (stopTarget) {
            stopSidecar.mutate(stopTarget.pid, { onSettled: () => setStopOpen(false) });
          }
        }}
      />
      {outdated.length > 0 ? (
        <p role="status" className="mt-2 text-[12.5px] leading-5 text-warning">
          {oldVersion
            ? t("dashboard.sidecarOutdated", { old: oldVersion, current })
            : t("dashboard.sidecarOutdatedUnknown", { current })}
        </p>
      ) : null}
    </>
  );
}

/** Quits and reopens Claude Desktop so it spawns the current sidecar. */
function RestartClaudeDesktopButton() {
  const { t } = useTranslation();
  const restart = useRestartClaudeDesktop();
  const [confirmOpen, setConfirmOpen] = useState(false);

  return (
    <>
      <Button
        size="sm"
        variant="secondary"
        className="ml-auto h-7 px-2.5 text-[12px]"
        disabled={restart.isPending}
        onClick={() => setConfirmOpen(true)}
      >
        {restart.isPending
          ? t("dashboard.restartingClaudeDesktop")
          : t("dashboard.restartClaudeDesktop")}
      </Button>
      <ConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        title={t("dashboard.restartClaudeDesktopTitle")}
        description={t("dashboard.restartClaudeDesktopDescription")}
        confirmLabel={t("dashboard.restartClaudeDesktopConfirm")}
        isPending={restart.isPending}
        onConfirm={() => {
          restart.mutate(undefined, { onSettled: () => setConfirmOpen(false) });
        }}
      />
    </>
  );
}

function McpStatCard() {
  const { data: status, isPending } = useAppStatus();
  const { t } = useTranslation();

  if (isPending || !status) {
    return (
      <StatCard label={t("dashboard.mcpServer")} className={MCP_CARD_SPAN}>
        <Skeleton className="h-16" />
      </StatCard>
    );
  }

  const statusLabel = status.mcpRunning ? t("common.running") : t("common.offline");

  return (
    <StatCard label={t("dashboard.mcpServer")} className={MCP_CARD_SPAN}>
      <div className="flex items-center gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="inline-flex">
              <StatusDot status={status.mcpRunning ? "live" : "idle"} />
            </span>
          </TooltipTrigger>
          <TooltipContent>{statusLabel}</TooltipContent>
        </Tooltip>
        <span className="text-[15px] font-semibold text-ink">{statusLabel}</span>
        {status.claudeDesktopRunning ? <RestartClaudeDesktopButton /> : null}
      </div>
      {status.mcpRunning && status.sidecars.length > 0 ? (
        <SidecarList
          sidecars={status.sidecars}
          appVersion={status.appVersion}
          bundledSidecarPath={status.bundledSidecarPath}
          managedSidecarPid={status.managedSidecarPid}
        />
      ) : status.mcpRunning ? (
        <p className="mt-2 font-mono text-[12px] text-ink-muted">
          PID <span className="text-ink">{status.mcpPid ?? "?"}</span>
          {status.mcpLastSeen ? (
            <>
              {" · heartbeat "}
              <RelativeTime iso={status.mcpLastSeen} className="text-ink" />
            </>
          ) : null}
        </p>
      ) : (
        <p className="mt-2 text-[12.5px] leading-5 text-ink-muted">
          {t("dashboard.mcpOfflineHint")}
        </p>
      )}
    </StatCard>
  );
}

function DatabaseStatCard() {
  const { data: status, isPending } = useAppStatus();
  const { t } = useTranslation();

  return (
    <StatCard
      label={t("dashboard.database")}
      action={
        status ? <CopyButton value={status.dbPath} label={t("dashboard.copyDatabasePath")} /> : undefined
      }
    >
      {isPending || !status ? (
        <Skeleton className="h-16" />
      ) : (
        <>
          <p className="text-[13px] font-medium text-ink">{t("dashboard.sharedSqlite")}</p>
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="mt-1 truncate font-mono text-[12px] text-ink-muted">{status.dbPath}</p>
            </TooltipTrigger>
            <TooltipContent className="max-w-md break-all font-mono">{status.dbPath}</TooltipContent>
          </Tooltip>
          <p className="mt-2 text-[12.5px] text-ink-muted">
            {t("dashboard.version")} <span className="font-mono text-ink">{status.appVersion}</span>
          </p>
        </>
      )}
    </StatCard>
  );
}

function TokenVaultStatCard() {
  const { data: tokens, isPending } = useTokenStatus();
  const { t } = useTranslation();
  const rows = [{ label: t("dashboard.googleSheets"), stored: tokens?.googleSheets ?? false }];

  return (
    <StatCard label={t("dashboard.tokenVault")}>
      {isPending || !tokens ? (
        <Skeleton className="h-16" />
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.label} className="flex items-center justify-between gap-3">
              <span className="text-[12.5px] text-ink">{row.label}</span>
              <Badge variant={row.stored ? "success" : "muted"}>
                {row.stored ? t("dashboard.inKeychain") : t("dashboard.notStored")}
              </Badge>
            </div>
          ))}
          <p className="pt-1 text-[12px] leading-4 text-ink-muted">
            {t("dashboard.tokensNeverLeave")}
          </p>
        </div>
      )}
    </StatCard>
  );
}

/** Nudges first-run users toward connecting Google Sheets. */
function ConnectSourceCallout({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: sources, isPending } = useSources();
  const { t } = useTranslation();
  if (isPending || (sources ?? []).length > 0) {
    return null;
  }

  return (
    <section className="mb-4 flex flex-wrap items-center justify-between gap-x-6 gap-y-3 rounded-card border border-accent/30 bg-accent/[0.06] px-5 py-4">
      <div className="min-w-0">
        <p className="text-[13px] font-semibold text-ink">{t("dashboard.noSourcesTitle")}</p>
        <p className="mt-0.5 text-[12.5px] text-ink-muted">
          {t("dashboard.noSourcesDescription")}
        </p>
      </div>
      <Button size="sm" onClick={() => onNavigate("sources")}>
        {t("dashboard.connectDataSource")}
      </Button>
    </section>
  );
}

function ListEmpty({ message }: { message: string }) {
  return <p className="py-6 text-center text-[13px] text-ink-muted">{message}</p>;
}

function RecentActivityCard() {
  const { data, isPending } = useAuditEvents();
  const { t } = useTranslation();
  const events = (data?.pages[0] ?? []).slice(0, DASHBOARD_AUDIT_COUNT);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("dashboard.recentActivity")}</CardTitle>
      </CardHeader>
      <CardContent className="py-1">
        {isPending ? (
          <Skeleton className="my-3 h-40" />
        ) : events.length === 0 ? (
          <ListEmpty message={t("dashboard.recentActivityEmpty")} />
        ) : (
          <ol className="divide-y divide-edge">
            {events.map((event) => (
              <li key={event.id} className="flex h-9 items-center gap-2.5">
                <Badge variant={ACTOR_VARIANTS[event.actor]}>{event.actor}</Badge>
                <span className="truncate text-[13px] text-ink">{event.action}</span>
                <RelativeTime iso={event.timestamp} className="ml-auto font-mono text-[11px] text-ink-muted" />
              </li>
            ))}
          </ol>
        )}
      </CardContent>
    </Card>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { t } = useTranslation();
  return (
    <>
      <ScreenHeader
        title={t("screen.dashboard.title")}
        description={t("screen.dashboard.description")}
      />
      <ConnectSourceCallout onNavigate={onNavigate} />
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <McpStatCard />
        <DatabaseStatCard />
        <TokenVaultStatCard />
      </div>
      <div className="mt-4">
        <RecentActivityCard />
      </div>
    </>
  );
}
