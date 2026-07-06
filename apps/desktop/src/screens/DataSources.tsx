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
  connected: "success",
  placeholder: "muted",
  error: "danger"
};

const STATUS_LABELS: Record<SourceStatus, string> = {
  connected: "Connected",
  placeholder: "Placeholder",
  error: "Error"
};

const PLACEHOLDER_TOOLTIP = "OAuth arrives with the real connector";

function SourceCard({ source }: { source: DataSource }) {
  const status = source.status ?? "placeholder";

  return (
    <article className="flex flex-col rounded-card border border-edge bg-raised shadow-card">
      <header className="flex items-center justify-between gap-3 border-b border-edge px-5 py-3">
        <h3 className="overline-label">{source.kind}</h3>
        <Badge variant={STATUS_VARIANTS[status]}>{STATUS_LABELS[status]}</Badge>
      </header>
      <div className="flex-1 px-5 py-4">
        <p className="text-[15px] font-semibold text-ink">{source.name}</p>
        <p className="mt-1 text-[13px] leading-5 text-ink-muted">
          {status === "connected"
            ? "Available to agents through permission rules"
            : "Connector scaffolded; authentication is not wired up yet"}
        </p>
      </div>
      <footer className="px-5 pb-4">
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
                  Connect
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
      />
      {isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-40 rounded-card" />
          <Skeleton className="h-40 rounded-card" />
          <Skeleton className="h-40 rounded-card" />
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
