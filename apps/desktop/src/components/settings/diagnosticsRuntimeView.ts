export type GatewayRuntimeSummaryTone = "pass" | "warn" | "fail";

export interface GatewayRuntimeSnapshotLike {
  ok: boolean;
  message: string;
  uptime_seconds: number;
  active_sessions: number;
  pending_triggers: number;
  pending_session_inputs?: number;
  claimed_triggers: number;
  dead_letter_runs: number;
  recent_runs: unknown[];
  runtime_tasks?: GatewayRuntimeTaskLike[];
}

export interface GatewayRuntimeTaskLike {
  name: string;
  running: boolean;
  last_started_at_ms?: number | null;
  last_error?: string | null;
}

export interface GatewaySessionLike {
  session_id: string;
  provider: string;
  model: string;
  workspace_path: string;
  created_at_ms: number;
  owner_pid?: number | null;
  last_seen_at_ms?: number | null;
  restored_from_registry: boolean;
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

export interface GatewaySessionInputFormState {
  sessionId: string;
  message: string;
}

export interface GatewaySessionInput {
  sessionId: string;
  message: string;
}

export interface GatewaySessionInputResult {
  input: GatewaySessionInput | null;
  error: string | null;
}

export interface GatewayTriggerInput {
  message: string;
  profile_id?: string;
  provider?: string;
  model?: string;
  workspace_path?: string;
}

export interface GatewayPendingTriggerLike {
  id: string;
  message: string;
  profile_id?: string | null;
  provider?: string | null;
  model?: string | null;
  workspace_path?: string | null;
  attempt_count: number;
  claimed_at_ms?: number | null;
  received_at_ms: number;
}

export interface GatewayTriggerRunLike {
  id: string;
  trigger_id: string;
  session_id?: string | null;
  attempt: number;
  status: string;
  message: string;
  started_at_ms: number;
  ended_at_ms: number;
  trigger_message?: string | null;
  profile_id?: string | null;
  provider?: string | null;
  model?: string | null;
  workspace_path?: string | null;
}

export interface GatewayTriggerRow {
  id: string;
  stateLabel: "pending" | "claimed";
  subtitle: string;
  message: string;
  workspacePath: string | null;
}

export interface GatewayTriggerRunRow {
  id: string;
  title: string;
  subtitle: string;
  message: string;
  canReplay: boolean;
}

export type GatewaySessionRowState = "live" | "stale" | "restored";

export interface GatewaySessionRow {
  id: string;
  stateLabel: GatewaySessionRowState;
  runtime: string;
  workspacePath: string | null;
  subtitle: string;
}

export interface GatewaySessionEventLike {
  event_type?: string | null;
  block_id?: string | null;
  content?: string | null;
  message?: string | null;
  tool_name?: string | null;
  command?: string | null;
}

export interface GatewaySessionEventTailLike {
  ok: boolean;
  session_id: string;
  events: GatewaySessionEventLike[];
  next_cursor: number;
  total_events: number;
  cursor_reset: boolean;
}

export interface GatewaySessionEventRow {
  id: string;
  eventType: string;
  label: string;
  preview: string;
}

export interface GatewaySessionEventRowsView {
  summary: string;
  rows: GatewaySessionEventRow[];
  nextCursor: number;
}

export interface GatewayTriggerInputResult {
  input: GatewayTriggerInput | null;
  error: string | null;
}

export const GATEWAY_SESSION_STALE_AFTER_MS = 5 * 60 * 1000;

export function buildGatewayRuntimeSummary(
  status: GatewayRuntimeSnapshotLike,
): GatewayRuntimeSummary {
  const runtimeTasks = status.runtime_tasks ?? [];
  const runningTasks = runtimeTasks.filter((task) => task.running).length;
  const taskCounts =
    runtimeTasks.length > 0 ? ` · ${runningTasks}/${runtimeTasks.length} loops` : "";
  const pendingSessionInputs = status.pending_session_inputs ?? 0;
  const counts = `${status.pending_triggers} pending · ${pendingSessionInputs} inputs · ${status.claimed_triggers} claimed · ${status.dead_letter_runs} dead-letter${taskCounts}`;

  if (!status.ok) {
    return {
      tone: "fail",
      statusText: "不可用",
      counts,
    };
  }

  if (
    status.pending_triggers > 0 ||
    pendingSessionInputs > 0 ||
    status.claimed_triggers > 0 ||
    status.dead_letter_runs > 0 ||
    runtimeTasks.some((task) => !task.running || task.last_error)
  ) {
    return {
      tone: "warn",
      statusText: runtimeTasks.some((task) => !task.running || task.last_error)
        ? "后台循环异常"
        : "有积压",
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

export function buildGatewaySessionInput(
  form: GatewaySessionInputFormState,
): GatewaySessionInputResult {
  const sessionId = form.sessionId.trim();
  if (!sessionId) {
    return {
      input: null,
      error: "Session id is required.",
    };
  }
  const message = form.message.trim();
  if (!message) {
    return {
      input: null,
      error: "Message is required.",
    };
  }

  return {
    input: {
      sessionId,
      message,
    },
    error: null,
  };
}

export function buildGatewayTriggerRows(
  triggers: GatewayPendingTriggerLike[],
): GatewayTriggerRow[] {
  return [...triggers]
    .sort((left, right) => {
      const leftClaimed = left.claimed_at_ms != null;
      const rightClaimed = right.claimed_at_ms != null;
      if (leftClaimed !== rightClaimed) {
        return leftClaimed ? 1 : -1;
      }
      return right.received_at_ms - left.received_at_ms;
    })
    .map((trigger) => {
      const stateLabel = trigger.claimed_at_ms != null ? "claimed" : "pending";
      const runtime = buildRuntimeLabel(trigger.provider, trigger.model);
      const subtitleParts = [
        `profile=${trigger.profile_id?.trim() || "-"}`,
        runtime,
        `attempts=${trigger.attempt_count}`,
        `received=${trigger.received_at_ms}`,
      ].filter(Boolean);

      return {
        id: trigger.id,
        stateLabel,
        subtitle: subtitleParts.join(" · "),
        message: truncateTriggerMessage(trigger.message),
        workspacePath: trigger.workspace_path?.trim() || null,
      };
    });
}

export function buildGatewaySessionRows(
  sessions: GatewaySessionLike[],
  nowMs: number,
): GatewaySessionRow[] {
  return [...sessions]
    .map((session) => {
      const stateLabel = gatewaySessionState(session, nowMs);
      const runtime = buildRuntimeLabel(session.provider, session.model) ?? "-";
      const workspacePath = session.workspace_path.trim() || null;
      const pid = session.owner_pid ?? "-";
      const lastSeen = session.last_seen_at_ms ?? "-";

      return {
        id: session.session_id,
        stateLabel,
        runtime,
        workspacePath,
        subtitle: `pid=${pid} · last_seen=${lastSeen} · created=${session.created_at_ms}`,
      };
    })
    .sort((left, right) => {
      const stateOrder = sessionStateOrder(left.stateLabel) - sessionStateOrder(right.stateLabel);
      if (stateOrder !== 0) return stateOrder;
      return left.id.localeCompare(right.id);
    });
}

export function buildGatewayTriggerRunRows(
  runs: GatewayTriggerRunLike[],
): GatewayTriggerRunRow[] {
  return runs.map((run) => {
    const runtime = buildRuntimeLabel(run.provider, run.model);
    const subtitleParts = [
      `trigger=${run.trigger_id}`,
      run.session_id?.trim() ? `session=${run.session_id.trim()}` : null,
      `profile=${run.profile_id?.trim() || "-"}`,
      runtime,
    ].filter(Boolean);

    return {
      id: run.id,
      title: `${run.status} · attempt ${run.attempt}`,
      subtitle: subtitleParts.join(" · "),
      message: truncateTriggerMessage(run.message),
      canReplay: Boolean(run.trigger_message?.trim()),
    };
  });
}

export function buildGatewaySessionEventRows(
  tail: GatewaySessionEventTailLike,
): GatewaySessionEventRowsView {
  const reset = tail.cursor_reset ? " · reset" : "";
  return {
    summary: `${tail.events.length} events · next=${tail.next_cursor} · total=${tail.total_events}${reset}`,
    rows: tail.events.map((event, index) => {
      const eventType = event.event_type?.trim() || "event";
      const id = event.block_id?.trim() || `${eventType}-${index}`;
      return {
        id,
        eventType,
        label: eventType,
        preview: eventPreview(event),
      };
    }),
    nextCursor: tail.next_cursor,
  };
}

function gatewaySessionState(
  session: GatewaySessionLike,
  nowMs: number,
): GatewaySessionRowState {
  if (session.restored_from_registry) return "restored";
  const lastSeen = session.last_seen_at_ms;
  if (lastSeen != null && Math.max(0, nowMs - lastSeen) > GATEWAY_SESSION_STALE_AFTER_MS) {
    return "stale";
  }
  return "live";
}

function sessionStateOrder(state: GatewaySessionRowState): number {
  switch (state) {
    case "live":
      return 0;
    case "stale":
      return 1;
    case "restored":
      return 2;
  }
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

function buildRuntimeLabel(provider?: string | null, model?: string | null): string | null {
  const cleanProvider = provider?.trim();
  const cleanModel = model?.trim();
  if (cleanProvider && cleanModel) return `${cleanProvider}/${cleanModel}`;
  return cleanProvider || cleanModel || null;
}

function truncateTriggerMessage(message: string, maxLength = 120): string {
  const chars = [...message.trim()];
  if (chars.length <= maxLength) return chars.join("");
  return `${chars.slice(0, maxLength).join("")}…`;
}

function eventPreview(event: GatewaySessionEventLike): string {
  const value =
    event.content?.trim() ||
    event.message?.trim() ||
    event.tool_name?.trim() ||
    event.command?.trim() ||
    "";
  return truncateTriggerMessage(value, 120);
}
