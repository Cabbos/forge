import type { CapabilityInfo } from "@/lib/tauri";
import type { CapabilityTab } from "@/components/settings/capabilityTypes";

export function filterCapabilities(list: CapabilityInfo[], search: string, tab: CapabilityTab) {
  const query = search.trim().toLowerCase();
  if (!query) return list;

  return list.filter((capability) => {
    const searchable = tab === "mcp"
      ? [capability.name, capability.id, capability.source]
      : [capability.name, capability.description, capability.id, capability.version, capability.source];

    return searchable.some((value) => String(value ?? "").toLowerCase().includes(query));
  });
}
