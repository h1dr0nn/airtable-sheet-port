import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useGoogleConfig() {
  return useQuery({
    queryKey: queryKeys.googleConfig,
    queryFn: () => ipc.getGoogleConfig()
  });
}

/** Connect/disconnect changes sources, tables, tokens, and audit history. */
function invalidateGoogleState(queryClient: QueryClient): void {
  void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
  void queryClient.invalidateQueries({ queryKey: queryKeys.sources });
  void queryClient.invalidateQueries({ queryKey: queryKeys.tokenStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.tablesRoot });
  void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.changesRoot });
  void queryClient.invalidateQueries({ queryKey: queryKeys.auditEvents });
}

export function useSetGoogleClientId() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (clientId: string) => ipc.setGoogleClientId(clientId),
    onError: (error: unknown) => {
      toast.error("Client ID not saved", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("Google client ID saved");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
    }
  });
}

/** Stores the OAuth client secret in the keychain; empty string clears it. */
export function useSetGoogleClientSecret() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (clientSecret: string) => ipc.setGoogleClientSecret(clientSecret),
    onError: (error: unknown) => {
      toast.error("Client secret not saved", { description: getErrorMessage(error) });
    },
    onSuccess: (_result, clientSecret) => {
      if (clientSecret === "") {
        toast.success("Google client secret cleared");
      } else {
        toast.success("Google client secret saved", { description: "Stored in the OS keychain" });
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
    }
  });
}

export function useGoogleConnect() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => ipc.googleConnect(),
    onError: (error: unknown) => {
      toast.error("Google Sheets connection failed", { description: getErrorMessage(error) });
    },
    onSuccess: (result) => {
      toast.success("Google Sheets connected", { description: `Signed in as ${result.email}` });
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}

export function useGoogleDisconnect() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => ipc.googleDisconnect(),
    onError: (error: unknown) => {
      toast.error("Google Sheets disconnect failed", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("Google Sheets disconnected");
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}
