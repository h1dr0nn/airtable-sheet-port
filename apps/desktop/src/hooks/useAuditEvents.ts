import { useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { AUDIT_PAGE_SIZE } from "../lib/constants.js";
import { getErrorMessage } from "../lib/errors.js";
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

/** Wipes the audit log, then refreshes every audit query. Invalidating the
 * `["audit-events"]` prefix covers both the paged dropdown key and the
 * dashboard key so all activity surfaces reflect the cleared log. */
export function useClearAuditEvents() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => ipc.clearAuditLog(),
    onError: (error: unknown) => {
      toast.error("Activity not cleared", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("Activity cleared");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
    }
  });
}
