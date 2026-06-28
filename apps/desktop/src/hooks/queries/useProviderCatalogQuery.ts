import { useQuery } from "@tanstack/react-query";
import { getProviderCatalog, type ProviderCatalogEntry } from "@/lib/tauri";
import { mergeProviderCatalog, PROVIDERS, type ProviderDefinition } from "@/lib/providers";
import { queryKeys } from "./queryKeys";

export function useProviderCatalogQuery(enabled = true) {
  return useQuery<ProviderCatalogEntry[], Error, ProviderDefinition[]>({
    queryKey: queryKeys.providerCatalog,
    queryFn: getProviderCatalog,
    enabled,
    select: (entries) => mergeProviderCatalog(entries),
    placeholderData: [],
  });
}

export function useProviderCatalog(enabled = true): ProviderDefinition[] {
  const { data } = useProviderCatalogQuery(enabled);
  return data ?? PROVIDERS;
}
