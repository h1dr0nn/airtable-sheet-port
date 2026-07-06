import type { ReactNode } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  cn,
  FOCUS_RING,
  Skeleton,
  StatusDot,
  type BadgeVariant
} from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { useAuditEvents } from "../hooks/useAuditEvents.js";
import { useChanges } from "../hooks/useChanges.js";
import { useSources } from "../hooks/useSources.js";
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
  agent: "accent",
  user: "default",
  system: "muted"
};

const CHANGE_STATUS_VARIANTS: Record<ChangeStatus, BadgeVariant> = {
  pending: "warning",
  approved: "default",
  committed: "success",
  rejected: "danger"
};

type StatCardProps = {
  label: string;
  action?: ReactNode;
  children: ReactNode;
};

function StatCard({ label, action, children }: StatCardProps) {
  return (
    <section className="flex flex-col rounded-card border border-edge bg-raised p-5 shadow-card">
      <div className="flex items-center justify-between gap-2">
        <p className="overline-label">{label}</p>
        {action}
      </div>
      <div className="mt-3 flex-1">{children}</div>
    </section>
  );
}

function McpStatCard() {
  const { data: status, isPending } = useAppStatus();

  if (isPending || !status) {
    return (
      <StatCard label="MCP Server">
        <Skeleton className="h-16" />
      </StatCard>
    );
  }

  return (
    <StatCard label="MCP Server">
      <div className="flex items-center gap-2">
        <StatusDot status={status.mcpRunning ? "live" : "idle"} />
        <span className="text-[15px] font-semibold text-ink">
          {status.mcpRunning ? "Running" : "Offline"}
        </span>
      </div>
      {status.mcpRunning ? (
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
        <>
          <p className="mt-2 text-[12.5px] leading-5 text-ink-muted">
            Add sheet-port to claude_desktop_config.json, then restart Claude Desktop.
          </p>
          <div className="mt-2 -ml-2">
            <CopyButton value={CLAUDE_DESKTOP_CONFIG_HINT} label="Copy Claude Desktop config">
              Copy Config
            </CopyButton>
          </div>
        </>
      )}
    </StatCard>
  );
}

function PendingStatCard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: status, isPending } = useAppStatus();
  const count = status?.pendingCount ?? 0;

  return (
    <StatCard label="Pending Approvals">
      {isPending || !status ? (
        <Skeleton className="h-16" />
      ) : (
        <>
          <p
            className={cn(
              "text-[28px] font-semibold leading-none tabular-nums",
              count > 0 ? "text-warning" : "text-ink"
            )}
          >
            {count}
          </p>
          <p className="mt-1.5 text-[12.5px] text-ink-muted">
            {count === 0 ? "Nothing waiting on you" : count === 1 ? "Change awaiting review" : "Changes awaiting review"}
          </p>
          <Button variant="secondary" size="sm" className="mt-3" onClick={() => onNavigate("changes")}>
            Review Changes
          </Button>
        </>
      )}
    </StatCard>
  );
}

function DatabaseStatCard() {
  const { data: status, isPending } = useAppStatus();

  return (
    <StatCard
      label="Database"
      action={status ? <CopyButton value={status.dbPath} label="Copy database path" /> : undefined}
    >
      {isPending || !status ? (
        <Skeleton className="h-16" />
      ) : (
        <>
          <p className="text-[13px] font-medium text-ink">Shared SQLite</p>
          <p className="mt-1 truncate font-mono text-[12px] text-ink-muted" title={status.dbPath}>
            {status.dbPath}
          </p>
          <p className="mt-2 text-[12.5px] text-ink-muted">
            Version <span className="font-mono text-ink">{status.appVersion}</span>
          </p>
        </>
      )}
    </StatCard>
  );
}

function TokenVaultStatCard() {
  const { data: tokens, isPending } = useTokenStatus();
  const rows = [
    { label: "Google Sheets", stored: tokens?.googleSheets ?? false },
    { label: "Provider", stored: tokens?.provider ?? false }
  ];

  return (
    <StatCard label="Token Vault">
      {isPending || !tokens ? (
        <Skeleton className="h-16" />
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.label} className="flex items-center justify-between gap-3">
              <span className="text-[12.5px] text-ink">{row.label}</span>
              <Badge variant={row.stored ? "success" : "muted"}>
                {row.stored ? "In Keychain" : "Not Stored"}
              </Badge>
            </div>
          ))}
          <p className="pt-1 text-[12px] leading-4 text-ink-muted">
            Tokens never leave the OS keychain.
          </p>
        </div>
      )}
    </StatCard>
  );
}

/** Nudges first-run users toward connecting Google Sheets. */
function ConnectSourceCallout({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: sources, isPending } = useSources();
  if (isPending || (sources ?? []).length > 0) {
    return null;
  }

  return (
    <section className="mb-4 flex flex-wrap items-center justify-between gap-x-6 gap-y-3 rounded-card border border-accent/30 bg-accent/[0.06] px-5 py-4">
      <div className="min-w-0">
        <p className="text-[13px] font-semibold text-ink">No Data Sources Connected</p>
        <p className="mt-0.5 text-[12.5px] text-ink-muted">
          Connect a data source such as Google Sheets to give agents something to read.
        </p>
      </div>
      <Button size="sm" onClick={() => onNavigate("sources")}>
        Connect a Data Source
      </Button>
    </section>
  );
}

function ListEmpty({ message }: { message: string }) {
  return <p className="py-6 text-center text-[13px] text-ink-muted">{message}</p>;
}

function RecentActivityCard() {
  const { data, isPending } = useAuditEvents();
  const events = (data?.pages[0] ?? []).slice(0, DASHBOARD_AUDIT_COUNT);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent Activity</CardTitle>
      </CardHeader>
      <CardContent className="py-1">
        {isPending ? (
          <Skeleton className="my-3 h-40" />
        ) : events.length === 0 ? (
          <ListEmpty message="Agent activity shows up here as it happens." />
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

function RecentChangesCard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: changes, isPending } = useChanges(null);
  const recent = (changes ?? []).slice(0, DASHBOARD_CHANGES_COUNT);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent Changes</CardTitle>
        <button
          type="button"
          onClick={() => onNavigate("changes")}
          className={cn(
            "rounded text-[12px] font-medium text-accent transition-colors hover:text-accent-hover",
            FOCUS_RING
          )}
        >
          View All
        </button>
      </CardHeader>
      <CardContent className="py-1">
        {isPending ? (
          <Skeleton className="my-3 h-40" />
        ) : recent.length === 0 ? (
          <ListEmpty message="Agent write previews land here for review." />
        ) : (
          <ol className="divide-y divide-edge">
            {recent.map((change) => (
              <li key={change.id} className="flex h-9 items-center gap-2.5">
                <Badge variant={CHANGE_STATUS_VARIANTS[change.status]}>{change.status}</Badge>
                <span className="truncate text-[13px] text-ink-muted">
                  {change.type}{" "}
                  <span className="font-mono text-[12px]">
                    {change.sourceId}/{change.tableId}
                  </span>
                </span>
                <RelativeTime iso={change.createdAt} className="ml-auto font-mono text-[11px] text-ink-muted" />
              </li>
            ))}
          </ol>
        )}
      </CardContent>
    </Card>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  return (
    <>
      <ScreenHeader
        title="Dashboard"
        description="Local capability broker between agents and your spreadsheets"
      />
      <ConnectSourceCallout onNavigate={onNavigate} />
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <McpStatCard />
        <PendingStatCard onNavigate={onNavigate} />
        <DatabaseStatCard />
        <TokenVaultStatCard />
      </div>
      <div className="mt-4 grid gap-4 xl:grid-cols-2">
        <RecentActivityCard />
        <RecentChangesCard onNavigate={onNavigate} />
      </div>
    </>
  );
}
