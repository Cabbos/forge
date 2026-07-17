import type { GatewayRuntimeStatus } from "@/lib/tauri";

/** Pure display formatters for the diagnostics and gateway runtime panels. */

export function formatRuntimeTaskName(name: string): string {
  switch (name) {
    case "webhook_listener":
      return "webhook";
    case "trigger_runner":
      return "trigger";
    case "scheduler_tick":
      return "scheduler";
    default:
      return name.replace(/_/g, " ");
  }
}

export function formatGatewayDegradedMode(
  degraded?: GatewayRuntimeStatus["degraded_mode"],
): string | null {
  if (!degraded?.active) return null;
  const fallback = degraded.fallback || "desktop_runtime";
  const recovery = degraded.recovery_command || "forge service restart";
  return `${degraded.reason || "Gateway degraded mode is active."} · fallback ${fallback} · recovery ${recovery}`;
}

export function formatDiagnosticDetail(detail: unknown): string {
  if (detail === null || detail === undefined) return "";
  if (typeof detail !== "object") return String(detail);
  const entries = Object.entries(detail as Record<string, unknown>);
  if (entries.length === 0) return "{}";
  return entries
    .slice(0, 4)
    .map(([key, value]) => `${key}: ${formatDetailValue(value)}`)
    .join(" · ");
}

export function formatDetailValue(value: unknown): string {
  if (Array.isArray(value)) return `[${value.length}]`;
  if (value && typeof value === "object") return "{...}";
  return String(value);
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h`;
}
