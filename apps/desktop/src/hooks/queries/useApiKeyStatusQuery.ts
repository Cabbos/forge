import { useQuery } from "@tanstack/react-query";
import { getApiKeyStatus, type KeyStatus } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useApiKeyStatusQuery(enabled = true) {
  return useQuery<KeyStatus[]>({
    queryKey: queryKeys.apiKeyStatus,
    queryFn: getApiKeyStatus,
    enabled,
  });
}
