import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "@sheet-port/ui";
import { isTauri } from "../lib/ipc.js";

type WindowControlProps = {
  label: string;
  glyph: string;
  onClick: () => void;
  isClose?: boolean;
};

function WindowControl({ label, glyph, onClick, isClose = false }: WindowControlProps) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "flex h-full w-[46px] items-center justify-center font-mono text-xs text-ink-muted transition-colors",
        "focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-2 focus-visible:outline-hazard",
        isClose ? "hover:bg-hazard hover:text-bg" : "hover:bg-raised hover:text-ink"
      )}
    >
      <span aria-hidden>{glyph}</span>
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
      className="flex h-10 shrink-0 select-none items-center justify-between bg-bg"
    >
      <div data-tauri-drag-region className="flex items-baseline pl-4">
        <span className="pointer-events-none font-mono text-[10px] uppercase tracking-[0.15em] text-ink-muted">
          Airtable - Sheet Port
        </span>
        <span
          aria-hidden
          className="pointer-events-none ml-1 font-mono text-[10px] text-ink-muted motion-safe:animate-blink"
        >
          _
        </span>
      </div>
      <div className="flex h-full items-stretch">
        <WindowControl label="Minimize window" glyph="-" onClick={() => void appWindow.minimize()} />
        <WindowControl label="Toggle maximize" glyph="□" onClick={() => void appWindow.toggleMaximize()} />
        <WindowControl label="Close window" glyph="✕" isClose onClick={() => void appWindow.close()} />
      </div>
    </header>
  );
}
