import { useQuery } from "@tanstack/react-query";
import { listMemoryFacts, type MemoryFact } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useMemoryFactsQuery(query?: string) {
  return useQuery<MemoryFact[]>({
    queryKey: queryKeys.memoryFacts(query),
    queryFn: async () => {
      return await listMemoryFacts(query);
    },
    staleTime: 5_000, // 5s — quick enough for an editing UI
  });
}
