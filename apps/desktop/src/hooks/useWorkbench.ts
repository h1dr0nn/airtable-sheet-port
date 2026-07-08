import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc, type AddSpreadsheetInput, type GridData } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

// React Query bindings for the Workbench. Reads are cached per item/sheet;
// writes toast on error and invalidate the affected queries. The cell edit is
// optimistic so typing feels instant, with a rollback + toast when it fails.

/** The user-curated folder tree plus every added spreadsheet. */
export function useWorkbenchTree() {
  return useQuery({
    queryKey: queryKeys.workbenchTree,
    queryFn: () => ipc.workbenchTree()
  });
}

/** Sheet tabs of one spreadsheet; disabled until an item is selected. */
export function useSheetTabs(itemId: string | null) {
  return useQuery({
    queryKey: queryKeys.sheetTabs(itemId ?? ""),
    queryFn: () => ipc.listSheetTabs(itemId ?? ""),
    enabled: itemId !== null
  });
}

/** Full grid for one sheet tab. v1 reads every row (no server paging). */
export function useSheet(itemId: string | null, gid: string | null) {
  return useQuery({
    queryKey: queryKeys.sheet(itemId ?? "", gid ?? ""),
    queryFn: () => ipc.readSheet(itemId ?? "", gid ?? "", null, null),
    enabled: itemId !== null && gid !== null,
    placeholderData: keepPreviousData
  });
}

export function useCreateFolder() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (name: string) => ipc.createFolder(name),
    onError: (error: unknown) => {
      toast.error(t("toast.folderCreateError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.folderCreated"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.workbenchTree });
    }
  });
}

export function useRenameFolder() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (input: { id: string; name: string }) => ipc.renameFolder(input.id, input.name),
    onError: (error: unknown) => {
      toast.error(t("toast.folderRenameError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.folderRenamed"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.workbenchTree });
    }
  });
}

export function useDeleteFolder() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (id: string) => ipc.deleteFolder(id),
    onError: (error: unknown) => {
      toast.error(t("toast.folderDeleteError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.folderDeleted"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.workbenchTree });
    }
  });
}

export function useAddSpreadsheet() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (input: AddSpreadsheetInput) => ipc.addSpreadsheet(input),
    onError: (error: unknown) => {
      toast.error(t("toast.spreadsheetAddError"), { description: getErrorMessage(error) });
    },
    onSuccess: (item) => {
      toast.success(t("toast.spreadsheetAdded"), { description: item.name });
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.workbenchTree });
    }
  });
}

export function useRemoveWorkbenchItem() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (id: string) => ipc.removeWorkbenchItem(id),
    onError: (error: unknown) => {
      toast.error(t("toast.spreadsheetRemoveError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.spreadsheetRemoved"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.workbenchTree });
    }
  });
}

export function useMoveWorkbenchItem() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (input: { id: string; folderId: string | null }) =>
      ipc.moveWorkbenchItem(input.id, input.folderId),
    onError: (error: unknown) => {
      toast.error(t("toast.spreadsheetMoveError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.spreadsheetMoved"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.workbenchTree });
    }
  });
}

export type UpdateCellInput = {
  itemId: string;
  gid: string;
  rowIndex: number;
  columnId: string;
  value: string;
};

/** Optimistic cell edit: patches the cached grid, rolls back on failure. */
export function useUpdateCell() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (input: UpdateCellInput) =>
      ipc.updateCell(input.itemId, input.gid, input.rowIndex, input.columnId, input.value),
    onMutate: async (input): Promise<{ previous: GridData | undefined }> => {
      const key = queryKeys.sheet(input.itemId, input.gid);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<GridData>(key);
      if (previous) {
        const rows = previous.rows.map((row, index) =>
          index === input.rowIndex ? { ...row, [input.columnId]: input.value } : row
        );
        queryClient.setQueryData<GridData>(key, { ...previous, rows });
      }
      return { previous };
    },
    onError: (error: unknown, input, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKeys.sheet(input.itemId, input.gid), context.previous);
      }
      toast.error(t("toast.cellUpdateError"), { description: getErrorMessage(error) });
    },
    onSettled: (_data, _error, input) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.sheet(input.itemId, input.gid) });
    }
  });
}

export type AppendRowInput = {
  itemId: string;
  gid: string;
  values: Record<string, string>;
};

/** Optimistic row append: shows the new row instantly, rolls back on failure. */
export function useAppendSheetRow() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation({
    mutationFn: (input: AppendRowInput) => ipc.appendSheetRow(input.itemId, input.gid, input.values),
    onMutate: async (input): Promise<{ previous: GridData | undefined }> => {
      const key = queryKeys.sheet(input.itemId, input.gid);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<GridData>(key);
      if (previous) {
        // Build the appended row from the sheet's columns so the grid keeps its
        // shape; unspecified cells default to empty, mirroring the backend.
        const appended: Record<string, string> = {};
        for (const column of previous.columns) {
          appended[column.id] = input.values[column.id] ?? "";
        }
        const rows = [...previous.rows, appended];
        queryClient.setQueryData<GridData>(key, { ...previous, rows, totalRows: rows.length });
      }
      return { previous };
    },
    onError: (error: unknown, input, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKeys.sheet(input.itemId, input.gid), context.previous);
      }
      toast.error(t("toast.rowAddError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.rowAdded"));
    },
    onSettled: (_data, _error, input) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.sheet(input.itemId, input.gid) });
    }
  });
}
