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

export interface GatewayTriggerFormState {
  message: string;
  profileId: string;
  provider: string;
  model: string;
  workspacePath: string;
}

export interface GatewayTriggerInput {
  message: string;
  profile_id?: string;
  provider?: string;
  model?: string;
  workspace_path?: string;
}

export interface GatewayTriggerInputResult {
  input: GatewayTriggerInput | null;
  error: string | null;
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

export function buildGatewayTriggerInput(
  form: GatewayTriggerFormState,
): GatewayTriggerInputResult {
  const message = form.message.trim();
  if (!message) {
    return {
      input: null,
      error: "Message is required.",
    };
  }

  const input: GatewayTriggerInput = { message };
  assignOptional(input, "profile_id", form.profileId);
  assignOptional(input, "provider", form.provider);
  assignOptional(input, "model", form.model);
  assignOptional(input, "workspace_path", form.workspacePath);

  return {
    input,
    error: null,
  };
}

function assignOptional(
  input: GatewayTriggerInput,
  key: keyof Omit<GatewayTriggerInput, "message">,
  value: string,
) {
  const trimmed = value.trim();
  if (trimmed) {
    input[key] = trimmed;
  }
}
