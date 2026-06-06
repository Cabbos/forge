import { useQuery } from "@tanstack/react-query";
import { getProjectCheckpointStatus, type ProjectCheckpointStatus } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useProjectCheckpointStatusQuery(
  sessionId: string | undefined | null,
  workingDir: string | null | undefined,
  enabled = true,
) {
  return useQuery<ProjectCheckpointStatus | null>({
    queryKey: queryKeys.projectCheckpointStatus(sessionId, workingDir),
    queryFn: async () => {
      return await getProjectCheckpointStatus(sessionId ?? undefined, workingDir);
    },
    enabled: enabled && (!!sessionId || !!workingDir),
  });
}
