import { ArrowRight } from "lucide-react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Skeleton,
  StatusDot,
  cn
} from "@sheet-port/ui";
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

const ACTOR_VARIANTS = { agent: "info", user: "success", system: "muted" } as const;

function McpServerCard() {
  const { data: status, isPending } = useAppStatus();
  if (isPending || !status) {
    return <Skeleton className="h-36" />;
  }
  return (
    <Card>
      <CardHeader>
        <CardTitle>MCP server</CardTitle>
        <StatusDot status={status.mcpRunning ? "live" : "idle"} />
      </CardHeader>
      <CardContent>
        <p className={cn("text-lg font-semibold", status.mcpRunning ? "text-accent" : "text-ink-muted")}>
          {status.mcpRunning ? "Running" : "Stopped"}
        </p>
        {status.mcpRunning ? (
          <p className="mt-1 text-xs text-ink-muted">
            pid <span className="font-mono text-ink">{status.mcpPid ?? "?"}</span>
            {status.mcpLastSeen ? (
              <>
                {" · heartbeat "}
                <RelativeTime iso={status.mcpLastSeen} />
              </>
            ) : null}
          </p>
        ) : (
          <div className="mt-2">
            <div className="flex items-start justify-between gap-1">
              <pre className="flex-1 overflow-x-auto rounded-md border border-edge bg-bg p-2 font-mono text-[10px] leading-4 text-ink-muted">
                {CLAUDE_DESKTOP_CONFIG_HINT}
              </pre>
              <CopyButton value={CLAUDE_DESKTOP_CONFIG_HINT} label="Copy Claude Desktop config" />
            </div>
            <p className="mt-1.5 text-[11px] text-ink-muted">
              Add to claude_desktop_config.json, then restart Claude Desktop.
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function DatabaseCard() {
  const { data: status, isPending } = useAppStatus();
  if (isPending || !status) {
    return <Skeleton className="h-36" />;
  }
  return (
    <Card>
      <CardHeader>
        <CardTitle>Database</CardTitle>
        <CopyButton value={status.dbPath} label="Copy database path" />
      </CardHeader>
      <CardContent>
        <p className="text-lg font-semibold text-ink">Shared SQLite</p>
        <p className="mt-1 break-all font-mono text-[11px] leading-4 text-ink-muted" title={status.dbPath}>
          {status.dbPath}
        </p>
        <p className="mt-1.5 text-[11px] text-ink-muted">
          App version <span className="font-mono text-ink">{status.appVersion}</span>
        </p>
      </CardContent>
    </Card>
  );
}

function PendingCard({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: status, isPending } = useAppStatus();
  if (isPending || !status) {
    return <Skeleton className="h-36" />;
  }
  const count = status.pendingCount;
  return (
    <Card>
      <CardHeader>
        <CardTitle>Pending approvals</CardTitle>
        {count > 0 ? <StatusDot status="warning" /> : null}
      </CardHeader>
      <CardContent>
        <p className={cn("font-mono text-3xl font-semibold tabular-nums", count > 0 ? "text-warning" : "text-ink")}>
          {count}
        </p>
        <Button variant="secondary" size="sm" className="mt-3" onClick={() => onNavigate("changes")}>
          Review changes
          <ArrowRight size={13} aria-hidden />
        </Button>
      </CardContent>
    </Card>
  );
}

function TokensCard() {
  const { data: tokens, isPending } = useTokenStatus();
  if (isPending || !tokens) {
    return <Skeleton className="h-36" />;
  }
  const rows = [
    { label: "Google Sheets", stored: tokens.googleSheets },
    { label: "Provider", stored: tokens.provider }
  ];
  return (
    <Card>
      <CardHeader>
        <CardTitle>Tokens</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2.5">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center justify-between">
            <span className="text-[13px] text-ink">{row.label}</span>
            <Badge variant={row.stored ? "success" : "muted"}>{row.stored ? "in keychain" : "not stored"}</Badge>
          </div>
        ))}
        <p className="pt-1 text-[11px] text-ink-muted">Tokens never leave the OS keychain.</p>
      </CardContent>
    </Card>
  );
}

function RecentAudit() {
  const { data, isPending } = useAuditEvents();
  const events = (data?.pages[0] ?? []).slice(0, DASHBOARD_AUDIT_COUNT);
  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent activity</CardTitle>
      </CardHeader>
      <CardContent>
        {isPending ? (
          <Skeleton className="h-40" />
        ) : events.length === 0 ? (
          <p className="py-6 text-center text-[13px] text-ink-muted">No activity yet</p>
        ) : (
          <ol className="space-y-0">
            {events.map((event) => (
              <li key={event.id} className="flex items-center gap-2.5 border-t border-edge py-2 first:border-t-0">
                <Badge variant={ACTOR_VARIANTS[event.actor]}>{event.actor}</Badge>
                <span className="truncate font-mono text-[13px] text-ink">{event.action}</span>
                <RelativeTime iso={event.timestamp} className="ml-auto text-xs text-ink-muted" />
              </li>
            ))}
          </ol>
        )}
      </CardContent>
    </Card>
  );
}

function RecentChanges({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: changes, isPending } = useChanges(null);
  const recent = (changes ?? []).slice(0, DASHBOARD_CHANGES_COUNT);
  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent changes</CardTitle>
        <Button variant="ghost" size="sm" onClick={() => onNavigate("changes")}>
          View all
        </Button>
      </CardHeader>
      <CardContent>
        {isPending ? (
          <Skeleton className="h-40" />
        ) : recent.length === 0 ? (
          <p className="py-6 text-center text-[13px] text-ink-muted">No changes proposed yet</p>
        ) : (
          <ol>
            {recent.map((change) => (
              <li key={change.id} className="flex items-center gap-2.5 border-t border-edge py-2 first:border-t-0">
                <Badge
                  variant={
                    change.status === "pending" ? "warning" : change.status === "committed" ? "success" : change.status === "rejected" ? "danger" : "info"
                  }
                >
                  {change.status}
                </Badge>
                <span className="truncate font-mono text-[13px] text-ink-muted">
                  {change.type} · {change.sourceId}/{change.tableId}
                </span>
                <RelativeTime iso={change.createdAt} className="ml-auto text-xs text-ink-muted" />
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
        description="Local capability broker between agents and your spreadsheets."
      />
      <div className="grid gap-4 lg:grid-cols-2 xl:grid-cols-4">
        <McpServerCard />
        <DatabaseCard />
        <PendingCard onNavigate={onNavigate} />
        <TokensCard />
      </div>
      <div className="mt-6 grid gap-4 xl:grid-cols-2">
        <RecentAudit />
        <RecentChanges onNavigate={onNavigate} />
      </div>
    </>
  );
}
