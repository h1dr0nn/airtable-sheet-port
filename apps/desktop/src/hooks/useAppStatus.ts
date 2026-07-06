import { useQuery } from "@tanstack/react-query";
import { APP_STATUS_REFETCH_MS } from "../lib/constants.js";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useAppStatus() {
  return useQuery({
    queryKey: queryKeys.appStatus,
    queryFn: () => ipc.getAppStatus(),
    refetchInterval: APP_STATUS_REFETCH_MS
  });
}
