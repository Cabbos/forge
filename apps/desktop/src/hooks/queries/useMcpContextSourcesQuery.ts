import { useQuery } from "@tanstack/react-query";
import { listMcpContextSources, type McpContextSources } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useMcpContextSourcesQuery(
  sessionId: string | undefined | null,
  enabled = true,
) {
  return useQuery<McpContextSources>({
    queryKey: queryKeys.mcpContextSources(sessionId),
    queryFn: async () => {
      return await listMcpContextSources(sessionId ?? undefined);
    },
    enabled: enabled && !!sessionId,
  });
}
