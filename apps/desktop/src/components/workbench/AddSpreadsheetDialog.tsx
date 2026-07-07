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
  SelectValue
} from "@sheet-port/ui";
import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { AddSpreadsheetInput, WorkbenchFolder } from "../../lib/ipc.js";

// Radix Select needs a non-empty string value, so Ungrouped rides a sentinel
// that maps back to a null folderId on submit.
const UNGROUPED_VALUE = "__ungrouped__";

type AddSpreadsheetDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  folders: WorkbenchFolder[];
  /** Preselected target folder; null selects Ungrouped. */
  defaultFolderId: string | null;
  isPending: boolean;
  onSubmit: (input: AddSpreadsheetInput) => void;
};

/** Dialog for adding a spreadsheet by URL/id into a chosen folder. */
export function AddSpreadsheetDialog({
  open,
  onOpenChange,
  folders,
  defaultFolderId,
  isPending,
  onSubmit
}: AddSpreadsheetDialogProps) {
  const { t } = useTranslation();
  const [urlOrId, setUrlOrId] = useState("");
  const [folderValue, setFolderValue] = useState(defaultFolderId ?? UNGROUPED_VALUE);

  // Reset the form each time the dialog opens, honoring the context folder.
  useEffect(() => {
    if (open) {
      setUrlOrId("");
      setFolderValue(defaultFolderId ?? UNGROUPED_VALUE);
    }
  }, [open, defaultFolderId]);

  const trimmed = urlOrId.trim();
  const canSubmit = trimmed !== "" && !isPending;

  const submit = () => {
    if (!canSubmit) {
      return;
    }
    onSubmit({
      folderId: folderValue === UNGROUPED_VALUE ? null : folderValue,
      urlOrId: trimmed
    });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("workbench.addSpreadsheet")}</DialogTitle>
          <DialogDescription>{t("workbench.addSpreadsheetDescription")}</DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4">
          <label className="flex flex-col gap-1.5">
            <span className="text-[12px] font-medium text-ink-muted">
              {t("workbench.spreadsheetUrlLabel")}
            </span>
            <Input
              autoFocus
              value={urlOrId}
              onChange={(event) => setUrlOrId(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  submit();
                }
              }}
              placeholder={t("workbench.spreadsheetUrlPlaceholder")}
            />
          </label>

          <label className="flex flex-col gap-1.5">
            <span className="text-[12px] font-medium text-ink-muted">
              {t("workbench.folderLabel")}
            </span>
            <Select value={folderValue} onValueChange={setFolderValue}>
              <SelectTrigger aria-label={t("workbench.folderLabel")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNGROUPED_VALUE}>{t("workbench.ungrouped")}</SelectItem>
                {folders.map((folder) => (
                  <SelectItem key={folder.id} value={folder.id}>
                    {folder.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </label>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={isPending}>
            {t("common.cancel")}
          </Button>
          <Button onClick={submit} disabled={!canSubmit}>
            {isPending ? t("common.working") : t("workbench.add")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
