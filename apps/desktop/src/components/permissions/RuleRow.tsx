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
    <article className="bg-surface">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-edge px-4 py-2">
        <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-ink-muted">
          RULE / <span className="text-ink">{scope}</span>
        </span>
        <span className="font-mono text-[10px] uppercase tracking-[0.05em] text-ink-muted">
          Updated <RelativeTime iso={rule.updatedAt} />
        </span>
        <Button
          variant="ghost"
          size="sm"
          aria-label={`Delete rule for ${scope}`}
          className="ml-auto text-hazard hover:border-hazard hover:text-hazard"
          onClick={() => setIsConfirmOpen(true)}
        >
          Del
        </Button>
      </header>

      <div className="px-4 py-3">
        <div className="flex flex-wrap items-center gap-6">
          {TOGGLES.map(({ field, label }) => (
            <label
              key={field}
              className="flex cursor-pointer items-center gap-2 font-mono text-[11px] uppercase tracking-[0.08em] text-ink-muted"
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

        <div className="mt-3 border-t border-edge pt-3">
          <p className="mb-2 font-mono text-[10px] font-bold uppercase tracking-[0.1em] text-ink-muted">
            [ Require confirmation for ]
          </p>
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
