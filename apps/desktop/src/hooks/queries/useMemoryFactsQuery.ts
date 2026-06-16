import { useQuery } from "@tanstack/react-query";
import { listMemoryFacts, type MemoryFact } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useMemoryFactsQuery(query?: string, profileId?: string | null) {
  return useQuery<MemoryFact[]>({
    queryKey: queryKeys.memoryFacts(query, profileId),
    queryFn: async () => {
      return await listMemoryFacts(query, profileId);
    },
    staleTime: 5_000, // 5s — quick enough for an editing UI
  });
}
