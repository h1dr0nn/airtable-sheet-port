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

const FIELD_LABEL_CLASS = "text-[12px] font-medium text-ink-muted";

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
  const sourceList = sources ?? [];
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
          <DialogTitle>New rule</DialogTitle>
          <DialogDescription>
            Grant agents scoped access to a source, or a single table inside it.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <label className={FIELD_LABEL_CLASS} htmlFor="rule-source">
              Source
            </label>
            {sourceList.length === 0 ? (
              <p className="rounded-md border border-edge bg-surface px-3 py-2 text-[12.5px] leading-5 text-ink-muted">
                No data sources connected yet. Connect a data source first; rules always target a
                connected source.
              </p>
            ) : (
              <Select value={draft.sourceId} onValueChange={(sourceId) => setDraft({ ...draft, sourceId })}>
                <SelectTrigger id="rule-source" className="w-full" aria-label="Source">
                  <SelectValue placeholder="Choose a source" />
                </SelectTrigger>
                <SelectContent>
                  {sourceList.map((source) => (
                    <SelectItem key={source.id} value={source.id}>
                      {source.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          <div className="space-y-1.5">
            <label className={FIELD_LABEL_CLASS} htmlFor="rule-table">
              Table id <span className="font-normal">(blank = entire source)</span>
            </label>
            <Input
              id="rule-table"
              value={draft.tableId}
              placeholder="customers"
              onChange={(event) => setDraft({ ...draft, tableId: event.target.value })}
            />
          </div>

          <div className="flex items-center gap-6">
            <label className="flex cursor-pointer items-center gap-2 text-[13px] text-ink">
              <Switch checked={draft.read} onCheckedChange={(read) => setDraft({ ...draft, read })} aria-label="Read" />
              Read
            </label>
            <label className="flex cursor-pointer items-center gap-2 text-[13px] text-ink">
              <Switch checked={draft.write} onCheckedChange={(write) => setDraft({ ...draft, write })} aria-label="Write" />
              Write
            </label>
            <label className="flex cursor-pointer items-center gap-2 text-[13px] text-ink">
              <Switch
                checked={draft.deleteRecords}
                onCheckedChange={(deleteRecords) => setDraft({ ...draft, deleteRecords })}
                aria-label="Delete"
              />
              Delete
            </label>
          </div>

          <div className="space-y-1.5">
            <p className={FIELD_LABEL_CLASS}>Require confirmation for</p>
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
