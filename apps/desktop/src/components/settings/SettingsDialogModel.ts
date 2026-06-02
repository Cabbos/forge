import type { KeyStatus } from "@/lib/tauri";
import { PROVIDERS } from "@/lib/providers";

interface SettingsProviderState {
  configuredCount: number;
  providerTotal: number;
  sortedKeys: KeyStatus[];
}

export function buildSettingsProviderState(keys: KeyStatus[]): SettingsProviderState {
  const keyByProvider = new Map(keys.map((key) => [key.provider, key]));
  const knownProviderStatuses: KeyStatus[] = PROVIDERS.map((provider) =>
    keyByProvider.get(provider.id) ?? { provider: provider.id, set: false, preview: "" },
  );
  const unknownProviderStatuses = keys.filter((key) => !PROVIDERS.some((provider) => provider.id === key.provider));
  const sortedKeys = [...knownProviderStatuses, ...unknownProviderStatuses].sort((a, b) => {
    const aIndex = PROVIDERS.findIndex((provider) => provider.id === a.provider);
    const bIndex = PROVIDERS.findIndex((provider) => provider.id === b.provider);
    return (aIndex < 0 ? 99 : aIndex) - (bIndex < 0 ? 99 : bIndex);
  });

  return {
    configuredCount: sortedKeys.filter((key) => key.set).length,
    providerTotal: sortedKeys.length || PROVIDERS.length,
    sortedKeys,
  };
}
