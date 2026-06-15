import { useQuery } from "@tanstack/react-query";
import {
  listGatewayTriggers,
  type GatewayPendingTrigger,
} from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useGatewayTriggersQuery(enabled = true) {
  return useQuery<GatewayPendingTrigger[]>({
    queryKey: queryKeys.gatewayTriggers,
    queryFn: async () => {
      return await listGatewayTriggers();
    },
    enabled,
    staleTime: 10_000,
  });
}
