import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useToast } from "@sheet-port/ui";
import type { ChangeStatus, PendingChange } from "@sheet-port/shared";
import { getErrorMessage } from "../lib/errors.js";
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
  successMessage: string
) {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: decide,
    onError: (error: unknown) => {
      toast(getErrorMessage(error), "error");
    },
    onSuccess: () => {
      toast(successMessage, "success");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.changesRoot });
      void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
      void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
    }
  });
}

export function useApproveChange() {
  return useDecideChange((changeId) => ipc.approveChange(changeId), "Change approved");
}

export function useRejectChange() {
  return useDecideChange((changeId) => ipc.rejectChange(changeId), "Change rejected");
}
