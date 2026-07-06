import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { TABLE_PAGE_SIZE } from "../lib/constants.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useTables(sourceId: string | null) {
  return useQuery({
    queryKey: queryKeys.tables(sourceId ?? ""),
    queryFn: () => ipc.listTables(sourceId ?? ""),
    enabled: sourceId !== null
  });
}

export function useTableSchema(sourceId: string | null, tableId: string | null) {
  return useQuery({
    queryKey: queryKeys.tableSchema(sourceId ?? "", tableId ?? ""),
    queryFn: () => ipc.describeTable(sourceId ?? "", tableId ?? ""),
    enabled: sourceId !== null && tableId !== null
  });
}

/** Zero-based page of TABLE_PAGE_SIZE records. */
export function useTablePage(sourceId: string | null, tableId: string | null, page: number) {
  return useQuery({
    queryKey: queryKeys.tablePage(sourceId ?? "", tableId ?? "", page),
    queryFn: () => ipc.readTable(sourceId ?? "", tableId ?? "", TABLE_PAGE_SIZE, page * TABLE_PAGE_SIZE),
    enabled: sourceId !== null && tableId !== null,
    placeholderData: keepPreviousData
  });
}
