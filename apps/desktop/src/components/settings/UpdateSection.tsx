import { useState } from "react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "@sheet-port/ui";
import { useAppStatus } from "../../hooks/useAppStatus.js";
import { useUpdate } from "../../hooks/useUpdate.js";

/**
 * Settings block for manual update checks. Exported for the Settings screen
 * (owned by another agent) to import and render. It runs its own useUpdate
 * instance independent of the launch check so the "Check for Updates" button
 * has fresh state and a visible result.
 *
 * Flow: check -> when found, show version + release notes -> confirm modal ->
 * download & install (progress) -> relaunch (handled inside useUpdate.install).
 */
export function UpdateSection() {
  const { data: status } = useAppStatus();
  const currentVersion = status?.appVersion ?? "unknown";
  const update = useUpdate();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);
  const [wasChecked, setWasChecked] = useState(false);

  const handleCheck = async () => {
    setWasChecked(true);
    await update.check();
  };

  const handleConfirmInstall = () => {
    // The modal stays open while downloading so the progress label is visible;
    // a successful install relaunches the app so nothing else needs to close it.
    void update.install();
  };

  // "Up to date" only after an explicit check that found nothing (not on mount).
  const showUpToDate = wasChecked && !update.checking && !update.available && !update.error;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-[13px] font-medium text-ink">Current Version</p>
          <p className="mt-0.5 text-[12px] text-ink-muted">v{currentVersion}</p>
        </div>
        <Button size="sm" variant="secondary" disabled={update.checking} onClick={handleCheck}>
          {update.checking ? "Checking..." : "Check for Updates"}
        </Button>
      </div>

      {update.error ? (
        <p className="text-[12px] text-danger">{update.error}</p>
      ) : null}

      {showUpToDate ? (
        <p className="text-[12px] text-ink-muted">You are running the latest version.</p>
      ) : null}

      {update.available ? (
        <div className="rounded-card border border-accent/30 bg-accent/[0.07] p-4">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <p className="text-[13px] font-semibold text-accent">
                Update Available
                {update.version ? <span className="text-ink"> - v{update.version}</span> : null}
              </p>
            </div>
            <Button size="sm" disabled={update.downloading} onClick={() => setIsConfirmOpen(true)}>
              {update.downloading ? "Installing..." : "Download & Install"}
            </Button>
          </div>
          {update.notes ? (
            <div className="mt-3 border-t border-accent/20 pt-3">
              <p className="mb-1 text-[11.5px] font-semibold uppercase tracking-wide text-ink-muted">
                Release Notes
              </p>
              <p className="whitespace-pre-wrap text-[12px] leading-5 text-ink-muted">
                {update.notes}
              </p>
            </div>
          ) : null}
        </div>
      ) : null}

      <Dialog open={isConfirmOpen} onOpenChange={setIsConfirmOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Install Update</DialogTitle>
            <DialogDescription>
              {update.version
                ? `Version ${update.version} will be downloaded and installed. The app will restart to finish.`
                : "The update will be downloaded and installed. The app will restart to finish."}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="ghost"
              disabled={update.downloading}
              onClick={() => setIsConfirmOpen(false)}
            >
              Cancel
            </Button>
            <Button disabled={update.downloading} onClick={handleConfirmInstall}>
              {update.downloading ? "Installing..." : "Download & Install"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
