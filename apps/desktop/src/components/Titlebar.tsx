import { type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQueryClient } from "@tanstack/react-query";
import { Check } from "lucide-react";
import {
  cn,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
  FOCUS_RING,
  toast
} from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { useTheme } from "../hooks/useTheme.js";
import { isTauri } from "../lib/ipc.js";
import { buildAppMenu, type MenuEntry } from "../lib/menu.js";
import type { ScreenId } from "../lib/nav.js";

// Shared 46px hover zone so menu/search buttons read like the window controls.
const TITLEBAR_BUTTON_CLASS = cn(
  "flex h-full w-[46px] items-center justify-center text-ink-muted transition-colors",
  "hover:bg-surface hover:text-ink",
  FOCUS_RING,
  "focus-visible:ring-offset-0"
);

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

const MENU_GLYPH_PROPS = {
  width: 12,
  height: 12,
  viewBox: "0 0 12 12",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.25,
  strokeLinecap: "round",
  "aria-hidden": true
} as const;

function renderMenuEntries(entries: readonly MenuEntry[]): ReactNode {
  return entries.map((entry) => {
    if (entry.kind === "separator") {
      return <DropdownMenuSeparator key={entry.id} />;
    }
    if (entry.kind === "submenu") {
      return (
        <DropdownMenuSub key={entry.id}>
          <DropdownMenuSubTrigger>{entry.label}</DropdownMenuSubTrigger>
          <DropdownMenuSubContent>{renderMenuEntries(entry.items)}</DropdownMenuSubContent>
        </DropdownMenuSub>
      );
    }
    return (
      <DropdownMenuItem key={entry.id} onSelect={entry.run}>
        <span className="min-w-0 flex-1">{entry.label}</span>
        {entry.checked ? <Check size={14} aria-hidden className="shrink-0 text-accent" /> : null}
      </DropdownMenuItem>
    );
  });
}

function WindowControls() {
  const appWindow = getCurrentWindow();
  return (
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
  );
}

type TitlebarProps = {
  onNavigate: (screen: ScreenId) => void;
  /** Opens the app-wide command palette (also bound to Ctrl/Cmd+K). */
  onOpenPalette: () => void;
};

/** Custom titlebar: hamburger menu + command-palette search on the left,
 * window controls on the right (Tauri only). The middle stays a drag region. */
export function Titlebar({ onNavigate, onOpenPalette }: TitlebarProps) {
  const queryClient = useQueryClient();
  const { setting, setSetting } = useTheme();
  const { data: status } = useAppStatus();

  const copyVersion = () => {
    const version = status?.appVersion;
    if (!version) {
      toast.error("Version unavailable", { description: "App status has not loaded yet" });
      return;
    }
    navigator.clipboard
      .writeText(version)
      .then(() => toast.success("Version copied", { description: version }))
      .catch((error: unknown) =>
        toast.error("Copy failed", { description: error instanceof Error ? error.message : String(error) })
      );
  };

  const menu = buildAppMenu({
    navigate: onNavigate,
    reloadData: () => {
      void queryClient.invalidateQueries();
      toast.info("Reloading data");
    },
    quit: isTauri ? () => void getCurrentWindow().close() : null,
    themeSetting: setting,
    setTheme: setSetting,
    copyVersion
  });

  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 select-none items-stretch justify-between border-b border-edge bg-bg"
    >
      <div className="flex h-full items-stretch">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button type="button" aria-label="Application menu" className={TITLEBAR_BUTTON_CLASS}>
              <svg {...MENU_GLYPH_PROPS}>
                <path d="M1.5 3h9M1.5 6h9M1.5 9h9" />
              </svg>
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">{renderMenuEntries(menu)}</DropdownMenuContent>
        </DropdownMenu>

        <button
          type="button"
          aria-label="Open command palette"
          onClick={onOpenPalette}
          className={TITLEBAR_BUTTON_CLASS}
        >
          <svg {...MENU_GLYPH_PROPS}>
            <circle cx="5.25" cy="5.25" r="3.5" />
            <path d="M8 8l2.5 2.5" />
          </svg>
        </button>
      </div>

      {isTauri ? <WindowControls /> : null}
    </header>
  );
}
