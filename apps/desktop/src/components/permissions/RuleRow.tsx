import { Trash2 } from "lucide-react";
import { useState } from "react";
import {
  Button,
  Card,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Switch
} from "@sheet-port/ui";
import { useDeletePermissionRule, useSavePermissionRule } from "../../hooks/usePermissions.js";
import type { PermissionRuleRow, SavePermissionRule } from "../../lib/ipc.js";
import { RelativeTime } from "../RelativeTime.js";
import { ConfirmationChips } from "./ConfirmationChips.js";

type ToggleField = "read" | "write" | "deleteRecords";

const TOGGLES: ReadonlyArray<{ field: ToggleField; label: string }> = [
  { field: "read", label: "Read" },
  { field: "write", label: "Write" },
  { field: "deleteRecords", label: "Delete" }
];

function buildTogglePatch(field: ToggleField, checked: boolean): Partial<SavePermissionRule> {
  switch (field) {
    case "read":
      return { read: checked };
    case "write":
      return { write: checked };
    case "deleteRecords":
      return { deleteRecords: checked };
  }
}

function toSaveShape(rule: PermissionRuleRow): SavePermissionRule {
  return {
    id: rule.id,
    sourceId: rule.sourceId,
    tableId: rule.tableId,
    read: rule.read,
    write: rule.write,
    deleteRecords: rule.deleteRecords,
    requireConfirmationFor: [...rule.requireConfirmationFor]
  };
}

export function RuleRow({ rule }: { rule: PermissionRuleRow }) {
  const save = useSavePermissionRule();
  const remove = useDeletePermissionRule();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  const saveWith = (patch: Partial<SavePermissionRule>) => {
    save.mutate({ ...toSaveShape(rule), ...patch });
  };

  return (
    <Card className="p-4">
      <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
        <div className="min-w-44 flex-1">
          <p className="font-mono text-[13px] text-ink">{rule.sourceId}</p>
          <p className="mt-0.5 text-xs text-ink-muted">
            {rule.tableId ? (
              <span className="font-mono">{rule.tableId}</span>
            ) : (
              "entire source"
            )}
            {" · updated "}
            <RelativeTime iso={rule.updatedAt} />
          </p>
        </div>
        <div className="flex items-center gap-5">
          {TOGGLES.map(({ field, label }) => (
            <label key={field} className="flex cursor-pointer items-center gap-2 text-xs text-ink-muted">
              <Switch
                checked={rule[field]}
                disabled={save.isPending}
                onCheckedChange={(checked) => saveWith(buildTogglePatch(field, checked))}
                aria-label={`${label} access for ${rule.sourceId}/${rule.tableId ?? "entire source"}`}
              />
              {label}
            </label>
          ))}
        </div>
        <Button
          variant="ghost"
          size="icon"
          aria-label={`Delete rule for ${rule.sourceId}/${rule.tableId ?? "entire source"}`}
          className="hover:text-danger"
          onClick={() => setIsConfirmOpen(true)}
        >
          <Trash2 size={14} aria-hidden />
        </Button>
      </div>

      <div className="mt-3 border-t border-edge pt-3">
        <p className="mb-2 text-[11px] font-semibold uppercase tracking-widest text-ink-muted">
          Require confirmation for
        </p>
        <ConfirmationChips
          value={rule.requireConfirmationFor}
          disabled={save.isPending}
          onChange={(next) => saveWith({ requireConfirmationFor: next })}
        />
      </div>

      <Dialog open={isConfirmOpen} onOpenChange={setIsConfirmOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete permission rule</DialogTitle>
            <DialogDescription>
              Agents lose all access granted by the rule for{" "}
              <span className="font-mono text-ink">
                {rule.sourceId}/{rule.tableId ?? "*"}
              </span>
              . This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setIsConfirmOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={() => {
                remove.mutate(rule.id, { onSuccess: () => setIsConfirmOpen(false) });
              }}
            >
              {remove.isPending ? "Deleting..." : "Delete rule"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
