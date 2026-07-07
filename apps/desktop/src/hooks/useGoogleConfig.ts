import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useGoogleConfig() {
  return useQuery({
    queryKey: queryKeys.googleConfig,
    queryFn: () => ipc.getGoogleConfig()
  });
}

/** Every connected Google account (sourceId + email). */
export function useGoogleAccounts() {
  return useQuery({
    queryKey: queryKeys.googleAccounts,
    queryFn: () => ipc.googleListAccounts()
  });
}

/** Connect/disconnect changes sources, tables, tokens, and audit history. */
function invalidateGoogleState(queryClient: QueryClient): void {
  void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
  void queryClient.invalidateQueries({ queryKey: queryKeys.googleAccounts });
  void queryClient.invalidateQueries({ queryKey: queryKeys.sources });
  void queryClient.invalidateQueries({ queryKey: queryKeys.tokenStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.tablesRoot });
  void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.changesRoot });
  void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
}

export function useSetGoogleClientId() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (clientId: string) => ipc.setGoogleClientId(clientId),
    onError: (error: unknown) => {
      toast.error(t("toast.clientIdError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.clientIdSaved"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
    }
  });
}

/** Stores the OAuth client secret in the keychain; empty string clears it. */
export function useSetGoogleClientSecret() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (clientSecret: string) => ipc.setGoogleClientSecret(clientSecret),
    onError: (error: unknown) => {
      toast.error(t("toast.clientSecretError"), { description: getErrorMessage(error) });
    },
    onSuccess: (_result, clientSecret) => {
      if (clientSecret === "") {
        toast.success(t("toast.clientSecretCleared"));
      } else {
        toast.success(t("toast.clientSecretSaved"), { description: t("toast.clientSecretSavedDesc") });
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
    }
  });
}

export function useGoogleConnect() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: () => ipc.googleConnect(),
    onError: (error: unknown) => {
      toast.error(t("toast.googleConnectError"), { description: getErrorMessage(error) });
    },
    onSuccess: (result) => {
      toast.success(t("toast.googleConnected"), { description: t("toast.googleConnectedDesc", { email: result.email }) });
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}

export function useGoogleDisconnect() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (sourceId: string) => ipc.googleDisconnect(sourceId),
    onError: (error: unknown) => {
      toast.error(t("toast.googleDisconnectError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.googleDisconnected"));
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}
