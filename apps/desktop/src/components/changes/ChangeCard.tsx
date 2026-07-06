import {
  Badge,
  Button,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import type { ChangeStatus, ChangeType, PendingChange } from "@sheet-port/shared";
import { useApproveChange, useRejectChange } from "../../hooks/useChanges.js";
import { RelativeTime } from "../RelativeTime.js";
import { DiffViewer } from "./DiffViewer.js";

const CHANGE_ID_LENGTH = 8;

const STATUS_VARIANTS: Record<ChangeStatus, BadgeVariant> = {
  pending: "warning",
  approved: "default",
  committed: "success",
  rejected: "danger"
};

const STATUS_LABELS: Record<ChangeStatus, string> = {
  pending: "Pending",
  approved: "Approved",
  committed: "Committed",
  rejected: "Rejected"
};

const TYPE_VARIANTS: Record<ChangeType, BadgeVariant> = {
  append: "default",
  update: "default",
  delete: "danger"
};

function ChangeOutcome({ change }: { change: PendingChange }) {
  const baseClass = "text-[12.5px] text-ink-muted";
  if (change.status === "committed" && change.committedAt) {
    return (
      <p className={baseClass}>
        Committed <RelativeTime iso={change.committedAt} className="text-ink" />
        {change.decidedBy ? ` by ${change.decidedBy}` : null}
      </p>
    );
  }
  if (change.status === "rejected" && change.decidedAt) {
    return (
      <p className={baseClass}>
        <span className="font-medium text-danger">Rejected</span>{" "}
        <RelativeTime iso={change.decidedAt} className="text-danger" />
      </p>
    );
  }
  if (change.status === "approved" && change.decidedAt) {
    return (
      <p className={baseClass}>
        Approved <RelativeTime iso={change.decidedAt} className="text-ink" /> · waiting for the agent
        to commit
      </p>
    );
  }
  return <p className={baseClass}>Auto-commit · no confirmation required by policy</p>;
}

export function ChangeCard({ change }: { change: PendingChange }) {
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
                <Badge variant="warning">Needs confirmation</Badge>
              </span>
            </TooltipTrigger>
            <TooltipContent>Policy requires user confirmation before commit</TooltipContent>
          </Tooltip>
        ) : null}
        <span className="ml-auto flex items-center gap-2.5">
          <RelativeTime iso={change.createdAt} className="font-mono text-[11px] text-ink-muted" />
          <Badge variant={STATUS_VARIANTS[change.status]}>{STATUS_LABELS[change.status]}</Badge>
        </span>
      </header>

      <div className="px-5 py-4">
        <DiffViewer change={change} />
      </div>

      <footer className="flex items-center justify-between gap-3 border-t border-edge px-5 py-3">
        {needsDecision ? (
          <>
            <p className="text-[13px] font-medium text-warning">Awaiting your decision</p>
            <div className="flex items-center gap-2">
              <Button
                variant="destructive"
                size="sm"
                disabled={isDeciding}
                onClick={() => reject.mutate(change.id)}
              >
                {reject.isPending ? "Rejecting..." : "Reject"}
              </Button>
              <Button size="sm" disabled={isDeciding} onClick={() => approve.mutate(change.id)}>
                {approve.isPending ? "Approving..." : "Approve"}
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
