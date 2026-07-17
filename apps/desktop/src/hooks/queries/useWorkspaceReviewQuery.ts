import { useQuery } from "@tanstack/react-query";
import { getWorkspaceReview } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useWorkspaceReviewQuery(
  sessionId?: string | null,
  workingDir?: string | null,
) {
  return useQuery({
    queryKey: queryKeys.workspaceReview(sessionId, workingDir),
    queryFn: () => getWorkspaceReview(sessionId ?? undefined, workingDir),
    enabled: Boolean(sessionId || workingDir),
    refetchOnWindowFocus: false,
  });
}
