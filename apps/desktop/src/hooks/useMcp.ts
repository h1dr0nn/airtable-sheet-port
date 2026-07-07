import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc } from "../lib/ipc.js";
import type { McpTransport } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

/** Persisted transport/port plus the live sidecar heartbeat state. */
export function useMcpConfig() {
  return useQuery({
    queryKey: queryKeys.mcpConfig,
    queryFn: () => ipc.getMcpConfig(),
    // Poll so the running state reflects the sidecar heartbeat shortly after a
    // Start/Stop (the child writes its first heartbeat asynchronously).
    refetchInterval: 4000
  });
}

/** Detected MCP clients and their per-client config state. */
export function useMcpClients() {
  return useQuery({
    queryKey: queryKeys.mcpClients,
    queryFn: () => ipc.mcpDetectClients()
  });
}

function invalidateMcpConfig(queryClient: QueryClient): void {
  void queryClient.invalidateQueries({ queryKey: queryKeys.mcpConfig });
  // The dashboard/status surfaces read the same sidecar heartbeat.
  void queryClient.invalidateQueries({ queryKey: queryKeys.appStatus });
}

export function useSetMcpTransport() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (transport: McpTransport) => ipc.setMcpTransport(transport),
    onError: (error: unknown) => {
      toast.error(t("toast.transportError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.transportSaved"), { description: t("toast.restartToApply") });
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

export function useSetMcpPort() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (port: number) => ipc.setMcpPort(port),
    onError: (error: unknown) => {
      toast.error(t("toast.portError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.portSaved"), { description: t("toast.restartToApply") });
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

export function useConfigureMcpClient() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (id: string) => ipc.mcpConfigureClient(id),
    onError: (error: unknown) => {
      toast.error(t("toast.clientConfigError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.clientConfigured"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClients });
    }
  });
}

export function useUnregisterMcpClient() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: (id: string) => ipc.mcpUnregisterClient(id),
    onError: (error: unknown) => {
      toast.error(t("toast.clientUnregisterError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.clientUnregistered"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClients });
    }
  });
}

/** Starts the desktop-managed HTTP sidecar, then refreshes the running state. */
export function useStartMcpServer() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: () => ipc.mcpServerStart(),
    onError: (error: unknown) => {
      toast.error(t("toast.serverStartError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.serverStarted"));
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

/** Stops the desktop-managed sidecar, then refreshes the running state. */
export function useStopMcpServer() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: () => ipc.mcpServerStop(),
    onError: (error: unknown) => {
      toast.error(t("toast.serverStopError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.serverStopped"));
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

export function useConfigureAllMcpClients() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();

  return useMutation({
    mutationFn: () => ipc.mcpConfigureAll(),
    onError: (error: unknown) => {
      toast.error(t("toast.clientsConfigError"), { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success(t("toast.clientsConfigured"));
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClients });
    }
  });
}
