import {
  Badge,
  Button,
  EmptyState,
  Skeleton,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import type { DataSource, SourceStatus } from "@sheet-port/shared";
import { useSources } from "../hooks/useSources.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const STATUS_VARIANTS: Record<SourceStatus, BadgeVariant> = {
  connected: "strong",
  placeholder: "muted",
  error: "danger"
};

const STATUS_LABELS: Record<SourceStatus, string> = {
  connected: "OK / Connected",
  placeholder: "Placeholder",
  error: "Error"
};

const PLACEHOLDER_TOOLTIP = "OAuth arrives with the real connector";

function SourceCard({ source }: { source: DataSource }) {
  const status = source.status ?? "placeholder";

  return (
    <article className="flex flex-col border border-edge bg-surface">
      <header className="flex items-center justify-between gap-3 border-b border-edge px-4 py-2">
        <h3 className="font-mono text-[10px] font-bold uppercase tracking-[0.1em] text-ink-muted">
          {"[ "}
          {source.kind}
          {" ]"}
        </h3>
        <Badge variant={STATUS_VARIANTS[status]}>{STATUS_LABELS[status]}</Badge>
      </header>
      <div className="flex-1 px-4 py-3">
        <p className="font-mono text-sm font-bold text-ink">{source.name}</p>
        <p className="mt-1.5 font-mono text-[11px] uppercase leading-4 tracking-[0.05em] text-ink-muted">
          {status === "connected"
            ? "Available to agents through permission rules"
            : "Connector scaffolded; authentication is not wired up yet"}
        </p>
      </div>
      <footer className="px-4 pb-3">
        {status === "connected" ? (
          <Button variant="secondary" size="sm" disabled>
            Connected
          </Button>
        ) : (
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Disabled buttons swallow pointer events; the span keeps the tooltip alive. */}
              <span className="inline-block">
                <Button variant="outline" size="sm" disabled>
                  {">>> Connect"}
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>{PLACEHOLDER_TOOLTIP}</TooltipContent>
          </Tooltip>
        )}
      </footer>
    </article>
  );
}

export function DataSources() {
  const { data: sources, isPending } = useSources();
  const list = sources ?? [];

  return (
    <>
      <ScreenHeader
        title="Data Sources"
        description="Connected table providers and placeholders waiting on OAuth"
        meta={isPending ? "SRC / SCAN" : `SRC ${list.length}`}
      />
      {isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-40" />
          <Skeleton className="h-40" />
          <Skeleton className="h-40" />
        </div>
      ) : list.length === 0 ? (
        <EmptyState title="No sources" description="No table providers are configured yet" />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {list.map((source) => (
            <SourceCard key={source.id} source={source} />
          ))}
        </div>
      )}
    </>
  );
}
