import { useQuery } from "@tanstack/react-query";
import {
  listGatewaySessions,
  type GatewaySessionInfo,
} from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useGatewaySessionsQuery(enabled = true) {
  return useQuery<GatewaySessionInfo[]>({
    queryKey: queryKeys.gatewaySessions,
    queryFn: async () => {
      return await listGatewaySessions();
    },
    enabled,
    staleTime: 10_000,
  });
}
