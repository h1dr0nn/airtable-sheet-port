import { Button, cn, FOCUS_RING, Tooltip, TooltipContent, TooltipTrigger } from "@sheet-port/ui";
import { Plus, RefreshCw, Search } from "lucide-react";
import { useTranslation } from "../../i18n/useTranslation.js";

type WorkbenchToolbarProps = {
  spreadsheetName: string;
  sheetName: string | null;
  query: string;
  onQueryChange: (value: string) => void;
  onRefresh: () => void;
  isRefreshing: boolean;
  onAddRow: () => void;
  canAddRow: boolean;
};

/**
 * Work-area top bar: in-sheet find, a refresh control, an add-row action, and a
 * muted "Spreadsheet / Sheet" breadcrumb on the right.
 */
export function WorkbenchToolbar({
  spreadsheetName,
  sheetName,
  query,
  onQueryChange,
  onRefresh,
  isRefreshing,
  onAddRow,
  canAddRow
}: WorkbenchToolbarProps) {
  const { t } = useTranslation();

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-edge bg-raised px-3 py-2">
      <div className="relative min-w-[180px] flex-1">
        <Search
          size={14}
          strokeWidth={1.75}
          aria-hidden
          className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-faint"
        />
        <input
          type="search"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder={t("workbench.findInSheet")}
          aria-label={t("workbench.findInSheet")}
          className={cn(
            "h-8 w-full rounded-md border border-edge-strong bg-bg pl-8 pr-3 text-[13px] text-ink",
            "placeholder:text-ink-faint transition-colors hover:border-ink-faint",
            FOCUS_RING
          )}
        />
      </div>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="secondary"
            size="sm"
            onClick={onRefresh}
            disabled={isRefreshing}
            aria-label={t("workbench.refresh")}
          >
            <RefreshCw
              size={14}
              strokeWidth={1.75}
              aria-hidden
              className={isRefreshing ? "motion-safe:animate-spin" : undefined}
            />
            <span className="hidden sm:inline">{t("workbench.refresh")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("workbench.refresh")}</TooltipContent>
      </Tooltip>

      <Button size="sm" onClick={onAddRow} disabled={!canAddRow}>
        <Plus size={14} strokeWidth={2} aria-hidden />
        <span className="hidden sm:inline">{t("workbench.addRow")}</span>
      </Button>

      <p className="ml-auto hidden max-w-[45%] items-center gap-1 truncate text-[12px] text-ink-muted md:flex">
        <span className="truncate font-medium text-ink">{spreadsheetName}</span>
        {sheetName ? (
          <>
            <span className="text-ink-faint">/</span>
            <span className="truncate">{sheetName}</span>
          </>
        ) : null}
      </p>
    </div>
  );
}
