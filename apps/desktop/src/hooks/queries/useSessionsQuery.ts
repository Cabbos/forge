import { useQuery } from "@tanstack/react-query";
import { listSessions } from "@/lib/tauri";
import type { SessionInfo } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useSessionsQuery(enabled = true) {
  return useQuery<SessionInfo[]>({
    queryKey: queryKeys.sessions,
    queryFn: async () => {
      return await listSessions();
    },
    enabled,
  });
}
