import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { DiagnosticsReport, LogEntry, ServiceStatus } from "./types";

export async function getServiceStatus(): Promise<ServiceStatus> {
  if (!hasTauriRuntime()) {
    return {
      installed: false,
      running: false,
      message: "Service status not available outside Tauri runtime.",
      supported: false,
    };
  }
  return invoke<ServiceStatus>("get_service_status");
}

export async function setAutostart(enabled: boolean): Promise<ServiceStatus> {
  return invoke<ServiceStatus>("set_autostart", { enabled });
}

export async function getRecentLogs(
  limit?: number,
  level?: string,
): Promise<LogEntry[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<LogEntry[]>("get_recent_logs", { limit, level });
}

export async function getDiagnosticsReport(): Promise<DiagnosticsReport> {
  if (!hasTauriRuntime()) {
    return {
      ok: false,
      generatedAtMs: Date.now(),
      checks: [
        {
          id: "runtime",
          label: "Tauri runtime",
          status: "warn",
          message: "Diagnostics report not available outside Tauri runtime.",
        },
      ],
    };
  }
  return invoke<DiagnosticsReport>("get_diagnostics_report");
}
