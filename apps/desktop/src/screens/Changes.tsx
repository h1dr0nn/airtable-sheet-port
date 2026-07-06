import { useState } from "react";
import { EmptyState, Skeleton } from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { useChanges } from "../hooks/useChanges.js";
import { ChangeCard } from "../components/changes/ChangeCard.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import { SegmentedControl, type SegmentedOption } from "../components/SegmentedControl.js";

type FilterValue = ChangeStatus | "all";

const FILTERS: ReadonlyArray<SegmentedOption<FilterValue>> = [
  { value: "all", label: "All" },
  { value: "pending", label: "Pending" },
  { value: "approved", label: "Approved" },
  { value: "committed", label: "Committed" },
  { value: "rejected", label: "Rejected" }
];

export function Changes() {
  const [filter, setFilter] = useState<FilterValue>("all");
  const { data: changes, isPending } = useChanges(filter === "all" ? null : filter);
  const list = changes ?? [];

  return (
    <>
      <ScreenHeader
        title="Changes"
        description="Every agent write lands here as a preview before it can commit"
        actions={
          <SegmentedControl
            options={FILTERS}
            value={filter}
            onChange={setFilter}
            ariaLabel="Filter changes by status"
          />
        }
      />

      {isPending ? (
        <div className="space-y-4">
          <Skeleton className="h-44 rounded-card" />
          <Skeleton className="h-44 rounded-card" />
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          title={filter === "all" ? "No changes yet" : `No ${filter} changes`}
          description="When an agent previews a write it appears here for review"
        />
      ) : (
        <div className="space-y-4">
          {list.map((change) => (
            <ChangeCard key={change.id} change={change} />
          ))}
        </div>
      )}
    </>
  );
}
