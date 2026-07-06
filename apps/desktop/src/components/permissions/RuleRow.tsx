import { useState } from "react";
import {
  Button,
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
  const scope = `${rule.sourceId}/${rule.tableId ?? "*"}`;

  const saveWith = (patch: Partial<SavePermissionRule>) => {
    save.mutate({ ...toSaveShape(rule), ...patch });
  };

  return (
    <article className="rounded-card border border-edge bg-raised shadow-card">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-edge px-5 py-3">
        <span className="font-mono text-[13px] font-medium text-ink">{scope}</span>
        <span className="text-[12px] text-ink-muted">
          Updated <RelativeTime iso={rule.updatedAt} />
        </span>
        <Button
          variant="ghost"
          size="sm"
          aria-label={`Delete rule for ${scope}`}
          className="ml-auto text-danger hover:bg-danger/10 hover:text-danger"
          onClick={() => setIsConfirmOpen(true)}
        >
          Delete
        </Button>
      </header>

      <div className="px-5 py-4">
        <div className="flex flex-wrap items-center gap-6">
          {TOGGLES.map(({ field, label }) => (
            <label
              key={field}
              className="flex cursor-pointer items-center gap-2 text-[13px] text-ink"
            >
              <Switch
                checked={rule[field]}
                disabled={save.isPending}
                onCheckedChange={(checked) => saveWith(buildTogglePatch(field, checked))}
                aria-label={`${label} access for ${scope}`}
              />
              {label}
            </label>
          ))}
        </div>

        <div className="mt-4 border-t border-edge pt-4">
          <p className="overline-label mb-2.5">Require confirmation for</p>
          <ConfirmationChips
            value={rule.requireConfirmationFor}
            disabled={save.isPending}
            onChange={(next) => saveWith({ requireConfirmationFor: next })}
          />
        </div>
      </div>

      <Dialog open={isConfirmOpen} onOpenChange={setIsConfirmOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete rule</DialogTitle>
            <DialogDescription>
              Agents lose all access granted by the rule for{" "}
              <span className="font-mono text-ink">{scope}</span>. This cannot be undone.
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
    </article>
  );
}
