import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useToast } from "@sheet-port/ui";
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
  const { toast } = useToast();

  return useMutation({
    mutationFn: (clientId: string) => ipc.setGoogleClientId(clientId),
    onError: (error: unknown) => {
      toast(getErrorMessage(error), "error");
    },
    onSuccess: () => {
      toast("Google client ID saved", "success");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
    }
  });
}

export function useGoogleConnect() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: () => ipc.googleConnect(),
    onError: (error: unknown) => {
      toast(getErrorMessage(error), "error");
    },
    onSuccess: (result) => {
      toast(`Connected as ${result.email}`, "success");
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}

export function useGoogleDisconnect() {
  const queryClient = useQueryClient();
  const { toast } = useToast();

  return useMutation({
    mutationFn: () => ipc.googleDisconnect(),
    onError: (error: unknown) => {
      toast(getErrorMessage(error), "error");
    },
    onSuccess: () => {
      toast("Google Sheets disconnected", "success");
    },
    onSettled: () => {
      invalidateGoogleState(queryClient);
    }
  });
}
