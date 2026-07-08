import { Skeleton } from "@sheet-port/ui";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { WorkbenchItem } from "../../lib/ipc.js";
import { queryKeys } from "../../lib/queryKeys.js";
import {
  useAppendSheetRow,
  useSheet,
  useSheetTabs,
  useUpdateCell
} from "../../hooks/useWorkbench.js";
import { useSheetHistory } from "../../hooks/useSheetHistory.js";
import { SheetGrid } from "./SheetGrid.js";
import { SheetTabsBar } from "./SheetTabsBar.js";
import { WorkbenchToolbar } from "./WorkbenchToolbar.js";

/** True when focus sits in an editable field or an open dialog, where the
 * app's undo/redo shortcuts must yield to the browser's native handling. */
function isTypingContext(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.isContentEditable) {
    return true;
  }
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") {
    return true;
  }
  // Any open dialog (e.g. Add Spreadsheet) owns the keyboard while it is up.
  return document.querySelector('[role="dialog"]') !== null;
}

type SheetWorkAreaProps = {
  item: WorkbenchItem;
};

/** Right pane for the selected spreadsheet: toolbar, editable grid, tab strip. */
export function SheetWorkArea({ item }: SheetWorkAreaProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [activeGid, setActiveGid] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const tabsQuery = useSheetTabs(item.id);
  const tabs = tabsQuery.data ?? [];

  // Reset per-spreadsheet view state whenever the selection changes.
  useEffect(() => {
    setActiveGid(null);
    setQuery("");
  }, [item.id]);

  // Default to the first tab (or recover if the active tab disappears).
  useEffect(() => {
    const firstTab = tabs[0];
    if (!firstTab) {
      return;
    }
    const stillExists = activeGid !== null && tabs.some((tab) => tab.gid === activeGid);
    if (!stillExists) {
      setActiveGid(firstTab.gid);
    }
  }, [tabs, activeGid]);

  const sheetQuery = useSheet(item.id, activeGid);
  const updateCell = useUpdateCell();
  const appendRow = useAppendSheetRow();

  const activeTab = tabs.find((tab) => tab.gid === activeGid) ?? null;
  const grid = sheetQuery.data ?? null;

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.sheetTabs(item.id) });
    if (activeGid !== null) {
      void queryClient.invalidateQueries({ queryKey: queryKeys.sheet(item.id, activeGid) });
    }
  };

  const addRow = () => {
    if (activeGid === null) {
      return;
    }
    appendRow.mutate({ itemId: item.id, gid: activeGid, values: {} });
  };

  // Writes a cell without touching history; the shared path for user edits,
  // undo, and redo. History records only around user edits (see editCell).
  const applyCell = useCallback(
    (rowIndex: number, columnId: string, value: string) => {
      if (activeGid === null) {
        return;
      }
      updateCell.mutate({ itemId: item.id, gid: activeGid, rowIndex, columnId, value });
    },
    [activeGid, item.id, updateCell]
  );

  // History resets whenever the active spreadsheet or sheet tab changes.
  const historyScope = `${item.id}:${activeGid ?? ""}`;
  const history = useSheetHistory(historyScope, applyCell);
  const { record, undo, redo, canUndo, canRedo } = history;

  const editCell = useCallback(
    (rowIndex: number, columnId: string, value: string) => {
      if (activeGid === null || !grid) {
        return;
      }
      const prevValue = grid.rows[rowIndex]?.[columnId] ?? "";
      applyCell(rowIndex, columnId, value);
      record({ rowIndex, columnId, prevValue, nextValue: value });
    },
    [activeGid, grid, applyCell, record]
  );

  // Ctrl/Cmd+Z undoes, Ctrl+Y or Ctrl/Cmd+Shift+Z redoes. Suppressed while the
  // user is typing in a field or a dialog is open so native undo still works.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) {
        return;
      }
      const key = event.key.toLowerCase();
      const isUndo = key === "z" && !event.shiftKey;
      const isRedo = key === "y" || (key === "z" && event.shiftKey);
      if (!isUndo && !isRedo) {
        return;
      }
      if (isTypingContext(event.target)) {
        return;
      }
      event.preventDefault();
      if (isUndo) {
        undo();
      } else {
        redo();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [undo, redo]);

  const isRefreshing = tabsQuery.isFetching || sheetQuery.isFetching;
  const showGridSkeleton = activeGid === null || (sheetQuery.isLoading && !grid);

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <WorkbenchToolbar
        spreadsheetName={item.name}
        sheetName={activeTab?.title ?? null}
        query={query}
        onQueryChange={setQuery}
        onRefresh={refresh}
        isRefreshing={isRefreshing}
        onAddRow={addRow}
        canAddRow={grid !== null && activeGid !== null}
        onUndo={undo}
        onRedo={redo}
        canUndo={canUndo}
        canRedo={canRedo}
      />

      <div className="relative min-h-0 flex-1 bg-raised">
        {showGridSkeleton ? (
          <div className="flex flex-col gap-2 p-3">
            <Skeleton className="h-8 w-full" />
            {Array.from({ length: 8 }).map((_, index) => (
              <Skeleton key={index} className="h-7 w-full" />
            ))}
          </div>
        ) : grid ? (
          <SheetGrid grid={grid} query={query} onEditCell={editCell} />
        ) : (
          <p className="p-6 text-center text-[13px] text-ink-muted">{t("workbench.sheetLoadError")}</p>
        )}
      </div>

      <SheetTabsBar tabs={tabs} activeGid={activeGid} onSelect={setActiveGid} />
    </div>
  );
}
