export type GatewayRuntimeSummaryTone = "pass" | "warn" | "fail";

export interface GatewayRuntimeSnapshotLike {
  ok: boolean;
  message: string;
  uptime_seconds: number;
  active_sessions: number;
  pending_triggers: number;
  claimed_triggers: number;
  dead_letter_runs: number;
  recent_runs: unknown[];
}

export interface GatewayRuntimeSummary {
  tone: GatewayRuntimeSummaryTone;
  statusText: string;
  counts: string;
}

export function buildGatewayRuntimeSummary(
  status: GatewayRuntimeSnapshotLike,
): GatewayRuntimeSummary {
  const counts = `${status.pending_triggers} pending · ${status.claimed_triggers} claimed · ${status.dead_letter_runs} dead-letter`;

  if (!status.ok) {
    return {
      tone: "fail",
      statusText: "不可用",
      counts,
    };
  }

  if (
    status.pending_triggers > 0 ||
    status.claimed_triggers > 0 ||
    status.dead_letter_runs > 0
  ) {
    return {
      tone: "warn",
      statusText: "有积压",
      counts,
    };
  }

  return {
    tone: "pass",
    statusText: "运行中",
    counts,
  };
}
