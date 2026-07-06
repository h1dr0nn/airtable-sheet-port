import { useInfiniteQuery } from "@tanstack/react-query";
import { AUDIT_PAGE_SIZE } from "../lib/constants.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

/** Pages of AUDIT_PAGE_SIZE events; "load more" fetches the next offset. */
export function useAuditEvents() {
  return useInfiniteQuery({
    queryKey: queryKeys.auditEvents,
    queryFn: ({ pageParam }) => ipc.listAuditEvents(AUDIT_PAGE_SIZE, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) =>
      lastPage.length < AUDIT_PAGE_SIZE ? undefined : allPages.length * AUDIT_PAGE_SIZE
  });
}
