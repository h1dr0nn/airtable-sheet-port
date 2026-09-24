import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation, type TFunction } from "../i18n/useTranslation.js";
import { ipc, type GoogleAccount } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

/** Every connected Google account, one per Apps Script bridge. */
export function useGoogleAccounts() {
  return useQuery({
    queryKey: queryKeys.googleAccounts,
    queryFn: () => ipc.googleListAccounts()
  });
}

/** Bridge changes touch sources, tables, tokens, and audit history. */
function invalidateGoogleState(queryClient: QueryClient): void {
  void queryClient.invalidateQueries({ queryKey: queryKeys.googleAccounts });
  void queryClient.invalidateQueries({ queryKey: queryKeys.sources });
  void queryClient.invalidateQueries({ queryKey: queryKeys.tokenStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.tablesRoot });
  void queryClient.invalidateQueries({ queryKey: queryKeys.permissionRules });
  void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
}

export type AddBridgeInput = {
  url: string;
  secret: string;
};

/**
 * Options for useAddGoogleBridge, split out so the secret handling can be
 * tested without React. The mutation cache keeps `variables` (and so the
 * secret) for as long as the mutation lives, so the secret is wiped from the
 * variables object once the call has settled; it has already crossed IPC.
 */
export function addBridgeMutationOptions(queryClient: QueryClient, t: TFunction) {
  return {
    mutationFn: ({ url, secret }: AddBridgeInput) => ipc.googleAddBridge(url, secret),
    onError: (error: unknown) => {
      toast.error(t("toast.bridgeAddError"), { description: getErrorMessage(error) });
    },
    onSuccess: (account: GoogleAccount) => {
      toast.success(t("toast.bridgeAdded"), {
        description: t("toast.bridgeSignedInAs", { email: account.email })
      });
    },
    onSettled: (_account: GoogleAccount | undefined, _error: unknown, input: AddBridgeInput) => {
      input.secret = "";
      invalidateGoogleState(queryClient);
    }
  };
}

/** Adds (or replaces) a bridge; the backend calls it once to learn the email. */
export function useAddGoogleBridge() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation(addBridgeMutationOptions(queryClient, t));
}

/** Fetches a fresh token from one bridge to prove it still works. */
export function useTestGoogleBridge() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (sourceId: string) => ipc.googleTestBridge(sourceId),
    onError: (error: unknown) => {
      toast.error(t("toast.bridgeTestError"), { description: getErrorMessage(error) });
    },
    onSuccess: (account) => {
      toast.success(t("toast.bridgeTestOk"), {
        description: t("toast.bridgeSignedInAs", { email: account.email })
      });
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}

/** Removes one bridge (keychain credential + source row). Idempotent. */
export function useRemoveGoogleBridge() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (sourceId: string) => ipc.googleRemoveBridge(sourceId),
    onError: (error: unknown) => {
      toast.error(t("toast.bridgeRemoveError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.bridgeRemoved"));
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}
