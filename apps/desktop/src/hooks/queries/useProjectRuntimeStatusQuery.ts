import { useQuery } from "@tanstack/react-query";
import { getProjectRuntimeStatus, type ProjectRuntimeStatus } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useProjectRuntimeStatusQuery(
  sessionId: string | undefined | null,
  workingDir: string | null | undefined,
  enabled = true,
) {
  return useQuery<ProjectRuntimeStatus | null>({
    queryKey: queryKeys.projectRuntimeStatus(sessionId, workingDir),
    queryFn: async () => {
      return await getProjectRuntimeStatus(sessionId ?? undefined, workingDir);
    },
    enabled: enabled && (!!sessionId || !!workingDir),
  });
}
