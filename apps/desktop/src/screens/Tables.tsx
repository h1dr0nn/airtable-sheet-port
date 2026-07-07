import { EmptyState } from "@sheet-port/ui";
import { useMemo, useState } from "react";
import { useTranslation } from "../i18n/useTranslation.js";
import { useWorkbenchTree } from "../hooks/useWorkbench.js";
import { SheetWorkArea } from "../components/workbench/SheetWorkArea.js";
import { WorkbenchSidebar } from "../components/workbench/WorkbenchSidebar.js";

/**
 * The Workbench: a Google-Sheets-like workspace over a user-curated tree of
 * spreadsheets. Left pane is the folder tree + search; right pane is the
 * editable grid with a bottom sheet-tab strip. Fills the whole content area.
 *
 * Driven entirely by demo data in the browser preview; the Tauri path throws a
 * clear "not wired yet" error until the real backend lands.
 */
export function Tables() {
  const { t } = useTranslation();
  const treeQuery = useWorkbenchTree();
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);

  const folders = treeQuery.data?.folders ?? [];
  const items = treeQuery.data?.items ?? [];

  // Resolve the live selection; a removed item collapses back to no selection.
  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedItemId) ?? null,
    [items, selectedItemId]
  );

  return (
    <div className="app-workbench flex h-full w-full overflow-hidden bg-bg">
      <WorkbenchSidebar
        folders={folders}
        items={items}
        isLoading={treeQuery.isLoading}
        selectedItemId={selectedItemId}
        onSelectItem={setSelectedItemId}
        onItemRemoved={(id) => {
          setSelectedItemId((current) => (current === id ? null : current));
        }}
        onSpreadsheetAdded={(item) => setSelectedItemId(item.id)}
      />

      {selectedItem ? (
        <SheetWorkArea item={selectedItem} />
      ) : (
        <div className="flex min-w-0 flex-1 items-center justify-center bg-raised p-8">
          <EmptyState
            className="border-none bg-transparent"
            title={t("workbench.selectPromptTitle")}
            description={t("workbench.selectPromptDescription")}
          />
        </div>
      )}
    </div>
  );
}
