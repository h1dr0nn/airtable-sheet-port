import { useState } from "react";
import { Badge, Button, Card, cn, EmptyState, FOCUS_RING, Skeleton, type BadgeVariant } from "@sheet-port/ui";
import { ChevronRight } from "lucide-react";
import type { AuditEvent } from "@sheet-port/shared";
import { useAuditEvents } from "../hooks/useAuditEvents.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const ISO_DATE_END = 10;
const ISO_TIME_START = 11;
const ISO_TIME_END = 19;

const ACTOR_VARIANTS: Record<"agent" | "user" | "system", BadgeVariant> = {
  agent: "accent",
  user: "default",
  system: "muted"
};

/** "YYYY-MM-DD HH:MM:SS" readout from an ISO timestamp; raw value if malformed. */
function formatIsoTimestamp(iso: string): string {
  if (iso.length < ISO_TIME_END) {
    return iso;
  }
  return `${iso.slice(0, ISO_DATE_END)} ${iso.slice(ISO_TIME_START, ISO_TIME_END)}`;
}

function AuditRow({ event }: { event: AuditEvent }) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hasMetadata = event.metadata !== undefined && Object.keys(event.metadata).length > 0;
  const target = [event.sourceId, event.tableId].filter(Boolean).join("/");

  return (
    <li>
      <div className="flex h-9 items-center gap-3 px-4">
        <button
          type="button"
          aria-label={isExpanded ? "Collapse metadata" : "Expand metadata"}
          aria-expanded={isExpanded}
          disabled={!hasMetadata}
          onClick={() => setIsExpanded((current) => !current)}
          className={cn(
            "flex h-5 w-5 shrink-0 items-center justify-center rounded text-ink-muted transition-colors",
            "enabled:hover:text-ink disabled:opacity-30",
            FOCUS_RING
          )}
        >
          <ChevronRight
            size={14}
            aria-hidden
            className={cn("transition-transform", isExpanded && "rotate-90")}
          />
        </button>
        <time
          dateTime={event.timestamp}
          className="shrink-0 font-mono text-[12px] tabular-nums text-ink-muted"
        >
          {formatIsoTimestamp(event.timestamp)}
        </time>
        <Badge variant={ACTOR_VARIANTS[event.actor]}>{event.actor}</Badge>
        <span className="truncate text-[13px] text-ink">{event.action}</span>
        {target !== "" ? (
          <span className="ml-auto truncate font-mono text-[12px] text-ink-muted">{target}</span>
        ) : null}
      </div>
      {isExpanded && hasMetadata ? (
        <pre className="mx-4 mb-3 overflow-x-auto rounded-lg border border-edge bg-surface p-3 font-mono text-[12px] leading-5 text-ink-muted">
          {JSON.stringify(event.metadata, null, 2)}
        </pre>
      ) : null}
    </li>
  );
}

export function AuditLog() {
  const { data, isPending, hasNextPage, isFetchingNextPage, fetchNextPage } = useAuditEvents();
  const events = data?.pages.flat() ?? [];

  return (
    <>
      <ScreenHeader
        title="Audit Log"
        description="Every read, preview, decision, and commit, in order"
      />

      {isPending ? (
        <div className="space-y-2">
          <Skeleton className="h-9" />
          <Skeleton className="h-9" />
          <Skeleton className="h-9" />
        </div>
      ) : events.length === 0 ? (
        <EmptyState
          title="No Activity Yet"
          description="Agent activity is recorded here as soon as it happens"
        />
      ) : (
        <>
          <Card className="overflow-hidden">
            <ol className="divide-y divide-edge">
              {events.map((event) => (
                <AuditRow key={event.id} event={event} />
              ))}
            </ol>
          </Card>
          {hasNextPage ? (
            <div className="mt-4 flex justify-center">
              <Button
                variant="secondary"
                size="sm"
                disabled={isFetchingNextPage}
                onClick={() => void fetchNextPage()}
              >
                {isFetchingNextPage ? "Loading..." : "Load More"}
              </Button>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
