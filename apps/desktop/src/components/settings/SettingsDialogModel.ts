import type { KeyStatus } from "@/lib/tauri";
import { PROVIDERS, type ProviderDefinition } from "@/lib/providers";

interface SettingsProviderState {
  configuredCount: number;
  providerTotal: number;
  sortedKeys: KeyStatus[];
}

export function buildSettingsProviderState(
  keys: KeyStatus[],
  providers: ProviderDefinition[] = PROVIDERS,
): SettingsProviderState {
  const keyByProvider = new Map(keys.map((key) => [key.provider, key]));
  const knownProviderStatuses: KeyStatus[] = providers.map((provider) =>
    keyByProvider.get(provider.id) ?? {
      provider: provider.id,
      configured: false,
      source: "none",
      status: "not_configured",
      error: null,
    },
  );
  const unknownProviderStatuses = keys.filter((key) => !providers.some((provider) => provider.id === key.provider));
  const sortedKeys = [...knownProviderStatuses, ...unknownProviderStatuses].sort((a, b) => {
    const aIndex = providers.findIndex((provider) => provider.id === a.provider);
    const bIndex = providers.findIndex((provider) => provider.id === b.provider);
    return (aIndex < 0 ? 99 : aIndex) - (bIndex < 0 ? 99 : bIndex);
  });

  return {
    configuredCount: sortedKeys.filter((key) => key.configured && key.status === "available").length,
    providerTotal: sortedKeys.length || providers.length,
    sortedKeys,
  };
}
