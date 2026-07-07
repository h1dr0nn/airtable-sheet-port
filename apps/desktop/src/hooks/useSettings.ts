import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc, type FontFamily, type FontScale, type Language } from "../lib/ipc.js";
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
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (enabled: boolean) => ipc.setAutoApprove(enabled),
    onError: (error: unknown) => {
      toast.error(t("toast.autoApproveError"), { description: getErrorMessage(error) });
    },
    onSuccess: (_result, enabled) => {
      toast.success(enabled ? t("toast.autoApproveEnabled") : t("toast.autoApproveDisabled"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}

/** Persists the UI font-size scale; useFonts applies it live on invalidation. */
export function useSetFontScale() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (scale: FontScale) => ipc.setFontScale(scale),
    onError: (error: unknown) => {
      toast.error(t("toast.fontSizeError"), { description: getErrorMessage(error) });
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}

/** Persists the UI font family; useFonts applies it live on invalidation. */
export function useSetFontFamily() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (family: FontFamily) => ipc.setFontFamily(family),
    onError: (error: unknown) => {
      toast.error(t("toast.fontError"), { description: getErrorMessage(error) });
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}

/** Persists the UI language; the translator re-derives on settings invalidation. */
export function useSetLanguage() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (language: Language) => ipc.setLanguage(language),
    onError: (error: unknown) => {
      toast.error(t("toast.languageError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.languageUpdated"));
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
  const { t } = useTranslation();

  return useMutation({
    mutationFn: () => ipc.resetSettings(),
    onError: (error: unknown) => {
      toast.error(t("toast.resetFailed"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      // Theme is a frontend-only pref, so reset it client-side to System.
      setSetting("system");
      toast.success(t("toast.settingsReset"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
    }
  });
}
