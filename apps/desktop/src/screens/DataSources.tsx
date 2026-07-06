import { Database, FileSpreadsheet, FlaskConical, Plug, type LucideIcon } from "lucide-react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Skeleton,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import type { DataSource, DataSourceKind, SourceStatus } from "@sheet-port/shared";
import { useSources } from "../hooks/useSources.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const KIND_ICONS: Record<DataSourceKind, LucideIcon> = {
  google_sheets: FileSpreadsheet,
  provider: Plug,
  mock: FlaskConical
};

const STATUS_VARIANTS: Record<SourceStatus, BadgeVariant> = {
  connected: "success",
  placeholder: "muted",
  error: "danger"
};

const PLACEHOLDER_TOOLTIP = "OAuth arrives with the real connector";

function SourceCard({ source }: { source: DataSource }) {
  const Icon = KIND_ICONS[source.kind];
  const status = source.status ?? "placeholder";

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2.5">
          <span className="flex h-8 w-8 items-center justify-center rounded-md bg-raised text-ink-muted">
            <Icon size={15} aria-hidden />
          </span>
          <div>
            <p className="text-sm font-semibold text-ink">{source.name}</p>
            <CardTitle className="mt-0.5 font-mono normal-case tracking-normal">{source.kind}</CardTitle>
          </div>
        </div>
        <Badge variant={STATUS_VARIANTS[status]}>{status}</Badge>
      </CardHeader>
      <CardContent>
        <p className="text-[13px] text-ink-muted">
          {status === "connected"
            ? "Available to agents through permission rules."
            : "Connector scaffolded; authentication is not wired up yet."}
        </p>
        {status === "connected" ? (
          <Button variant="secondary" size="sm" className="mt-3" disabled>
            Connected
          </Button>
        ) : (
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Disabled buttons swallow pointer events; the span keeps the tooltip alive. */}
              <span className="mt-3 inline-block">
                <Button variant="outline" size="sm" disabled>
                  Connect
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>{PLACEHOLDER_TOOLTIP}</TooltipContent>
          </Tooltip>
        )}
      </CardContent>
    </Card>
  );
}

export function DataSources() {
  const { data: sources, isPending } = useSources();

  return (
    <>
      <ScreenHeader
        title="Data Sources"
        description="Connected table providers and placeholders waiting on OAuth."
      />
      {isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-40" />
          <Skeleton className="h-40" />
          <Skeleton className="h-40" />
        </div>
      ) : (sources ?? []).length === 0 ? (
        <Card className="p-8 text-center">
          <Database size={20} className="mx-auto text-ink-muted" aria-hidden />
          <p className="mt-2 text-sm text-ink-muted">No sources configured</p>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {(sources ?? []).map((source) => (
            <SourceCard key={source.id} source={source} />
          ))}
        </div>
      )}
    </>
  );
}
