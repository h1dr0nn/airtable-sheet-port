import { useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQueryClient } from "@tanstack/react-query";
import { Bell, Check, PanelLeft, PanelLeftClose } from "lucide-react";
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
  toast,
  Tooltip,
  TooltipContent,
  TooltipHint,
  TooltipTrigger
} from "@sheet-port/ui";
import { useAppStatus } from "../hooks/useAppStatus.js";
import { useTheme } from "../hooks/useTheme.js";
import { isTauri } from "../lib/ipc.js";
import { buildAppMenu, type MenuEntry } from "../lib/menu.js";
import type { ScreenId } from "../lib/nav.js";
import { AuditDropdown } from "./AuditDropdown.js";

// Gap that separates the activity button from the window-control cluster so it
// reads as a distinct control rather than another window button.
const CONTROL_CLUSTER_GAP = "mr-2";

// Shared 46px hover zone so menu/search buttons read like the window controls.
const TITLEBAR_BUTTON_CLASS = cn(
  "flex h-full w-[46px] items-center justify-center text-ink-muted transition-colors",
  "hover:bg-surface hover:text-ink",
  FOCUS_RING,
  "focus-visible:ring-offset-0"
);

type WindowControlProps = {
  /** Accessible name; also the tooltip text. */
  label: string;
  /** Short tooltip label when it should differ from the a11y name. */
  tooltip: string;
  children: ReactNode;
  onClick: () => void;
  isClose?: boolean;
};

function WindowControl({ label, tooltip, children, onClick, isClose = false }: WindowControlProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
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
      </TooltipTrigger>
      <TooltipContent side="bottom">{tooltip}</TooltipContent>
    </Tooltip>
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
      <WindowControl
        label="Minimize window"
        tooltip="Minimize"
        onClick={() => void appWindow.minimize()}
      >
        <svg {...GLYPH_PROPS}>
          <path d="M0.5 5h9" />
        </svg>
      </WindowControl>
      <WindowControl
        label="Toggle maximize"
        tooltip="Maximize"
        onClick={() => void appWindow.toggleMaximize()}
      >
        <svg {...GLYPH_PROPS}>
          <rect x="1" y="1" width="8" height="8" rx="1" />
        </svg>
      </WindowControl>
      <WindowControl
        label="Close window"
        tooltip="Close"
        isClose
        onClick={() => void appWindow.close()}
      >
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
  /** Current sidebar rail state; drives the toggle icon and tooltip. */
  sidebarCollapsed: boolean;
  /** Collapses/expands the sidebar rail (state owned by App). */
  onToggleSidebar: () => void;
};

/** Custom titlebar: sidebar toggle + hamburger menu + command-palette search on
 * the left, window controls on the right (Tauri only). The middle stays a drag
 * region. */
export function Titlebar({
  onNavigate,
  onOpenPalette,
  sidebarCollapsed,
  onToggleSidebar
}: TitlebarProps) {
  const queryClient = useQueryClient();
  const { setting, setSetting } = useTheme();
  const { data: status } = useAppStatus();
  const [isActivityOpen, setIsActivityOpen] = useState(false);

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
      // Positioned above the modal layer so the custom bar and its window
      // controls stay clickable while a dialog is open (see --z-titlebar).
      // Opaque background: the modal overlay starts BELOW the titlebar
      // (top: var(--titlebar-h)), so the bar is never dimmed and reads as a
      // solid strip. flex-nowrap keeps the fixed h-10 height at any window
      // width; the drag region below absorbs the shrink.
      style={{ zIndex: "var(--z-titlebar)" }}
      className="relative flex h-10 shrink-0 select-none flex-nowrap items-stretch border-b border-edge bg-bg"
    >
      <div className="flex h-full shrink-0 items-stretch whitespace-nowrap">
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  aria-label="Application menu"
                  className={TITLEBAR_BUTTON_CLASS}
                >
                  <svg {...MENU_GLYPH_PROPS}>
                    <path d="M1.5 3h9M1.5 6h9M1.5 9h9" />
                  </svg>
                </button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent side="bottom">Menu</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="start">{renderMenuEntries(menu)}</DropdownMenuContent>
        </DropdownMenu>

        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              aria-label={sidebarCollapsed ? "Expand Sidebar" : "Collapse Sidebar"}
              aria-pressed={sidebarCollapsed}
              onClick={onToggleSidebar}
              className={TITLEBAR_BUTTON_CLASS}
            >
              {sidebarCollapsed ? (
                <PanelLeft size={15} strokeWidth={1.75} aria-hidden />
              ) : (
                <PanelLeftClose size={15} strokeWidth={1.75} aria-hidden />
              )}
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {sidebarCollapsed ? "Expand Sidebar" : "Collapse Sidebar"}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
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
          </TooltipTrigger>
          <TooltipContent side="bottom">
            Search
            <TooltipHint>Ctrl K</TooltipHint>
          </TooltipContent>
        </Tooltip>
      </div>

      {/* Draggable middle: flex-1 min-w-0 so it soaks up all remaining width and
       * shrinks first, keeping the control clusters from wrapping to a second
       * row at narrow window widths. */}
      <div data-tauri-drag-region className="h-full min-w-0 flex-1" />

      {/* Right cluster: activity button (always) then window controls (Tauri
       * only), with extra spacing so activity reads as its own control. The
       * relative wrapper anchors the right-aligned dropdown under the bell and
       * is treated as "inside" so toggling via the button never double-fires. */}
      <div className="flex h-full shrink-0 items-stretch whitespace-nowrap">
        <div className={cn("relative flex h-full items-stretch", isTauri && CONTROL_CLUSTER_GAP)}>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label="Activity"
                aria-expanded={isActivityOpen}
                onClick={() => setIsActivityOpen((current) => !current)}
                className={cn(TITLEBAR_BUTTON_CLASS, isActivityOpen && "bg-surface text-ink")}
              >
                <Bell size={14} strokeWidth={1.5} aria-hidden />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">Activity</TooltipContent>
          </Tooltip>
          <AuditDropdown open={isActivityOpen} onOpenChange={setIsActivityOpen} />
        </div>
        {isTauri ? <WindowControls /> : null}
      </div>
    </header>
  );
}
