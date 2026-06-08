import { useQuery } from "@tanstack/react-query";
import { loadAppMetadata } from "@/lib/tauri";
import type { AppMetadata } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useAppMetadataQuery(enabled = true) {
  return useQuery<AppMetadata>({
    queryKey: queryKeys.appMetadata,
    queryFn: async () => {
      return await loadAppMetadata();
    },
    enabled,
  });
}
