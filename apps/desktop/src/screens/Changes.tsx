import { GitPullRequestArrow } from "lucide-react";
import { useState } from "react";
import { EmptyState, Skeleton, cn } from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { useChanges } from "../hooks/useChanges.js";
import { ChangeCard } from "../components/changes/ChangeCard.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const EMPTY_STATE_ICON_SIZE = 22;

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
      className="inline-flex rounded-md border border-edge bg-surface p-0.5"
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
              "rounded px-3 py-1 text-xs font-medium transition-colors",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60",
              isActive ? "bg-raised text-ink shadow-card" : "text-ink-muted hover:text-ink"
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

  return (
    <>
      <ScreenHeader
        title="Changes"
        description="Every agent write lands here as a preview before it can commit."
        actions={<FilterControl active={filter} onChange={setFilter} />}
      />

      {isPending ? (
        <div className="space-y-3">
          <Skeleton className="h-44" />
          <Skeleton className="h-44" />
        </div>
      ) : (changes ?? []).length === 0 ? (
        <EmptyState
          icon={<GitPullRequestArrow size={EMPTY_STATE_ICON_SIZE} aria-hidden />}
          title={filter === null ? "No changes yet" : `No ${filter} changes`}
          description="When an agent previews a write it appears here for review."
        />
      ) : (
        <div className="space-y-3">
          {(changes ?? []).map((change) => (
            <ChangeCard key={change.id} change={change} />
          ))}
        </div>
      )}
    </>
  );
}
