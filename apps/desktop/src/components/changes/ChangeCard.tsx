import {
  Badge,
  Button,
  cn,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import type { ChangeStatus, ChangeType, PendingChange } from "@sheet-port/shared";
import { useApproveChange, useRejectChange } from "../../hooks/useChanges.js";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { TranslationKey } from "../../i18n/translations.js";
import { formatRelativeTime } from "../../lib/format.js";
import { RelativeTime } from "../RelativeTime.js";
import { DiffViewer } from "./DiffViewer.js";

const CHANGE_ID_LENGTH = 8;

const STATUS_VARIANTS: Record<ChangeStatus, BadgeVariant> = {
  pending: "warning",
  approved: "default",
  committed: "success",
  rejected: "danger"
};

const STATUS_LABEL_KEYS: Record<ChangeStatus, TranslationKey> = {
  pending: "changes.statusPending",
  approved: "changes.statusApproved",
  committed: "changes.statusCommitted",
  rejected: "changes.statusRejected"
};

const TYPE_VARIANTS: Record<ChangeType, BadgeVariant> = {
  append: "default",
  update: "default",
  delete: "danger"
};

function ChangeOutcome({ change }: { change: PendingChange }) {
  const { t } = useTranslation();
  const baseClass = "text-[12.5px] text-ink-muted";
  if (change.status === "committed" && change.committedAt) {
    const time = formatRelativeTime(change.committedAt);
    return (
      <p className={baseClass}>
        {change.decidedBy
          ? t("changes.committedBy", { time, who: change.decidedBy })
          : t("changes.committed", { time })}
      </p>
    );
  }
  if (change.status === "rejected" && change.decidedAt) {
    return (
      <p className={cn(baseClass, "font-medium text-danger")}>
        {t("changes.rejected", { time: formatRelativeTime(change.decidedAt) })}
      </p>
    );
  }
  if (change.status === "approved" && change.decidedAt) {
    return (
      <p className={baseClass}>
        {t("changes.approvedWaiting", { time: formatRelativeTime(change.decidedAt) })}
      </p>
    );
  }
  return <p className={baseClass}>{t("changes.autoCommit")}</p>;
}

export function ChangeCard({ change }: { change: PendingChange }) {
  const { t } = useTranslation();
  const approve = useApproveChange();
  const reject = useRejectChange();
  const isDeciding = approve.isPending || reject.isPending;
  const needsDecision = change.status === "pending" && change.requiresConfirmation;

  return (
    <article className="overflow-hidden rounded-card border border-edge bg-raised shadow-card">
      <header className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-edge px-5 py-3">
        <span className="font-mono text-[12px] text-ink-muted">
          #{change.id.slice(0, CHANGE_ID_LENGTH)}
        </span>
        <Badge variant={TYPE_VARIANTS[change.type]}>{change.type}</Badge>
        <span className="font-mono text-[12px] text-ink-muted">
          {change.sourceId}/{change.tableId}
        </span>
        {change.requiresConfirmation ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <span tabIndex={0}>
                <Badge variant="warning">{t("changes.needsConfirmation")}</Badge>
              </span>
            </TooltipTrigger>
            <TooltipContent>{t("changes.needsConfirmationTooltip")}</TooltipContent>
          </Tooltip>
        ) : null}
        <span className="ml-auto flex items-center gap-2.5">
          <RelativeTime iso={change.createdAt} className="font-mono text-[11px] text-ink-muted" />
          <Badge variant={STATUS_VARIANTS[change.status]}>{t(STATUS_LABEL_KEYS[change.status])}</Badge>
        </span>
      </header>

      <div className="px-5 py-4">
        <DiffViewer change={change} />
      </div>

      <footer className="flex items-center justify-between gap-3 border-t border-edge px-5 py-3">
        {needsDecision ? (
          <>
            <p className="text-[13px] font-medium text-warning">{t("changes.awaitingDecision")}</p>
            <div className="flex items-center gap-2">
              <Button
                variant="destructive"
                size="sm"
                disabled={isDeciding}
                onClick={() => reject.mutate(change.id)}
              >
                {reject.isPending ? t("changes.rejecting") : t("changes.reject")}
              </Button>
              <Button size="sm" disabled={isDeciding} onClick={() => approve.mutate(change.id)}>
                {approve.isPending ? t("changes.approving") : t("changes.approve")}
              </Button>
            </div>
          </>
        ) : (
          <ChangeOutcome change={change} />
        )}
      </footer>
    </article>
  );
}
