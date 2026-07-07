import {
  Button,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input
} from "@sheet-port/ui";
import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n/useTranslation.js";

type FolderNameDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  submitLabel: string;
  /** Prefilled name; used by the rename flow, empty for create. */
  initialName?: string;
  isPending: boolean;
  onSubmit: (name: string) => void;
};

/** Small single-field dialog for creating or renaming a Workbench folder. */
export function FolderNameDialog({
  open,
  onOpenChange,
  title,
  submitLabel,
  initialName = "",
  isPending,
  onSubmit
}: FolderNameDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(initialName);

  // Reset the field whenever the dialog (re)opens so stale text never lingers.
  useEffect(() => {
    if (open) {
      setName(initialName);
    }
  }, [open, initialName]);

  const trimmed = name.trim();
  const canSubmit = trimmed !== "" && !isPending;

  const submit = () => {
    if (!canSubmit) {
      return;
    }
    onSubmit(trimmed);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <label className="flex flex-col gap-1.5">
          <span className="text-[12px] font-medium text-ink-muted">
            {t("workbench.folderNameLabel")}
          </span>
          <Input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                submit();
              }
            }}
            placeholder={t("workbench.folderNamePlaceholder")}
          />
        </label>
        <DialogFooter>
          <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={isPending}>
            {t("common.cancel")}
          </Button>
          <Button onClick={submit} disabled={!canSubmit}>
            {isPending ? t("common.working") : submitLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
