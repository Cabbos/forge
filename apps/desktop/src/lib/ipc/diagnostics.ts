import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";
import type {
  AttachGatewaySessionResult,
  CancelGatewayTriggerResult,
  DiagnosticsReport,
  EnqueueGatewaySessionInputResult,
  EnqueueGatewayTriggerInput,
  EnqueueGatewayTriggerResult,
  GatewayPendingTrigger,
  GatewayRuntimeStatus,
  GatewaySessionInfo,
  GetGatewaySessionSnapshotResult,
  GetGatewayTriggerRunResult,
  LogEntry,
  RepairAction,
  RepairResult,
  ReplayGatewayTriggerRunResult,
  ServiceStatus,
  TailGatewaySessionEventsResult,
} from "./types.ts";

export async function getServiceStatus(): Promise<ServiceStatus> {
  if (!hasTauriRuntime()) {
    return {
      installed: false,
      running: false,
      message: "Service status not available outside Tauri runtime.",
      supported: false,
      backend: "unsupported",
      service_id: "",
      label: "com.forge.gateway",
      launch_domain: "unsupported",
      service_path: "",
      plist_path: "",
      log_path: "",
      error_log_path: "",
      status_message: "Service status not available outside Tauri runtime.",
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

export async function listRepairActions(): Promise<RepairAction[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<RepairAction[]>("list_repair_actions");
}

export async function runRepairAction(actionId: string): Promise<RepairResult> {
  return invoke<RepairResult>("run_repair_action", { actionId });
}

export async function getGatewayRuntimeStatus(): Promise<GatewayRuntimeStatus> {
  if (!hasTauriRuntime()) {
    return {
      ok: false,
      message: "Gateway runtime status is not available outside Tauri runtime.",
      ownership: {
        ownership_mode: "local_default",
        gateway_default_enabled: false,
        gateway_can_own_sessions: false,
        requires_opt_in: true,
        parity_gate: "pending",
        recovery_gate: "pending",
        required_action:
          "Keep the desktop runtime as owner until parity and restart/recovery gates pass.",
      },
      degraded_mode: {
        active: true,
        reason: "Gateway runtime status is not available outside Tauri runtime.",
        fallback: "desktop_runtime",
        input_policy:
          "Queued session input stays pending until the owning desktop runtime accepts it.",
        confirmation_policy: "Pending confirmations stay with the owning desktop runtime.",
        recovery_command: "forge service restart",
      },
      runtime_health: {
        ok: false,
        generated_at_ms: 0,
        active_runs: {
          active_sessions: 0,
          running_loop_tasks: 0,
        },
        pending_confirmations: {
          count: 0,
          available: false,
          source: "non_tauri_fallback",
        },
        loop_tasks: {
          total: 0,
          pending: 0,
          running: 0,
          waiting_for_input: 0,
          waiting_for_review: 0,
          completed: 0,
          failed: 0,
          canceled: 0,
          interrupted: 0,
          stale_leases: 0,
          orphaned: 0,
          recoverable: 0,
        },
        gateway_queue: {
          pending_triggers: 0,
          claimed_triggers: 0,
          pending_session_inputs: 0,
          dead_letter_runs: 0,
        },
        scheduler_queue: {
          running: false,
          pending_tasks: 0,
          source: "non_tauri_fallback",
        },
        runtime_tasks: {
          total: 0,
          running: 0,
          failed: 0,
          webhook_listener_running: false,
          trigger_runner_running: false,
          loop_runner_running: false,
          scheduler_tick_running: false,
          dashboard_http_running: false,
        },
        last_replay: {
          ok: false,
          task_count: 0,
          message: "Gateway runtime status is not available outside Tauri runtime.",
        },
      },
      uptime_seconds: 0,
      active_sessions: 0,
      pending_triggers: 0,
      pending_session_inputs: 0,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      recent_runs: [],
      recent_session_inputs: [],
      runtime_tasks: [],
    };
  }
  return invoke<GatewayRuntimeStatus>("get_gateway_runtime_status");
}

export async function enqueueGatewayTrigger(
  input: EnqueueGatewayTriggerInput,
): Promise<EnqueueGatewayTriggerResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway trigger enqueue is not available outside Tauri runtime.");
  }
  return invoke<EnqueueGatewayTriggerResult>("enqueue_gateway_trigger", { input });
}

export async function listGatewayTriggers(): Promise<GatewayPendingTrigger[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<GatewayPendingTrigger[]>("list_gateway_triggers");
}

export async function listGatewaySessions(): Promise<GatewaySessionInfo[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<GatewaySessionInfo[]>("list_gateway_sessions");
}

export async function cancelGatewayTrigger(
  triggerId: string,
): Promise<CancelGatewayTriggerResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway trigger cancel is not available outside Tauri runtime.");
  }
  return invoke<CancelGatewayTriggerResult>("cancel_gateway_trigger", { triggerId });
}

export async function replayGatewayTriggerRun(
  runId: string,
): Promise<ReplayGatewayTriggerRunResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway trigger replay is not available outside Tauri runtime.");
  }
  return invoke<ReplayGatewayTriggerRunResult>("replay_gateway_trigger_run", { runId });
}

export async function getGatewayTriggerRun(
  runId: string,
): Promise<GetGatewayTriggerRunResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway trigger run detail is not available outside Tauri runtime.");
  }
  return invoke<GetGatewayTriggerRunResult>("get_gateway_trigger_run", { runId });
}

export async function attachGatewaySession(
  sessionId: string,
): Promise<AttachGatewaySessionResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway session attach is not available outside Tauri runtime.");
  }
  return invoke<AttachGatewaySessionResult>("attach_gateway_session", { sessionId });
}

export async function getGatewaySessionSnapshot(
  sessionId: string,
): Promise<GetGatewaySessionSnapshotResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway session snapshot detail is not available outside Tauri runtime.");
  }
  return invoke<GetGatewaySessionSnapshotResult>("get_gateway_session_snapshot", { sessionId });
}

export async function tailGatewaySessionEvents(
  sessionId: string,
  afterCursor?: number | null,
  limit?: number | null,
): Promise<TailGatewaySessionEventsResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway session events are not available outside Tauri runtime.");
  }
  return invoke<TailGatewaySessionEventsResult>("tail_gateway_session_events", {
    sessionId,
    afterCursor,
    limit,
  });
}

export async function enqueueGatewaySessionInput(
  sessionId: string,
  message: string,
): Promise<EnqueueGatewaySessionInputResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Gateway session input enqueue is not available outside Tauri runtime.");
  }
  return invoke<EnqueueGatewaySessionInputResult>("enqueue_gateway_session_input", {
    sessionId,
    message,
  });
}
