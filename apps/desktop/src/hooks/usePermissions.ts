import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { ipc, type PermissionRuleRow, type SavePermissionRule } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function usePermissionRules() {
  return useQuery({
    queryKey: queryKeys.permissionRules,
    queryFn: () => ipc.listPermissionRules()
  });
}

type SaveContext = {
  previous: PermissionRuleRow[] | undefined;
};

/** Optimistically applies edits to existing rules, then reconciles with the backend. */
export function useSavePermissionRule() {
  const queryClient = useQueryClient();

  return useMutation<PermissionRuleRow, unknown, SavePermissionRule, SaveContext>({
    mutationFn: (rule) => ipc.savePermissionRule(rule),
    onMutate: async (rule) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.permissionRules });
      const previous = queryClient.getQueryData<PermissionRuleRow[]>(queryKeys.permissionRules);
      if (previous && rule.id !== null) {
        const optimistic = previous.map((row) =>
          row.id === rule.id
            ? { ...row, ...rule, id: row.id, updatedAt: new Date().toISOString() }
            : row
        );
        queryClient.setQueryData(queryKeys.permissionRules, optimistic);
      }
      return { previous };
    },
    onError: (error, _rule, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKeys.permissionRules, context.previous);
      }
      toast.error("Permission rule not saved", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("Permission rule saved");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.permissionRules });
      void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
    }
  });
}
