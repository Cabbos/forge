import { useQuery } from "@tanstack/react-query";
import {
  listContinuityExperiences,
  searchContinuityExperiences,
  type ContinuityExperience,
} from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useContinuityExperiencesQuery(
  sessionId: string | undefined | null,
  projectPath: string | null | undefined,
  search?: string,
  enabled = true,
) {
  return useQuery<ContinuityExperience[]>({
    queryKey: queryKeys.continuityExperiences(sessionId, projectPath, search),
    queryFn: async () => {
      if (search?.trim()) {
        return await searchContinuityExperiences(
          search.trim(),
          sessionId ?? undefined,
          projectPath ?? undefined,
          20,
        );
      }
      return await listContinuityExperiences(sessionId ?? undefined, projectPath ?? undefined);
    },
    enabled: enabled && !!projectPath,
  });
}
