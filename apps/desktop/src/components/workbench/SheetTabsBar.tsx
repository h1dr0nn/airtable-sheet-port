import { cn, FOCUS_RING } from "@sheet-port/ui";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { SheetTab } from "../../lib/ipc.js";

type SheetTabsBarProps = {
  tabs: SheetTab[];
  activeGid: string | null;
  onSelect: (gid: string) => void;
};

/**
 * Google-Sheets-style bottom tab strip. Read-only for v1: it switches the
 * active sheet but does not create, rename, or reorder tabs.
 */
export function SheetTabsBar({ tabs, activeGid, onSelect }: SheetTabsBarProps) {
  const { t } = useTranslation();

  return (
    <div
      role="tablist"
      aria-label={t("workbench.sheetTabs")}
      className="flex shrink-0 items-stretch gap-1 overflow-x-auto border-t border-edge bg-surface px-2 py-1"
    >
      {tabs.map((tab) => {
        const isActive = tab.gid === activeGid;
        return (
          <button
            key={tab.gid}
            type="button"
            role="tab"
            aria-selected={isActive}
            onClick={() => onSelect(tab.gid)}
            className={cn(
              "whitespace-nowrap rounded-md px-3 py-1.5 text-[12.5px] font-medium transition-colors",
              FOCUS_RING,
              isActive
                ? "bg-raised text-accent shadow-card"
                : "text-ink-muted hover:bg-raised/60 hover:text-ink"
            )}
          >
            {tab.title}
          </button>
        );
      })}
    </div>
  );
}
