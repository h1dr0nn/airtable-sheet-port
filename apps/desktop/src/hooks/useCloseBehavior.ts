import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc, isTauri, type CloseBehavior } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

/**
 * Persists the window close behavior ("ask" | "tray" | "quit"). Shared by the
 * Settings General card and the close dialog's "Remember My Choice" path.
 */
export function useSetCloseBehavior() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (behavior: CloseBehavior) => ipc.setCloseBehavior(behavior),
    onError: (error: unknown) => {
      toast.error(t("toast.closeBehaviorError"), { description: getErrorMessage(error) });
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}

/**
 * Whether launch-at-login (autostart) is enabled. Only queried under Tauri;
 * the browser preview has no OS integration so it reports disabled.
 */
export function useAutostartEnabled() {
  return useQuery({
    queryKey: queryKeys.autostart,
    queryFn: () => ipc.getAutostartEnabled(),
    enabled: isTauri
  });
}

/** Toggles launch-at-login (autostart), then refreshes the query. */
export function useSetAutostartEnabled() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (enabled: boolean) => ipc.setAutostartEnabled(enabled),
    onError: (error: unknown) => {
      toast.error(t("toast.launchAtLoginError"), { description: getErrorMessage(error) });
    },
    onSuccess: (_result, enabled) => {
      toast.success(enabled ? t("toast.launchAtLoginEnabled") : t("toast.launchAtLoginDisabled"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.autostart });
    }
  });
}
