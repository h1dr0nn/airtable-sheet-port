import { useQuery } from "@tanstack/react-query";
import { ipc } from "../lib/ipc.js";
import { queryKeys } from "../lib/queryKeys.js";

export function useSources() {
  return useQuery({
    queryKey: queryKeys.sources,
    queryFn: () => ipc.listSources()
  });
}
