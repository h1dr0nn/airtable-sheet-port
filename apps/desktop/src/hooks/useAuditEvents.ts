import {
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  type QueryClient
} from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { AUDIT_PAGE_SIZE } from "../lib/constants.js";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation, type TFunction } from "../i18n/useTranslation.js";
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

/** Clear-activity mutation. Success is deliberately silent: announcing the
 * clear with a toast (or a new audit row) would add an entry right after the
 * user emptied the list. The panel's empty state is the feedback. Invalidating
 * the `["audit-events"]` prefix refreshes both the paged dropdown key and the
 * dashboard key so every activity surface reflects the cleared log. */
export function clearAuditMutationOptions(queryClient: QueryClient, t: TFunction) {
  return {
    mutationFn: () => ipc.clearAuditLog(),
    onError: (error: unknown) => {
      toast.error(t("toast.activityClearError"), { description: getErrorMessage(error) });
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
    }
  };
}

/** Wipes the audit log, then refreshes every audit query. */
export function useClearAuditEvents() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  return useMutation(clearAuditMutationOptions(queryClient, t));
}
