import { useQuery } from "@tanstack/react-query";
import { listProfiles, type ProfileListPayload } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useProfilesQuery() {
  return useQuery<ProfileListPayload>({
    queryKey: queryKeys.profilesAll,
    queryFn: async () => {
      return await listProfiles();
    },
    staleTime: 5_000, // 5s — quick enough for an editing UI
  });
}
