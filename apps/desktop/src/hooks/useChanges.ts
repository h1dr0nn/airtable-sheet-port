import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import type { ChangeStatus } from "@sheet-port/shared";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useChanges(status: ChangeStatus | null) {
  return useQuery({
    queryKey: queryKeys.changes(status),
    queryFn: () => ipc.listChanges(status)
  });
}

/** Discards a staged (dry-run) change that is still pending. */
export function useDiscardChange() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (changeId: string) => ipc.rejectChange(changeId),
    onError: (error: unknown) => {
      toast.error(t("toast.changeDiscardError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.changeDiscarded"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.changesRoot });
      void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
      void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
    }
  });
}
