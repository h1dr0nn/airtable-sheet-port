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
  FOCUS_RING
} from "@sheet-port/ui";
import { ChevronDown, ChevronRight, FileSpreadsheet, MoreHorizontal } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { WorkbenchFolder, WorkbenchItem } from "../../lib/ipc.js";

export type FolderTreeProps = {
  folders: WorkbenchFolder[];
  /** Items already filtered by the search query. */
  items: WorkbenchItem[];
  selectedItemId: string | null;
  expandedFolders: Set<string>;
  /** When true (active search), every folder renders expanded. */
  expandAll: boolean;
  onToggleFolder: (id: string) => void;
  onSelectItem: (id: string) => void;
  onRenameFolder: (folder: WorkbenchFolder) => void;
  onDeleteFolder: (folder: WorkbenchFolder) => void;
  onRemoveItem: (item: WorkbenchItem) => void;
  onMoveItem: (item: WorkbenchItem, folderId: string | null) => void;
};

/** The curated spreadsheet tree: folders with their sheets, plus Ungrouped. */
export function FolderTree({
  folders,
  items,
  selectedItemId,
  expandedFolders,
  expandAll,
  onToggleFolder,
  onSelectItem,
  onRenameFolder,
  onDeleteFolder,
  onRemoveItem,
  onMoveItem
}: FolderTreeProps) {
  const { t } = useTranslation();
  const ungrouped = items.filter((item) => item.folderId === null);

  return (
    <div className="flex flex-col gap-0.5">
      {folders.map((folder) => {
        const folderItems = items.filter((item) => item.folderId === folder.id);
        const isExpanded = expandAll || expandedFolders.has(folder.id);
        return (
          <div key={folder.id}>
            <div className="group/folder flex items-center gap-1">
              <button
                type="button"
                onClick={() => onToggleFolder(folder.id)}
                aria-expanded={isExpanded}
                className={cn(
                  "flex min-w-0 flex-1 items-center gap-1.5 rounded-md px-2 py-1.5 text-left",
                  "text-[13px] font-medium text-ink transition-colors hover:bg-surface",
                  FOCUS_RING
                )}
              >
                {isExpanded ? (
                  <ChevronDown size={14} strokeWidth={1.75} aria-hidden className="shrink-0 text-ink-faint" />
                ) : (
                  <ChevronRight size={14} strokeWidth={1.75} aria-hidden className="shrink-0 text-ink-faint" />
                )}
                <span className="truncate">{folder.name}</span>
                <span className="ml-auto shrink-0 text-[11px] tabular-nums text-ink-faint">
                  {folderItems.length}
                </span>
              </button>
              <RowMenu label={t("workbench.folderMenu", { name: folder.name })}>
                <DropdownMenuItem onSelect={() => onRenameFolder(folder)}>
                  {t("workbench.rename")}
                </DropdownMenuItem>
                <DropdownMenuItem
                  className="text-danger data-[highlighted]:text-danger"
                  onSelect={() => onDeleteFolder(folder)}
                >
                  {t("workbench.delete")}
                </DropdownMenuItem>
              </RowMenu>
            </div>

            {isExpanded ? (
              <div className="ml-3 flex flex-col gap-0.5 border-l border-edge pl-1.5">
                {folderItems.length === 0 ? (
                  <p className="px-2 py-1.5 text-[12px] text-ink-faint">{t("workbench.emptyFolder")}</p>
                ) : (
                  folderItems.map((item) => (
                    <ItemRow
                      key={item.id}
                      item={item}
                      folders={folders}
                      isSelected={item.id === selectedItemId}
                      onSelect={onSelectItem}
                      onRemove={onRemoveItem}
                      onMove={onMoveItem}
                    />
                  ))
                )}
              </div>
            ) : null}
          </div>
        );
      })}

      {ungrouped.length > 0 ? (
        <div className="mt-1">
          <p className="px-2 py-1.5 text-[11px] font-semibold uppercase tracking-[0.04em] text-ink-faint">
            {t("workbench.ungrouped")}
          </p>
          <div className="flex flex-col gap-0.5">
            {ungrouped.map((item) => (
              <ItemRow
                key={item.id}
                item={item}
                folders={folders}
                isSelected={item.id === selectedItemId}
                onSelect={onSelectItem}
                onRemove={onRemoveItem}
                onMove={onMoveItem}
              />
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

type ItemRowProps = {
  item: WorkbenchItem;
  folders: WorkbenchFolder[];
  isSelected: boolean;
  onSelect: (id: string) => void;
  onRemove: (item: WorkbenchItem) => void;
  onMove: (item: WorkbenchItem, folderId: string | null) => void;
};

function ItemRow({ item, folders, isSelected, onSelect, onRemove, onMove }: ItemRowProps) {
  const { t } = useTranslation();
  return (
    <div
      className={cn(
        "group/item flex items-center gap-1 rounded-md pr-1 transition-colors",
        isSelected ? "bg-accent/[0.09]" : "hover:bg-surface"
      )}
    >
      <button
        type="button"
        onClick={() => onSelect(item.id)}
        aria-current={isSelected ? "true" : undefined}
        className={cn(
          "flex min-w-0 flex-1 items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[12.5px]",
          FOCUS_RING,
          isSelected ? "font-medium text-accent" : "text-ink-muted"
        )}
      >
        <FileSpreadsheet
          size={13}
          strokeWidth={1.75}
          aria-hidden
          className={cn("shrink-0", isSelected ? "text-accent" : "text-ink-faint")}
        />
        <span className="truncate">{item.name}</span>
      </button>
      <RowMenu label={t("workbench.itemMenu", { name: item.name })}>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>{t("workbench.moveToFolder")}</DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuItem
              disabled={item.folderId === null}
              onSelect={() => onMove(item, null)}
            >
              {t("workbench.ungrouped")}
            </DropdownMenuItem>
            {folders.length > 0 ? <DropdownMenuSeparator /> : null}
            {folders.map((folder) => (
              <DropdownMenuItem
                key={folder.id}
                disabled={item.folderId === folder.id}
                onSelect={() => onMove(item, folder.id)}
              >
                {folder.name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          className="text-danger data-[highlighted]:text-danger"
          onSelect={() => onRemove(item)}
        >
          {t("workbench.remove")}
        </DropdownMenuItem>
      </RowMenu>
    </div>
  );
}

/** Shared kebab trigger + menu shell; reveals on row hover / focus / open. */
function RowMenu({ label, children }: { label: string; children: ReactNode }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={label}
          className={cn(
            "flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-ink-faint",
            "opacity-0 transition-opacity hover:bg-edge/60 hover:text-ink",
            "group-hover/folder:opacity-100 group-hover/item:opacity-100",
            "focus-visible:opacity-100 data-[state=open]:opacity-100",
            FOCUS_RING
          )}
        >
          <MoreHorizontal size={15} strokeWidth={1.75} aria-hidden />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">{children}</DropdownMenuContent>
    </DropdownMenu>
  );
}
