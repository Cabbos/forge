import { useQuery } from "@tanstack/react-query";
import { searchWorkspaceFiles } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useSearchWorkspaceFilesQuery(
  query: string,
  sessionId?: string,
  workingDir?: string | null,
  enabled = true,
) {
  return useQuery<string[]>({
    queryKey: queryKeys.searchWorkspaceFiles(query, sessionId, workingDir),
    queryFn: async () => {
      return await searchWorkspaceFiles(query, sessionId, workingDir);
    },
    enabled,
  });
}
