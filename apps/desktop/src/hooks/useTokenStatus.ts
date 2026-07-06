import { useQuery } from "@tanstack/react-query";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useTokenStatus() {
  return useQuery({
    queryKey: queryKeys.tokenStatus,
    queryFn: () => ipc.tokenStatus()
  });
}
