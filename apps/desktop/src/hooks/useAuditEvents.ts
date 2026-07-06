import { useInfiniteQuery } from "@tanstack/react-query";
import { AUDIT_PAGE_SIZE } from "../lib/constants.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

/** Pages of `pageSize` events; "load more" fetches the next offset. The page
 * size is part of the query key so callers requesting different sizes (e.g. the
 * dashboard preview vs. the titlebar dropdown) never share a cache entry. */
export function useAuditEvents(pageSize: number = AUDIT_PAGE_SIZE) {
  return useInfiniteQuery({
    queryKey: queryKeys.auditEventsPaged(pageSize),
    queryFn: ({ pageParam }) => ipc.listAuditEvents(pageSize, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) =>
      lastPage.length < pageSize ? undefined : allPages.length * pageSize
  });
}
