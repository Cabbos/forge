import { useQuery } from "@tanstack/react-query";
import { listUnifiedMemories, type UnifiedMemoryListFilter, type UnifiedMemoryRecord } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useUnifiedMemoriesQuery(
  sessionId: string | null | undefined,
  projectPath: string | null | undefined,
  query: string,
  filter: UnifiedMemoryListFilter,
  enabled: boolean,
) {
  return useQuery<UnifiedMemoryRecord[]>({
    queryKey: queryKeys.unifiedMemories(sessionId, projectPath, query, filter),
    enabled,
    queryFn: async () => {
      return await listUnifiedMemories(
        sessionId ?? undefined,
        projectPath ?? undefined,
        query || undefined,
        filter,
      );
    },
  });
}
