import { useState } from "react";
import { EmptyState, Skeleton, cn } from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { useChanges } from "../hooks/useChanges.js";
import { ChangeCard } from "../components/changes/ChangeCard.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

type StatusFilter = ChangeStatus | null;

const FILTERS: ReadonlyArray<{ value: StatusFilter; label: string }> = [
  { value: null, label: "All" },
  { value: "pending", label: "Pending" },
  { value: "approved", label: "Approved" },
  { value: "committed", label: "Committed" },
  { value: "rejected", label: "Rejected" }
];

function FilterControl({
  active,
  onChange
}: {
  active: StatusFilter;
  onChange: (value: StatusFilter) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Filter changes by status"
      className="inline-flex border border-edge bg-bg"
    >
      {FILTERS.map((filter) => {
        const isActive = filter.value === active;
        return (
          <button
            key={filter.label}
            type="button"
            role="radio"
            aria-checked={isActive}
            onClick={() => onChange(filter.value)}
            className={cn(
              "border-r border-edge px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.08em]",
              "transition-colors last:border-r-0",
              "focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-2 focus-visible:outline-hazard",
              isActive ? "bg-ink font-bold text-bg" : "text-ink-muted hover:text-ink"
            )}
          >
            {filter.label}
          </button>
        );
      })}
    </div>
  );
}

export function Changes() {
  const [filter, setFilter] = useState<StatusFilter>(null);
  const { data: changes, isPending } = useChanges(filter);
  const list = changes ?? [];

  return (
    <>
      <ScreenHeader
        title="Changes"
        description="Every agent write lands here as a preview before it can commit"
        meta={isPending ? "CHG / SCAN" : `CHG ${list.length}`}
        actions={<FilterControl active={filter} onChange={setFilter} />}
      />

      {isPending ? (
        <div className="grid gap-px border border-edge bg-edge">
          <Skeleton className="h-44" />
          <Skeleton className="h-44" />
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          title={filter === null ? "No changes" : `No ${filter} changes`}
          description="When an agent previews a write it appears here for review"
        />
      ) : (
        <div className="grid gap-px border border-edge bg-edge">
          {list.map((change) => (
            <ChangeCard key={change.id} change={change} />
          ))}
        </div>
      )}
    </>
  );
}
