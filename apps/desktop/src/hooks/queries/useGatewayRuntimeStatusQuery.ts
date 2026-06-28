import { useQuery } from "@tanstack/react-query";
import {
  getGatewayRuntimeStatus,
  type GatewayRuntimeStatus,
} from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useGatewayRuntimeStatusQuery(enabled = true) {
  return useQuery<GatewayRuntimeStatus>({
    queryKey: queryKeys.gatewayRuntimeStatus,
    queryFn: async () => {
      return await getGatewayRuntimeStatus();
    },
    enabled,
    staleTime: 10_000,
  });
}
