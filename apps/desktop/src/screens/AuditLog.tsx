import { ChevronDown, ChevronRight, ScrollText } from "lucide-react";
import { useState } from "react";
import { Badge, Button, Card, EmptyState, Skeleton } from "@sheet-port/ui";
import type { AuditEvent } from "@sheet-port/shared";
import { useAuditEvents } from "../hooks/useAuditEvents.js";
import { RelativeTime } from "../components/RelativeTime.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const EMPTY_STATE_ICON_SIZE = 22;
const EXPAND_ICON_SIZE = 13;

const ACTOR_VARIANTS = { agent: "info", user: "success", system: "muted" } as const;

function AuditRow({ event }: { event: AuditEvent }) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hasMetadata = event.metadata !== undefined && Object.keys(event.metadata).length > 0;
  const target = [event.sourceId, event.tableId].filter(Boolean).join("/");

  return (
    <li className="border-t border-edge first:border-t-0">
      <div className="flex items-center gap-3 px-4 py-2.5">
        <button
          type="button"
          aria-label={isExpanded ? "Collapse metadata" : "Expand metadata"}
          aria-expanded={isExpanded}
          disabled={!hasMetadata}
          onClick={() => setIsExpanded((current) => !current)}
          className="rounded p-0.5 text-ink-muted transition-colors enabled:hover:bg-raised enabled:hover:text-ink disabled:opacity-30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60"
        >
          {isExpanded ? (
            <ChevronDown size={EXPAND_ICON_SIZE} aria-hidden />
          ) : (
            <ChevronRight size={EXPAND_ICON_SIZE} aria-hidden />
          )}
        </button>
        <RelativeTime iso={event.timestamp} className="w-20 shrink-0 font-mono text-xs text-ink-muted" />
        <Badge variant={ACTOR_VARIANTS[event.actor]}>{event.actor}</Badge>
        <span className="truncate text-[13px] font-medium text-ink">{event.action}</span>
        {target !== "" ? (
          <span className="ml-auto truncate font-mono text-xs text-ink-muted">{target}</span>
        ) : null}
      </div>
      {isExpanded && hasMetadata ? (
        <pre className="mx-4 mb-3 overflow-x-auto rounded-md border border-edge bg-bg p-3 font-mono text-[11px] leading-4 text-ink-muted">
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
        description="Every read, preview, decision, and commit, in order."
      />

      {isPending ? (
        <div className="space-y-3">
          <Skeleton className="h-10" />
          <Skeleton className="h-10" />
          <Skeleton className="h-10" />
        </div>
      ) : events.length === 0 ? (
        <EmptyState
          icon={<ScrollText size={EMPTY_STATE_ICON_SIZE} aria-hidden />}
          title="No audit events"
          description="Agent activity is recorded here as soon as it happens."
        />
      ) : (
        <>
          <Card className="overflow-hidden">
            <ol>
              {events.map((event) => (
                <AuditRow key={event.id} event={event} />
              ))}
            </ol>
          </Card>
          {hasNextPage ? (
            <div className="mt-4 flex justify-center">
              <Button
                variant="outline"
                size="sm"
                disabled={isFetchingNextPage}
                onClick={() => void fetchNextPage()}
              >
                {isFetchingNextPage ? "Loading..." : "Load more"}
              </Button>
            </div>
          ) : null}
        </>
      )}
    </>
  );
}
