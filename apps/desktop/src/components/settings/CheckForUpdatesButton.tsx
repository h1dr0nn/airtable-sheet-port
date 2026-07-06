import { useState } from "react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  toast
} from "@sheet-port/ui";
import { useUpdate } from "../../hooks/useUpdate.js";

/**
 * Ghost "Check for Updates" control for the About card header. Manual checks
 * report their result transiently rather than leaving a permanent line in
 * Settings:
 *   - a newer version opens a confirm modal offering Install (download + relaunch)
 *   - up to date (including a platform-less seed manifest) shows a subtle toast
 *   - a real network/signature failure shows an error toast
 * The launch check in App stays silent regardless (it uses its own instance).
 */
export function CheckForUpdatesButton() {
  const update = useUpdate();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  // check() returns the outcome synchronously, so we branch on the result
  // rather than reading state that has not re-rendered yet.
  const onClickCheck = () => {
    void (async () => {
      const result = await update.check();
      switch (result.status) {
        case "available":
          setIsConfirmOpen(true);
          break;
        case "up-to-date":
          toast.success("You're on the latest version");
          break;
        case "error":
          toast.error("Update check failed", { description: result.message });
          break;
      }
    })();
  };

  const handleConfirmInstall = () => {
    // The modal stays open while downloading so the progress label is visible;
    // a successful install relaunches the app so nothing else needs to close it.
    void update.install();
  };

  return (
    <>
      <Button
        size="sm"
        variant="ghost"
        disabled={update.checking || update.downloading}
        onClick={onClickCheck}
      >
        {update.checking ? "Checking..." : "Check for Updates"}
      </Button>

      <Dialog open={isConfirmOpen} onOpenChange={setIsConfirmOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Update Available</DialogTitle>
            <DialogDescription>
              {update.version
                ? `Version ${update.version} will be downloaded and installed. The app will restart to finish.`
                : "A newer version will be downloaded and installed. The app will restart to finish."}
            </DialogDescription>
          </DialogHeader>
          {update.notes ? (
            <div className="max-h-40 overflow-y-auto rounded-md border border-edge bg-surface p-3">
              <p className="mb-1 text-[11.5px] font-semibold uppercase tracking-wide text-ink-muted">
                Release Notes
              </p>
              <p className="whitespace-pre-wrap text-[12px] leading-5 text-ink-muted">
                {update.notes}
              </p>
            </div>
          ) : null}
          <DialogFooter>
            <Button
              variant="ghost"
              disabled={update.downloading}
              onClick={() => setIsConfirmOpen(false)}
            >
              Cancel
            </Button>
            <Button disabled={update.downloading} onClick={handleConfirmInstall}>
              {update.downloading ? "Installing..." : "Install"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
