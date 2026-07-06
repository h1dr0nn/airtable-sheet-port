import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";
import { useTheme } from "./useTheme.js";

/** App-managed preferences stored in the shared meta table (e.g. auto-approve). */
export function useSettings() {
  return useQuery({
    queryKey: queryKeys.settings,
    queryFn: () => ipc.getSettings()
  });
}

/** Toggles auto-approve; enabling bypasses the human confirmation gate. */
export function useSetAutoApprove() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (enabled: boolean) => ipc.setAutoApprove(enabled),
    onError: (error: unknown) => {
      toast.error("Auto-approve not updated", { description: getErrorMessage(error) });
    },
    onSuccess: (_result, enabled) => {
      toast.success(enabled ? "Auto-approve enabled" : "Auto-approve disabled");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}

/** Resets app-managed prefs and the local theme setting back to their defaults. */
export function useResetSettings() {
  const queryClient = useQueryClient();
  const { setSetting } = useTheme();

  return useMutation({
    mutationFn: () => ipc.resetSettings(),
    onError: (error: unknown) => {
      toast.error("Reset failed", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      // Theme is a frontend-only pref, so reset it client-side to System.
      setSetting("system");
      toast.success("Settings reset to default");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}
