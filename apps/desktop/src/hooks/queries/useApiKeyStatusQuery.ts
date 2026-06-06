import { useQuery } from "@tanstack/react-query";
import { getApiKeyStatus, type KeyStatus } from "@/lib/tauri";

export function useApiKeyStatusQuery(enabled = true) {
  return useQuery<KeyStatus[]>({
    queryKey: ["api-key-status"],
    queryFn: getApiKeyStatus,
    enabled,
  });
}
