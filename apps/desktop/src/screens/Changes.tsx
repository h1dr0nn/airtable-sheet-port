import { useState } from "react";
import { EmptyState, Skeleton } from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { useChanges } from "../hooks/useChanges.js";
import { useTranslation } from "../i18n/useTranslation.js";
import type { TranslationKey } from "../i18n/translations.js";
import { ChangeCard } from "../components/changes/ChangeCard.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import { SegmentedControl, type SegmentedOption } from "../components/SegmentedControl.js";

type FilterValue = ChangeStatus | "all";

// Label keys are resolved through t() at render so filters follow the language.
const FILTER_LABEL_KEYS: Record<FilterValue, TranslationKey> = {
  all: "changes.filterAll",
  pending: "changes.filterPending",
  approved: "changes.filterApproved",
  committed: "changes.filterCommitted",
  rejected: "changes.filterRejected"
};

const FILTER_ORDER: readonly FilterValue[] = [
  "all",
  "pending",
  "approved",
  "committed",
  "rejected"
];

export function Changes() {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<FilterValue>("all");
  const { data: changes, isPending } = useChanges(filter === "all" ? null : filter);
  const list = changes ?? [];

  const filters: ReadonlyArray<SegmentedOption<FilterValue>> = FILTER_ORDER.map((value) => ({
    value,
    label: t(FILTER_LABEL_KEYS[value])
  }));
  const filterLabel = t(FILTER_LABEL_KEYS[filter]);

  return (
    <>
      <ScreenHeader
        title={t("screen.changes.title")}
        description={t("screen.changes.description")}
        actions={
          <SegmentedControl
            options={filters}
            value={filter}
            onChange={setFilter}
            ariaLabel={t("changes.filterAria")}
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
          title={filter === "all" ? t("changes.emptyAll") : t("changes.emptyFiltered", { filter: filterLabel })}
          description={t("changes.emptyDescription")}
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
