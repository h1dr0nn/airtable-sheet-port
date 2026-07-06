import { getCurrentWindow } from "@tauri-apps/api/window";
import { Grid2x2, Minus, Square, X } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@sheet-port/ui";
import { isTauri } from "../lib/ipc.js";

const CONTROL_ICON_SIZE = 14;

type WindowControlProps = {
  label: string;
  onClick: () => void;
  isClose?: boolean;
  children: ReactNode;
};

function WindowControl({ label, onClick, isClose = false, children }: WindowControlProps) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "flex h-full w-[46px] items-center justify-center text-ink-muted transition-colors",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-accent/60",
        isClose ? "hover:bg-danger hover:text-bg" : "hover:bg-raised hover:text-ink"
      )}
    >
      {children}
    </button>
  );
}

/** Custom titlebar for the undecorated Tauri window. Hidden in plain-browser dev. */
export function Titlebar() {
  if (!isTauri) {
    return null;
  }
  const appWindow = getCurrentWindow();

  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 select-none items-center justify-between border-b border-edge bg-bg"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5 pl-4">
        <span className="flex h-5 w-5 items-center justify-center rounded bg-accent/15 text-accent">
          <Grid2x2 size={12} aria-hidden />
        </span>
        <span className="pointer-events-none text-[11px] font-medium tracking-wide text-ink-muted">
          Airtable - Sheet Port
        </span>
      </div>
      <div className="flex h-full items-stretch">
        <WindowControl label="Minimize window" onClick={() => void appWindow.minimize()}>
          <Minus size={CONTROL_ICON_SIZE} aria-hidden />
        </WindowControl>
        <WindowControl label="Toggle maximize" onClick={() => void appWindow.toggleMaximize()}>
          <Square size={CONTROL_ICON_SIZE - 2} aria-hidden />
        </WindowControl>
        <WindowControl label="Close window" onClick={() => void appWindow.close()} isClose>
          <X size={CONTROL_ICON_SIZE} aria-hidden />
        </WindowControl>
      </div>
    </header>
  );
}
