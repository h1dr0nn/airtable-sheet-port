import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn, FOCUS_RING } from "@sheet-port/ui";
import type { ReactNode } from "react";
import { isTauri } from "../lib/ipc.js";

type WindowControlProps = {
  label: string;
  children: ReactNode;
  onClick: () => void;
  isClose?: boolean;
};

function WindowControl({ label, children, onClick, isClose = false }: WindowControlProps) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "flex h-full w-[46px] items-center justify-center text-ink-muted transition-colors",
        FOCUS_RING,
        "focus-visible:ring-offset-0",
        isClose ? "hover:bg-danger-solid hover:text-white" : "hover:bg-surface hover:text-ink"
      )}
    >
      {children}
    </button>
  );
}

const GLYPH_PROPS = {
  width: 10,
  height: 10,
  viewBox: "0 0 10 10",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1,
  "aria-hidden": true
} as const;

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
      <span data-tauri-drag-region className="pointer-events-none pl-4 text-[12px] font-medium text-ink-muted">
        Airtable - Sheet Port
      </span>
      <div className="flex h-full items-stretch">
        <WindowControl label="Minimize window" onClick={() => void appWindow.minimize()}>
          <svg {...GLYPH_PROPS}>
            <path d="M0.5 5h9" />
          </svg>
        </WindowControl>
        <WindowControl label="Toggle maximize" onClick={() => void appWindow.toggleMaximize()}>
          <svg {...GLYPH_PROPS}>
            <rect x="1" y="1" width="8" height="8" rx="1" />
          </svg>
        </WindowControl>
        <WindowControl label="Close window" isClose onClick={() => void appWindow.close()}>
          <svg {...GLYPH_PROPS}>
            <path d="M1.5 1.5l7 7M8.5 1.5l-7 7" />
          </svg>
        </WindowControl>
      </div>
    </header>
  );
}
