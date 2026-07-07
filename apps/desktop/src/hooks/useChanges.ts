import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import type { ChangeStatus, PendingChange } from "@sheet-port/shared";
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

function useDecideChange(
  decide: (changeId: string) => Promise<PendingChange>,
  successKey: "toast.changeApproved" | "toast.changeRejected"
) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: decide,
    onError: (error: unknown) => {
      toast.error(t("toast.changeDecisionFailed"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t(successKey));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.changesRoot });
      void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
      void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
    }
  });
}

export function useApproveChange() {
  return useDecideChange((changeId) => ipc.approveChange(changeId), "toast.changeApproved");
}

export function useRejectChange() {
  return useDecideChange((changeId) => ipc.rejectChange(changeId), "toast.changeRejected");
}
