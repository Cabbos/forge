import { useQuery } from "@tanstack/react-query";
import { listEcosystemItems, type EcosystemItem } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useEcosystemItemsQuery() {
  return useQuery<EcosystemItem[]>({
    queryKey: queryKeys.ecosystemItems,
    queryFn: async () => {
      return await listEcosystemItems();
    },
  });
}
