import { useState } from "react";
import {
  Button,
  Checkbox,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "@sheet-port/ui";
import { ipc, type CloseBehavior } from "../lib/ipc.js";
import { useSetCloseBehavior } from "../hooks/useCloseBehavior.js";

type CloseBehaviorDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

/**
 * Prompt shown when the window close button is clicked while the close behavior
 * is "ask". Offers Minimize to Tray or Quit; "Remember My Choice" first persists
 * the picked behavior so the prompt is skipped next time.
 */
export function CloseBehaviorDialog({ open, onOpenChange }: CloseBehaviorDialogProps) {
  const [remember, setRemember] = useState(false);
  const setCloseBehavior = useSetCloseBehavior();
  const [pending, setPending] = useState<CloseBehavior | null>(null);

  const decide = async (behavior: "tray" | "quit"): Promise<void> => {
    setPending(behavior);
    try {
      if (remember) {
        // Persist first so the next close skips this dialog entirely.
        await setCloseBehavior.mutateAsync(behavior);
      }
      if (behavior === "tray") {
        await ipc.windowHideToTray();
      } else {
        await ipc.windowQuit();
      }
      onOpenChange(false);
    } finally {
      setPending(null);
    }
  };

  const isBusy = pending !== null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Run in Background?</DialogTitle>
          <DialogDescription>
            Keep the app running in the system tray, or quit it entirely.
          </DialogDescription>
        </DialogHeader>

        <label className="flex cursor-pointer items-center gap-2 text-[13px] text-ink">
          <Checkbox
            checked={remember}
            onCheckedChange={(checked) => setRemember(checked === true)}
            disabled={isBusy}
          />
          Remember My Choice
        </label>

        <DialogFooter>
          <Button
            variant="outline"
            disabled={isBusy}
            onClick={() => void decide("quit")}
          >
            {pending === "quit" ? "Quitting..." : "Quit"}
          </Button>
          <Button disabled={isBusy} onClick={() => void decide("tray")}>
            {pending === "tray" ? "Minimizing..." : "Minimize to Tray"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
