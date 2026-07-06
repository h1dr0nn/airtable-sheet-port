import { useState } from "react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch
} from "@sheet-port/ui";
import { useSavePermissionRule } from "../../hooks/usePermissions.js";
import { useSources } from "../../hooks/useSources.js";
import { ConfirmationChips } from "./ConfirmationChips.js";

type RuleFormDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

type RuleDraft = {
  sourceId: string;
  tableId: string;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: string[];
};

const EMPTY_DRAFT: RuleDraft = {
  sourceId: "",
  tableId: "",
  read: true,
  write: false,
  deleteRecords: false,
  requireConfirmationFor: ["append", "update", "delete", "bulk_update"]
};

export function RuleFormDialog({ open, onOpenChange }: RuleFormDialogProps) {
  const { data: sources } = useSources();
  const save = useSavePermissionRule();
  const [draft, setDraft] = useState<RuleDraft>(EMPTY_DRAFT);
  const canSubmit = draft.sourceId !== "" && !save.isPending;

  const submit = () => {
    save.mutate(
      {
        id: null,
        sourceId: draft.sourceId,
        tableId: draft.tableId.trim() === "" ? null : draft.tableId.trim(),
        read: draft.read,
        write: draft.write,
        deleteRecords: draft.deleteRecords,
        requireConfirmationFor: draft.requireConfirmationFor
      },
      {
        onSuccess: () => {
          setDraft(EMPTY_DRAFT);
          onOpenChange(false);
        }
      }
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add permission rule</DialogTitle>
          <DialogDescription>
            Grant agents scoped access to a source, or a single table inside it.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-[11px] font-semibold uppercase tracking-widest text-ink-muted" htmlFor="rule-source">
              Source
            </label>
            <Select value={draft.sourceId} onValueChange={(sourceId) => setDraft({ ...draft, sourceId })}>
              <SelectTrigger id="rule-source" className="w-full" aria-label="Source">
                <SelectValue placeholder="Choose a source" />
              </SelectTrigger>
              <SelectContent>
                {(sources ?? []).map((source) => (
                  <SelectItem key={source.id} value={source.id}>
                    {source.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <label className="text-[11px] font-semibold uppercase tracking-widest text-ink-muted" htmlFor="rule-table">
              Table id <span className="normal-case tracking-normal text-ink-muted/70">(blank = entire source)</span>
            </label>
            <Input
              id="rule-table"
              value={draft.tableId}
              placeholder="customers"
              className="font-mono text-[13px]"
              onChange={(event) => setDraft({ ...draft, tableId: event.target.value })}
            />
          </div>

          <div className="flex items-center gap-6">
            <label className="flex cursor-pointer items-center gap-2 text-xs text-ink-muted">
              <Switch checked={draft.read} onCheckedChange={(read) => setDraft({ ...draft, read })} aria-label="Read" />
              Read
            </label>
            <label className="flex cursor-pointer items-center gap-2 text-xs text-ink-muted">
              <Switch checked={draft.write} onCheckedChange={(write) => setDraft({ ...draft, write })} aria-label="Write" />
              Write
            </label>
            <label className="flex cursor-pointer items-center gap-2 text-xs text-ink-muted">
              <Switch
                checked={draft.deleteRecords}
                onCheckedChange={(deleteRecords) => setDraft({ ...draft, deleteRecords })}
                aria-label="Delete"
              />
              Delete
            </label>
          </div>

          <div className="space-y-1.5">
            <p className="text-[11px] font-semibold uppercase tracking-widest text-ink-muted">
              Require confirmation for
            </p>
            <ConfirmationChips
              value={draft.requireConfirmationFor}
              onChange={(requireConfirmationFor) => setDraft({ ...draft, requireConfirmationFor })}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button disabled={!canSubmit} onClick={submit}>
            {save.isPending ? "Saving..." : "Save rule"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
