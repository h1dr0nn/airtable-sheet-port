import {
  Button,
  cn,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  FOCUS_RING,
  Skeleton
} from "@sheet-port/ui";
import { FilePlus2, FolderPlus, Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { WorkbenchFolder, WorkbenchItem } from "../../lib/ipc.js";
import {
  useAddSpreadsheet,
  useCreateFolder,
  useDeleteFolder,
  useMoveWorkbenchItem,
  useRemoveWorkbenchItem,
  useRenameFolder
} from "../../hooks/useWorkbench.js";
import { ConfirmDialog } from "../ConfirmDialog.js";
import { AddSpreadsheetDialog } from "./AddSpreadsheetDialog.js";
import { FolderNameDialog } from "./FolderNameDialog.js";
import { FolderTree } from "./FolderTree.js";

type WorkbenchSidebarProps = {
  folders: WorkbenchFolder[];
  items: WorkbenchItem[];
  isLoading: boolean;
  selectedItemId: string | null;
  onSelectItem: (id: string) => void;
  /** Notifies the parent so it can clear/replace the active selection. */
  onItemRemoved: (id: string) => void;
  onSpreadsheetAdded: (item: WorkbenchItem) => void;
};

/** Left secondary pane: header actions, search, the folder tree, and dialogs. */
export function WorkbenchSidebar({
  folders,
  items,
  isLoading,
  selectedItemId,
  onSelectItem,
  onItemRemoved,
  onSpreadsheetAdded
}: WorkbenchSidebarProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const [newFolderOpen, setNewFolderOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<WorkbenchFolder | null>(null);
  const [deleteFolderTarget, setDeleteFolderTarget] = useState<WorkbenchFolder | null>(null);
  const [removeItemTarget, setRemoveItemTarget] = useState<WorkbenchItem | null>(null);

  const createFolder = useCreateFolder();
  const renameFolder = useRenameFolder();
  const deleteFolder = useDeleteFolder();
  const addSpreadsheet = useAddSpreadsheet();
  const removeItem = useRemoveWorkbenchItem();
  const moveItem = useMoveWorkbenchItem();

  const trimmedQuery = query.trim().toLowerCase();
  const isSearching = trimmedQuery !== "";

  const folderNameById = useMemo(() => {
    const map = new Map<string, string>();
    folders.forEach((folder) => map.set(folder.id, folder.name.toLowerCase()));
    return map;
  }, [folders]);

  // Items surviving the search: match the sheet name or its parent folder name.
  const visibleItems = useMemo(() => {
    if (!isSearching) {
      return items;
    }
    return items.filter((item) => {
      const nameHit = item.name.toLowerCase().includes(trimmedQuery);
      const folderHit =
        item.folderId !== null && (folderNameById.get(item.folderId) ?? "").includes(trimmedQuery);
      return nameHit || folderHit;
    });
  }, [items, isSearching, trimmedQuery, folderNameById]);

  // Folders surviving the search: match by name or by owning a visible item.
  const visibleFolders = useMemo(() => {
    if (!isSearching) {
      return folders;
    }
    return folders.filter(
      (folder) =>
        folder.name.toLowerCase().includes(trimmedQuery) ||
        visibleItems.some((item) => item.folderId === folder.id)
    );
  }, [folders, isSearching, trimmedQuery, visibleItems]);

  const expandedFolders = useMemo(
    () => new Set(folders.filter((folder) => !collapsed.has(folder.id)).map((folder) => folder.id)),
    [folders, collapsed]
  );

  const toggleFolder = (id: string) => {
    setCollapsed((previous) => {
      const next = new Set(previous);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const isEmpty = !isLoading && folders.length === 0 && items.length === 0;
  const hasNoResults = !isLoading && !isEmpty && visibleFolders.length === 0 && visibleItems.length === 0;

  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-edge bg-bg">
      <header className="flex items-center justify-between gap-2 border-b border-edge px-3 py-2.5">
        <h2 className="text-[13px] font-semibold text-ink">{t("workbench.title")}</h2>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" aria-label={t("workbench.addMenu")}>
              <Plus size={16} strokeWidth={2} aria-hidden />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => setNewFolderOpen(true)}>
              <FolderPlus size={14} strokeWidth={1.75} aria-hidden className="text-ink-muted" />
              {t("workbench.newFolder")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => setAddOpen(true)}>
              <FilePlus2 size={14} strokeWidth={1.75} aria-hidden className="text-ink-muted" />
              {t("workbench.addSpreadsheet")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </header>

      <div className="border-b border-edge px-3 py-2">
        <div className="relative">
          <Search
            size={14}
            strokeWidth={1.75}
            aria-hidden
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-faint"
          />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("workbench.searchPlaceholder")}
            aria-label={t("workbench.searchPlaceholder")}
            className={cn(
              "h-8 w-full rounded-md border border-edge-strong bg-surface pl-8 pr-3 text-[13px] text-ink",
              "placeholder:text-ink-faint transition-colors hover:border-ink-faint",
              FOCUS_RING
            )}
          />
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {isLoading ? (
          <div className="flex flex-col gap-2 px-1 py-1">
            <Skeleton className="h-7 w-full" />
            <Skeleton className="h-7 w-11/12" />
            <Skeleton className="h-7 w-10/12" />
            <Skeleton className="h-7 w-full" />
          </div>
        ) : isEmpty ? (
          <EmptyState
            className="mt-4 border-none bg-transparent px-2 py-8"
            title={t("workbench.emptyTitle")}
            description={t("workbench.emptyDescription")}
            action={
              <Button size="sm" onClick={() => setAddOpen(true)}>
                <Plus size={14} strokeWidth={2} aria-hidden />
                {t("workbench.addSpreadsheet")}
              </Button>
            }
          />
        ) : hasNoResults ? (
          <p className="px-2 py-6 text-center text-[12.5px] text-ink-muted">
            {t("workbench.noResults")}
          </p>
        ) : (
          <FolderTree
            folders={visibleFolders}
            items={visibleItems}
            selectedItemId={selectedItemId}
            expandedFolders={expandedFolders}
            expandAll={isSearching}
            onToggleFolder={toggleFolder}
            onSelectItem={onSelectItem}
            onRenameFolder={setRenameTarget}
            onDeleteFolder={setDeleteFolderTarget}
            onRemoveItem={setRemoveItemTarget}
            onMoveItem={(item, folderId) => moveItem.mutate({ id: item.id, folderId })}
          />
        )}
      </div>

      <FolderNameDialog
        open={newFolderOpen}
        onOpenChange={setNewFolderOpen}
        title={t("workbench.newFolder")}
        submitLabel={t("workbench.create")}
        isPending={createFolder.isPending}
        onSubmit={(name) =>
          createFolder.mutate(name, { onSuccess: () => setNewFolderOpen(false) })
        }
      />

      <FolderNameDialog
        open={renameTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setRenameTarget(null);
          }
        }}
        title={t("workbench.renameFolder")}
        submitLabel={t("common.save")}
        initialName={renameTarget?.name ?? ""}
        isPending={renameFolder.isPending}
        onSubmit={(name) => {
          if (renameTarget) {
            renameFolder.mutate(
              { id: renameTarget.id, name },
              { onSuccess: () => setRenameTarget(null) }
            );
          }
        }}
      />

      <AddSpreadsheetDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        folders={folders}
        defaultFolderId={null}
        isPending={addSpreadsheet.isPending}
        onSubmit={(input) =>
          addSpreadsheet.mutate(input, {
            onSuccess: (item) => {
              onSpreadsheetAdded(item);
              setAddOpen(false);
            }
          })
        }
      />

      <ConfirmDialog
        open={deleteFolderTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteFolderTarget(null);
          }
        }}
        title={t("workbench.deleteFolderTitle")}
        description={t("workbench.deleteFolderDescription", { name: deleteFolderTarget?.name ?? "" })}
        confirmLabel={t("workbench.delete")}
        isPending={deleteFolder.isPending}
        onConfirm={() => {
          if (deleteFolderTarget) {
            deleteFolder.mutate(deleteFolderTarget.id, {
              onSuccess: () => setDeleteFolderTarget(null)
            });
          }
        }}
      />

      <ConfirmDialog
        open={removeItemTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setRemoveItemTarget(null);
          }
        }}
        title={t("workbench.removeItemTitle")}
        description={t("workbench.removeItemDescription", { name: removeItemTarget?.name ?? "" })}
        confirmLabel={t("workbench.remove")}
        isPending={removeItem.isPending}
        onConfirm={() => {
          if (removeItemTarget) {
            const removedId = removeItemTarget.id;
            removeItem.mutate(removedId, {
              onSuccess: () => {
                onItemRemoved(removedId);
                setRemoveItemTarget(null);
              }
            });
          }
        }}
      />
    </aside>
  );
}
