import type { ReactNode } from "react";
import { Badge, Skeleton, StatusDot, cn, type BadgeVariant } from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { useAuditEvents } from "../hooks/useAuditEvents.js";
import { useChanges } from "../hooks/useChanges.js";
import { useTokenStatus } from "../hooks/useTokenStatus.js";
import {
  CLAUDE_DESKTOP_CONFIG_HINT,
  DASHBOARD_AUDIT_COUNT,
  DASHBOARD_CHANGES_COUNT
} from "../lib/constants.js";
import type { ScreenId } from "../lib/nav.js";
import { CopyButton } from "../components/CopyButton.js";
import { RelativeTime } from "../components/RelativeTime.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const ACTOR_VARIANTS: Record<"agent" | "user" | "system", BadgeVariant> = {
  agent: "default",
  user: "strong",
  system: "muted"
};

const CHANGE_STATUS_VARIANTS: Record<ChangeStatus, BadgeVariant> = {
  pending: "default",
  approved: "strong",
  committed: "strong",
  rejected: "danger"
};

/** Dense telemetry compartment with a "[ LABEL ]" header strip. */
function Panel({ label, right, children }: { label: string; right?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col bg-surface">
      <header className="flex items-center justify-between gap-3 border-b border-edge px-4 py-2">
        <h3 className="font-mono text-[10px] font-bold uppercase tracking-[0.1em] text-ink-muted">
          {"[ "}
          {label}
          {" ]"}
        </h3>
        {right}
      </header>
      <div className="flex-1 px-4 py-3">{children}</div>
    </section>
  );
}

function PendingHero({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: status, isPending } = useAppStatus();
  const count = status?.pendingCount ?? 0;

  return (
    <section className="flex flex-col justify-between gap-10 bg-surface p-8 lg:col-span-2 lg:row-span-3">
      <p className="font-mono text-[10px] uppercase tracking-[0.15em] text-ink-muted">
        /// Approval queue
      </p>
      {isPending || !status ? (
        <Skeleton className="h-32 w-48" />
      ) : (
        <div>
          <p
            className={cn(
              "font-display text-[clamp(4rem,10vw,9rem)] leading-none tabular-nums",
              count > 0 ? "text-hazard" : "text-ink"
            )}
          >
            {count}
          </p>
          <p className="mt-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-ink-muted">
            [ Pending approvals ]
          </p>
        </div>
      )}
      <button
        type="button"
        onClick={() => onNavigate("changes")}
        className={cn(
          "self-start font-mono text-[11px] font-bold uppercase tracking-[0.08em] text-ink transition-colors",
          "hover:text-hazard focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-hazard"
        )}
      >
        {">>> Review changes"}
      </button>
    </section>
  );
}

function McpPanel() {
  const { data: status, isPending } = useAppStatus();

  return (
    <Panel
      label="MCP Server"
      right={status ? <StatusDot status={status.mcpRunning ? "live" : "idle"} /> : undefined}
    >
      {isPending || !status ? (
        <Skeleton className="h-16" />
      ) : status.mcpRunning ? (
        <>
          <p className="font-mono text-lg font-bold uppercase tracking-[0.05em] text-signal">Running</p>
          <p className="mt-1 font-mono text-[11px] uppercase tracking-[0.05em] text-ink-muted">
            PID <span className="text-ink">{status.mcpPid ?? "?"}</span>
            {status.mcpLastSeen ? (
              <>
                {" / HB "}
                <RelativeTime iso={status.mcpLastSeen} className="text-ink" />
              </>
            ) : null}
          </p>
        </>
      ) : (
        <>
          <p className="font-mono text-lg font-bold uppercase tracking-[0.05em] text-ink-muted">Offline</p>
          <div className="mt-2 flex items-start justify-between gap-1">
            <pre className="flex-1 overflow-x-auto border border-edge bg-bg p-2 font-mono text-[10px] leading-4 text-ink-muted">
              {CLAUDE_DESKTOP_CONFIG_HINT}
            </pre>
            <CopyButton value={CLAUDE_DESKTOP_CONFIG_HINT} label="Copy Claude Desktop config" />
          </div>
          <p className="mt-1.5 font-mono text-[10px] uppercase tracking-[0.05em] text-ink-muted">
            Add to claude_desktop_config.json, then restart Claude Desktop
          </p>
        </>
      )}
    </Panel>
  );
}

function DatabasePanel() {
  const { data: status, isPending } = useAppStatus();

  return (
    <Panel
      label="Database"
      right={status ? <CopyButton value={status.dbPath} label="Copy database path" /> : undefined}
    >
      {isPending || !status ? (
        <Skeleton className="h-16" />
      ) : (
        <>
          <p className="font-mono text-[13px] font-bold uppercase tracking-[0.05em] text-ink">
            Shared SQLite
          </p>
          <p className="mt-1 break-all font-mono text-[11px] leading-4 text-ink-muted" title={status.dbPath}>
            {status.dbPath}
          </p>
          <p className="mt-1.5 font-mono text-[10px] uppercase tracking-[0.05em] text-ink-muted">
            Rev <span className="text-ink">{status.appVersion}</span>
          </p>
        </>
      )}
    </Panel>
  );
}

function TokenVaultPanel() {
  const { data: tokens, isPending } = useTokenStatus();
  const rows = [
    { label: "GOOGLE_SHEETS", stored: tokens?.googleSheets ?? false },
    { label: "PROVIDER", stored: tokens?.provider ?? false }
  ];

  return (
    <Panel label="Token Vault">
      {isPending || !tokens ? (
        <Skeleton className="h-16" />
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.label} className="flex items-center justify-between gap-3">
              <span className="font-mono text-[11px] tracking-[0.05em] text-ink">{row.label}</span>
              <Badge variant={row.stored ? "strong" : "muted"}>
                {row.stored ? "In Keychain" : "Not Stored"}
              </Badge>
            </div>
          ))}
          <p className="pt-1 font-mono text-[10px] uppercase tracking-[0.05em] text-ink-muted">
            Tokens never leave the OS keychain
          </p>
        </div>
      )}
    </Panel>
  );
}

function RecentAudit() {
  const { data, isPending } = useAuditEvents();
  const events = (data?.pages[0] ?? []).slice(0, DASHBOARD_AUDIT_COUNT);

  return (
    <Panel label="Audit Log">
      {isPending ? (
        <Skeleton className="h-40" />
      ) : events.length === 0 ? (
        <p className="py-6 text-center font-mono text-[11px] uppercase tracking-[0.08em] text-ink-muted">
          [ No records ]
        </p>
      ) : (
        <ol>
          {events.map((event) => (
            <li key={event.id} className="flex h-8 items-center gap-2.5 border-t border-edge first:border-t-0">
              <Badge variant={ACTOR_VARIANTS[event.actor]}>{event.actor}</Badge>
              <span className="truncate font-mono text-xs text-ink">{event.action}</span>
              <RelativeTime iso={event.timestamp} className="ml-auto font-mono text-[11px] text-ink-muted" />
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}

function RecentChanges({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: changes, isPending } = useChanges(null);
  const recent = (changes ?? []).slice(0, DASHBOARD_CHANGES_COUNT);

  return (
    <Panel
      label="Changes"
      right={
        <button
          type="button"
          onClick={() => onNavigate("changes")}
          className={cn(
            "font-mono text-[10px] font-bold uppercase tracking-[0.08em] text-ink-muted transition-colors",
            "hover:text-ink focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-hazard"
          )}
        >
          {">>> View all"}
        </button>
      }
    >
      {isPending ? (
        <Skeleton className="h-40" />
      ) : recent.length === 0 ? (
        <p className="py-6 text-center font-mono text-[11px] uppercase tracking-[0.08em] text-ink-muted">
          [ No records ]
        </p>
      ) : (
        <ol>
          {recent.map((change) => (
            <li key={change.id} className="flex h-8 items-center gap-2.5 border-t border-edge first:border-t-0">
              <Badge variant={CHANGE_STATUS_VARIANTS[change.status]}>{change.status}</Badge>
              <span className="truncate font-mono text-xs text-ink-muted">
                {change.type} / {change.sourceId}/{change.tableId}
              </span>
              <RelativeTime iso={change.createdAt} className="ml-auto font-mono text-[11px] text-ink-muted" />
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: status } = useAppStatus();
  const meta = status ? `PENDING ${status.pendingCount} / REV ${status.appVersion}` : "SYS / BOOT";

  return (
    <>
      <ScreenHeader
        title="Dashboard"
        description="Local capability broker between agents and your spreadsheets"
        meta={meta}
      />
      <div className="grid gap-px border border-edge bg-edge lg:grid-cols-3">
        <PendingHero onNavigate={onNavigate} />
        <McpPanel />
        <DatabasePanel />
        <TokenVaultPanel />
      </div>
      <div className="mt-6 grid gap-px border border-edge bg-edge xl:grid-cols-2">
        <RecentAudit />
        <RecentChanges onNavigate={onNavigate} />
      </div>
    </>
  );
}
