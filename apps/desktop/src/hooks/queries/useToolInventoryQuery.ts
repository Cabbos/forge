import { useQuery } from "@tanstack/react-query";
import { getToolInventory, type ToolInventoryEntry } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useToolInventoryQuery() {
  return useQuery<ToolInventoryEntry[]>({
    queryKey: queryKeys.toolInventory,
    queryFn: async () => {
      return await getToolInventory();
    },
  });
}
