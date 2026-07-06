import { ShieldAlert } from "lucide-react";
import {
  Badge,
  Button,
  Card,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  type BadgeVariant
} from "@sheet-port/ui";
import type { ChangeStatus, ChangeType, PendingChange } from "@sheet-port/shared";
import { useApproveChange, useRejectChange } from "../../hooks/useChanges.js";
import { RelativeTime } from "../RelativeTime.js";
import { DiffViewer } from "./DiffViewer.js";

const STATUS_VARIANTS: Record<ChangeStatus, BadgeVariant> = {
  pending: "warning",
  approved: "info",
  committed: "success",
  rejected: "danger"
};

const TYPE_VARIANTS: Record<ChangeType, BadgeVariant> = {
  append: "success",
  update: "info",
  delete: "danger"
};

function ChangeOutcome({ change }: { change: PendingChange }) {
  if (change.status === "committed" && change.committedAt) {
    return (
      <p className="text-xs text-ink-muted">
        Committed <RelativeTime iso={change.committedAt} className="text-ink" />
        {change.decidedBy ? ` (decided by ${change.decidedBy})` : null}
      </p>
    );
  }
  if (change.status === "rejected" && change.decidedAt) {
    return (
      <p className="text-xs text-ink-muted">
        Rejected <RelativeTime iso={change.decidedAt} className="text-danger" />
      </p>
    );
  }
  if (change.status === "approved" && change.decidedAt) {
    return (
      <p className="text-xs text-ink-muted">
        Approved <RelativeTime iso={change.decidedAt} className="text-info" />, waiting for the agent to commit
      </p>
    );
  }
  return <p className="text-xs text-ink-muted">Commits automatically; no confirmation required by policy</p>;
}

export function ChangeCard({ change }: { change: PendingChange }) {
  const approve = useApproveChange();
  const reject = useRejectChange();
  const isDeciding = approve.isPending || reject.isPending;
  const needsDecision = change.status === "pending" && change.requiresConfirmation;

  return (
    <Card className="p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={TYPE_VARIANTS[change.type]}>{change.type}</Badge>
        <span className="font-mono text-[13px] text-ink-muted">
          {change.sourceId}/{change.tableId}
        </span>
        {change.requiresConfirmation ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="text-warning" tabIndex={0} aria-label="Requires confirmation">
                <ShieldAlert size={14} aria-hidden />
              </span>
            </TooltipTrigger>
            <TooltipContent>Policy requires user confirmation before commit</TooltipContent>
          </Tooltip>
        ) : null}
        <span className="ml-auto flex items-center gap-2">
          <RelativeTime iso={change.createdAt} className="text-xs text-ink-muted" />
          <Badge variant={STATUS_VARIANTS[change.status]}>{change.status}</Badge>
        </span>
      </div>

      <div className="mt-3">
        <DiffViewer change={change} />
      </div>

      <div className="mt-3 flex items-center justify-between gap-3">
        {needsDecision ? (
          <>
            <p className="text-xs text-warning">Waiting for your decision</p>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="border-danger/30 text-danger hover:bg-danger/10"
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
      </div>
    </Card>
  );
}
