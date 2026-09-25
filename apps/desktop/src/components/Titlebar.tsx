import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useQueryClient } from "@tanstack/react-query";
import { Bell, Check, Menu, PanelLeft, PanelLeftClose, Search } from "lucide-react";
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
import { useTranslation } from "../i18n/useTranslation.js";
import { isTauri } from "../lib/ipc.js";
import { buildAppMenu, type MenuEntry } from "../lib/menu.js";
import type { ScreenId } from "../lib/nav.js";
import { AuditDropdown } from "./AuditDropdown.js";

// Gap that separates the activity button from the window-control cluster so it
// reads as a distinct control rather than another window button.
const CONTROL_CLUSTER_GAP = "mr-2";

// Shared 46px hover zone so menu/search buttons read like the window controls.
// aria-expanded mirrors the hover look while a button's popup (app menu,
// activity feed) is open; Radix sets it on the menu trigger, the bell sets it
// by hand.
const TITLEBAR_BUTTON_CLASS = cn(
  "flex h-full w-[46px] items-center justify-center text-ink-muted transition-colors",
  "hover:bg-surface hover:text-ink aria-expanded:bg-surface aria-expanded:text-ink",
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

// One size and stroke for every titlebar tool icon (menu, sidebar, search,
// bell), all from lucide, so the left and right clusters match. The window
// controls keep their thinner 10px GLYPH_PROPS, like native caption buttons.
const TITLEBAR_ICON_PROPS = {
  size: 16,
  strokeWidth: 1.5,
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

function WindowControls({ t }: { t: ReturnType<typeof useTranslation>["t"] }) {
  const appWindow = getCurrentWindow();
  return (
    <div className="flex h-full items-stretch">
      <WindowControl
        label={t("titlebar.minimizeWindow")}
        tooltip={t("titlebar.minimize")}
        onClick={() => void appWindow.minimize()}
      >
        <svg {...GLYPH_PROPS}>
          <path d="M0.5 5h9" />
        </svg>
      </WindowControl>
      <WindowControl
        label={t("titlebar.toggleMaximize")}
        tooltip={t("titlebar.maximize")}
        onClick={() => void appWindow.toggleMaximize()}
      >
        <svg {...GLYPH_PROPS}>
          <rect x="1" y="1" width="8" height="8" rx="1" />
        </svg>
      </WindowControl>
      <WindowControl
        label={t("titlebar.closeWindow")}
        tooltip={t("titlebar.close")}
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

// Last input seen anywhere in the window, shared by every titlebar tooltip.
// Captured on window so it is known before any focus handler runs.
const lastInput = { tab: false, x: -1, y: -1 };
let isInputTrackerInstalled = false;

function installInputTracker(): void {
  if (isInputTrackerInstalled) {
    return;
  }
  isInputTrackerInstalled = true;
  const onPointer = (event: PointerEvent) => {
    lastInput.tab = false;
    lastInput.x = event.clientX;
    lastInput.y = event.clientY;
  };
  const onPointerMove = (event: PointerEvent) => {
    lastInput.x = event.clientX;
    lastInput.y = event.clientY;
  };
  window.addEventListener("pointerdown", onPointer, { capture: true, passive: true });
  window.addEventListener("pointermove", onPointerMove, { capture: true, passive: true });
  window.addEventListener(
    "keydown",
    (event: KeyboardEvent) => {
      lastInput.tab = event.key === "Tab";
    },
    { capture: true }
  );
}

/**
 * Tooltip state for a titlebar button, so its hint shows only on a genuine
 * hover or keyboard (Tab) focus. Radix Tooltip alone also opened it:
 *  - while the button's popup (app menu, activity feed) was open;
 *  - when the menu or command palette returned focus to the button on close
 *    (Escape, item select, click outside);
 *  - on the first pointer move after a modal menu closed with the pointer
 *    still on the button (the modal layer's pointer-events:none makes the
 *    browser replay the pointer entering it).
 * `onPopupClose` must be called from the popup's close handler, synchronously,
 * because the focus return can fire in the same commit.
 */
function useTitlebarTooltip(popupOpen = false) {
  const [isOpen, setIsOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const sourceRef = useRef<"focus" | "hover">("hover");
  const suppressHoverRef = useRef(false);

  useEffect(installInputTracker, []);

  const onPopupClose = useCallback(() => {
    const rect = triggerRef.current?.getBoundingClientRect();
    suppressHoverRef.current =
      rect !== undefined &&
      lastInput.x >= rect.left &&
      lastInput.x <= rect.right &&
      lastInput.y >= rect.top &&
      lastInput.y <= rect.bottom;
    setIsOpen(false);
  }, []);

  const onOpenChange = (next: boolean) => {
    if (next) {
      const blocked =
        popupOpen || (sourceRef.current === "focus" ? !lastInput.tab : suppressHoverRef.current);
      if (blocked) {
        return;
      }
    }
    setIsOpen(next);
  };

  return {
    onPopupClose,
    tooltipProps: { open: isOpen && !popupOpen, onOpenChange },
    // Spread on the <button>; Radix Slot runs these before its own handlers,
    // so the source is known when the tooltip asks to open.
    triggerProps: {
      ref: triggerRef,
      onFocus: () => {
        sourceRef.current = "focus";
      },
      onPointerMove: () => {
        sourceRef.current = "hover";
      },
      onPointerLeave: () => {
        suppressHoverRef.current = false;
      }
    }
  };
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
  const { t } = useTranslation();
  const [isActivityOpen, setIsActivityOpen] = useState(false);
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const menuTooltip = useTitlebarTooltip(isMenuOpen);
  const sidebarTooltip = useTitlebarTooltip();
  const searchTooltip = useTitlebarTooltip();
  const activityTooltip = useTitlebarTooltip(isActivityOpen);
  const { onPopupClose: onMenuClose } = menuTooltip;
  const { onPopupClose: onActivityClose } = activityTooltip;

  const handleMenuOpenChange = useCallback(
    (open: boolean) => {
      if (!open) {
        onMenuClose();
      }
      setIsMenuOpen(open);
    },
    [onMenuClose]
  );
  // Stable identity: AuditDropdown re-binds its dismiss listeners on change.
  const handleActivityOpenChange = useCallback(
    (open: boolean) => {
      if (!open) {
        onActivityClose();
      }
      setIsActivityOpen(open);
    },
    [onActivityClose]
  );

  const copyVersion = () => {
    const version = status?.appVersion;
    if (!version) {
      toast.error(t("titlebar.versionUnavailable"), {
        description: t("titlebar.versionUnavailableDesc")
      });
      return;
    }
    navigator.clipboard
      .writeText(version)
      .then(() => toast.success(t("titlebar.versionCopied"), { description: version }))
      .catch((error: unknown) =>
        toast.error(t("titlebar.copyFailed"), {
          description: error instanceof Error ? error.message : String(error)
        })
      );
  };

  const menu = buildAppMenu({
    navigate: onNavigate,
    reloadData: () => {
      void queryClient.invalidateQueries();
      toast.info(t("titlebar.reloadingData"));
    },
    quit: isTauri ? () => void getCurrentWindow().close() : null,
    themeSetting: setting,
    setTheme: setSetting,
    copyVersion,
    t
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
        <DropdownMenu open={isMenuOpen} onOpenChange={handleMenuOpenChange}>
          <Tooltip {...menuTooltip.tooltipProps}>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  aria-label={t("titlebar.applicationMenu")}
                  {...menuTooltip.triggerProps}
                  className={TITLEBAR_BUTTON_CLASS}
                >
                  <Menu {...TITLEBAR_ICON_PROPS} />
                </button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent side="bottom">{t("titlebar.menu")}</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="start">{renderMenuEntries(menu)}</DropdownMenuContent>
        </DropdownMenu>

        <Tooltip {...sidebarTooltip.tooltipProps}>
          <TooltipTrigger asChild>
            <button
              type="button"
              {...sidebarTooltip.triggerProps}
              aria-label={
                sidebarCollapsed ? t("titlebar.expandSidebar") : t("titlebar.collapseSidebar")
              }
              aria-pressed={sidebarCollapsed}
              onClick={onToggleSidebar}
              className={TITLEBAR_BUTTON_CLASS}
            >
              {sidebarCollapsed ? (
                <PanelLeft {...TITLEBAR_ICON_PROPS} />
              ) : (
                <PanelLeftClose {...TITLEBAR_ICON_PROPS} />
              )}
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {sidebarCollapsed ? t("titlebar.expandSidebar") : t("titlebar.collapseSidebar")}
          </TooltipContent>
        </Tooltip>

        <Tooltip {...searchTooltip.tooltipProps}>
          <TooltipTrigger asChild>
            <button
              type="button"
              {...searchTooltip.triggerProps}
              aria-label={t("titlebar.commandPalette")}
              onClick={onOpenPalette}
              className={TITLEBAR_BUTTON_CLASS}
            >
              <Search {...TITLEBAR_ICON_PROPS} />
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {t("titlebar.search")}
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
          <Tooltip {...activityTooltip.tooltipProps}>
            <TooltipTrigger asChild>
              <button
                type="button"
                {...activityTooltip.triggerProps}
                aria-label={t("titlebar.activity")}
                aria-expanded={isActivityOpen}
                onClick={() => handleActivityOpenChange(!isActivityOpen)}
                className={TITLEBAR_BUTTON_CLASS}
              >
                <Bell {...TITLEBAR_ICON_PROPS} />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">{t("titlebar.activity")}</TooltipContent>
          </Tooltip>
          <AuditDropdown open={isActivityOpen} onOpenChange={handleActivityOpenChange} />
        </div>
        {isTauri ? <WindowControls t={t} /> : null}
      </div>
    </header>
  );
}
