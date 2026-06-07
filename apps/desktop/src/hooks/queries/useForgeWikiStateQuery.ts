import { useQuery } from "@tanstack/react-query";
import { getForgeWikiState } from "@/lib/tauri";
import type { ForgeWikiState } from "@/lib/protocol";
import { queryKeys } from "./queryKeys";

export function useForgeWikiStateQuery(
  projectPath: string | null | undefined,
  sessionId?: string | null,
  enabled = true,
) {
  return useQuery<ForgeWikiState>({
    queryKey: queryKeys.forgeWikiState(projectPath ?? "", sessionId),
    queryFn: async () => {
      return await getForgeWikiState(projectPath!, sessionId);
    },
    enabled: enabled && !!projectPath,
  });
}
