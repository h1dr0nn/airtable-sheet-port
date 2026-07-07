import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { toast } from "@sheet-port/ui";
import { getErrorMessage } from "../lib/errors.js";
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

  return useMutation({
    mutationFn: (transport: McpTransport) => ipc.setMcpTransport(transport),
    onError: (error: unknown) => {
      toast.error("Transport not updated", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("MCP transport saved", { description: "Restart the sidecar to apply" });
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

export function useSetMcpPort() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (port: number) => ipc.setMcpPort(port),
    onError: (error: unknown) => {
      toast.error("Port not saved", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("MCP port saved", { description: "Restart the sidecar to apply" });
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

export function useConfigureMcpClient() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => ipc.mcpConfigureClient(id),
    onError: (error: unknown) => {
      toast.error("Client not configured", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("MCP client configured");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClients });
    }
  });
}

export function useUnregisterMcpClient() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => ipc.mcpUnregisterClient(id),
    onError: (error: unknown) => {
      toast.error("Client not unregistered", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("MCP client unregistered");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClients });
    }
  });
}

/** Starts the desktop-managed HTTP sidecar, then refreshes the running state. */
export function useStartMcpServer() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => ipc.mcpServerStart(),
    onError: (error: unknown) => {
      toast.error("MCP server not started", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("MCP server started");
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

/** Stops the desktop-managed sidecar, then refreshes the running state. */
export function useStopMcpServer() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => ipc.mcpServerStop(),
    onError: (error: unknown) => {
      toast.error("MCP server not stopped", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("MCP server stopped");
    },
    onSettled: () => {
      invalidateMcpConfig(queryClient);
    }
  });
}

export function useConfigureAllMcpClients() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => ipc.mcpConfigureAll(),
    onError: (error: unknown) => {
      toast.error("Clients not configured", { description: getErrorMessage(error) });
    },
    onSuccess: () => {
      toast.success("Detected MCP clients configured");
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClients });
    }
  });
}
