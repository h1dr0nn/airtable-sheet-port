import { useEffect, useRef, useState } from "react";
import { ChevronRight } from "lucide-react";
import {
  Badge,
  Button,
  cn,
  EmptyState,
  FOCUS_RING,
  Skeleton,
  type BadgeVariant
} from "@sheet-port/ui";
import type { AuditEvent } from "@sheet-port/shared";
import { useAuditEvents } from "../hooks/useAuditEvents.js";
import { AUDIT_DROPDOWN_PAGE_SIZE } from "../lib/constants.js";
import { formatRelativeTime } from "../lib/format.js";

const ACTOR_VARIANTS: Record<AuditEvent["actor"], BadgeVariant> = {
  agent: "accent",
  user: "default",
  system: "muted"
};

function AuditRow({ event }: { event: AuditEvent }) {
  const [isExpanded, setIsExpanded] = useState(false);
  const hasMetadata = event.metadata !== undefined && Object.keys(event.metadata).length > 0;
  const target = [event.sourceId, event.tableId].filter(Boolean).join("/");

  return (
    <li className="px-3 py-2.5">
      <div className="flex items-start gap-2.5">
        <button
          type="button"
          aria-label={isExpanded ? "Collapse metadata" : "Expand metadata"}
          aria-expanded={isExpanded}
          disabled={!hasMetadata}
          onClick={() => setIsExpanded((current) => !current)}
          className={cn(
            "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded text-ink-muted transition-colors",
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
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Badge variant={ACTOR_VARIANTS[event.actor]}>{event.actor}</Badge>
            <time
              dateTime={event.timestamp}
              className="ml-auto shrink-0 font-mono text-[11px] tabular-nums text-ink-faint"
            >
              {formatRelativeTime(event.timestamp)}
            </time>
          </div>
          <p className="mt-1 truncate text-[13px] text-ink">{event.action}</p>
          {target !== "" ? (
            <p className="mt-0.5 truncate font-mono text-[11px] text-ink-muted">{target}</p>
          ) : null}
          {isExpanded && hasMetadata ? (
            <pre className="mt-2 overflow-x-auto rounded-md border border-edge bg-surface p-2.5 font-mono text-[11px] leading-5 text-ink-muted">
              {JSON.stringify(event.metadata, null, 2)}
            </pre>
          ) : null}
        </div>
      </div>
    </li>
  );
}

type AuditDropdownProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

/** Notification-style activity feed anchored under the titlebar bell button.
 * Controlled by the Titlebar so a single button toggles it. Right-aligned,
 * fixed width, internally scrollable, with offset-based "Load More". */
export function AuditDropdown({ open, onOpenChange }: AuditDropdownProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const { data, isPending, hasNextPage, isFetchingNextPage, fetchNextPage } =
    useAuditEvents(AUDIT_DROPDOWN_PAGE_SIZE);
  const events = data?.pages.flat() ?? [];

  // Dismiss on outside pointer or Escape while open.
  useEffect(() => {
    if (!open) {
      return;
    }
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      // The parent wrapper holds both the trigger button and this panel, so a
      // click on the toggle button counts as "inside" and never double-fires.
      const anchor = panelRef.current?.parentElement ?? panelRef.current;
      if (target && anchor && !anchor.contains(target)) {
        onOpenChange(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onOpenChange(false);
      }
    };
    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, onOpenChange]);

  if (!open) {
    return null;
  }

  return (
    <div
      ref={panelRef}
      role="dialog"
      aria-label="Activity"
      // Rides the dropdown layer (see --z-dropdown) so activity opened from the
      // titlebar is never occluded by an in-flight toast stack.
      style={{ zIndex: "var(--z-dropdown)" }}
      className="absolute right-2 top-full mt-1 flex max-h-[70vh] w-[380px] flex-col overflow-hidden rounded-lg border border-edge bg-raised shadow-pop"
    >
      <div className="flex shrink-0 items-center border-b border-edge px-3 py-2.5">
        <h2 className="text-[13px] font-semibold text-ink">Activity</h2>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {isPending ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        ) : events.length === 0 ? (
          <div className="p-4">
            <EmptyState
              title="No Activity Yet"
              description="Agent activity is recorded here as soon as it happens"
            />
          </div>
        ) : (
          <>
            <ol className="divide-y divide-edge">
              {events.map((event) => (
                <AuditRow key={event.id} event={event} />
              ))}
            </ol>
            {hasNextPage ? (
              <div className="flex justify-center border-t border-edge p-3">
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
      </div>
    </div>
  );
}
