import { useQuery } from "@tanstack/react-query";
import { listScheduledTasks, type SchedulerListPayload } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useSchedulerQuery() {
  return useQuery<SchedulerListPayload>({
    queryKey: queryKeys.schedulerAll,
    queryFn: async () => {
      return await listScheduledTasks();
    },
    staleTime: 5_000, // 5s — quick enough for a management UI
  });
}
