import { useQuery } from "@tanstack/react-query";
import { listCapabilities, type CapabilityInfo } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useCapabilitiesQuery() {
  return useQuery<CapabilityInfo[]>({
    queryKey: queryKeys.capabilities,
    queryFn: async () => {
      return await listCapabilities();
    },
  });
}
