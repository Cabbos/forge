import type {
  HeadlessResumeApproval,
  HeadlessResumeMode,
  LoopTaskRecord,
  SubagentRuntimePayload,
} from "./protocol.ts";

export type LoopRuntimeTone = "running" | "review" | "waiting" | "success" | "failed" | "muted";

export type HeadlessResumeReadinessState =
  | "desktop_owner_required"
  | "approval_required"
  | "approval_recorded_lease_pending"
  | "approval_expired";

export interface HeadlessResumeReadinessSummary {
  state: HeadlessResumeReadinessState;
  label: string;
  detail: string;
  expiresAtMs: number | null;
}

export interface LoopRuntimeSummaryOptions {
  nowMs?: number;
}

export interface LoopRuntimeSummary {
  taskId: string;
  title: string;
  label: string;
  detail: string;
  tone: LoopRuntimeTone;
  needsHumanDecision: boolean;
  budgetWarning: string | null;
  reviewStatus: string | null;
  commitEligible: boolean;
  commitBlockers: string[];
  humanGateId: string | null;
  lastReviewDecision: Record<string, unknown> | null;
  headlessResumeReadiness: HeadlessResumeReadinessSummary | null;
  rawTask: LoopTaskRecord;
}

export interface LoopRuntimeFactSource {
  loop_task_id?: string | null;
  task_id: string;
  latest_event: SubagentRuntimePayload;
}

export interface LoopRuntimeFact {
  id: string;
  kind: "file_io" | "usage";
  label: string;
  detail: string;
  model?: string | null;
  source?: string | null;
  reason?: string | null;
  inputTokens?: number | null;
  outputTokens?: number | null;
  estimatedCostMicros?: number | null;
  inputTokensUnknown?: boolean;
  outputTokensUnknown?: boolean;
  costUnknown?: boolean;
}

export function summarizeLoopTask(
  task: LoopTaskRecord,
  options: LoopRuntimeSummaryOptions = {},
): LoopRuntimeSummary {
  const completion = readRecord(task.completion_result);
  const completionReasons = stringArray(completion?.reasons);
  const reviewStatus = stringValue(completion?.review_status);
  const commitEligible = completion?.commit_eligible === true;
  const commitBlockers = stringArray(completion?.commit_blockers);
  const humanGateId = stringValue(completion?.human_gate_id);
  const lastReviewDecision = readRecord(completion?.last_review_decision);
  const outcomeMessage = stringValue(readRecord(task.outcome)?.message);
  const budgetSnapshot = readRecord(task.latest_budget_snapshot);
  const budgetWarning = budgetWarningFor(budgetSnapshot);
  const usageDetail = usageDetailFor(budgetSnapshot);
  const headlessResumeReadiness = headlessResumeReadinessForTask(task, options);
  const detail = detailFor(
    task.status,
    completionReasons,
    outcomeMessage,
    usageDetail,
    reviewStatus,
    commitEligible,
    commitBlockers,
  );

  return {
    taskId: task.id,
    title: task.goal,
    label: labelForStatus(task.status, completionReasons, reviewStatus, commitEligible),
    detail,
    tone: toneForStatus(task.status, completionReasons, reviewStatus, commitEligible),
    needsHumanDecision:
      task.status === "waiting_for_input" ||
      task.status === "waiting_for_review" ||
      reviewStatus === "ready_for_review" ||
      reviewStatus === "rejected" ||
      (commitEligible && reviewStatus === "approved"),
    budgetWarning,
    reviewStatus,
    commitEligible,
    commitBlockers,
    humanGateId,
    lastReviewDecision,
    headlessResumeReadiness,
    rawTask: task,
  };
}

export function headlessResumeReadinessForTask(
  task: LoopTaskRecord,
  options: LoopRuntimeSummaryOptions = {},
): HeadlessResumeReadinessSummary | null {
  if (task.status !== "waiting_for_input") return null;

  const nowMs = finiteNumberOrNull(options.nowMs) ?? Date.now();
  return deriveHeadlessResumeReadinessFromState(
    task.headless_resume_mode,
    task.headless_resume_approval,
    nowMs,
  );
}

export function deriveHeadlessResumeReadinessFromState(
  mode: HeadlessResumeMode | null | undefined,
  approval: HeadlessResumeApproval | Record<string, unknown> | null | undefined,
  nowMs: number,
): HeadlessResumeReadinessSummary {
  const approvalRecord = readRecord(approval);
  const approvalNowMs = finiteNumberOrNull(nowMs) ?? 0;
  const expiresAtMs = numberValue(approvalRecord?.expires_at_ms);
  if (approvalRecord) {
    if (expiresAtMs != null && expiresAtMs <= approvalNowMs) {
      return {
        state: "approval_expired",
        label: "approval expired",
        detail: `Approval expired at ${expiresAtMs}; lease/desktop owner pending.`,
        expiresAtMs,
      };
    }
    return {
      state: "approval_recorded_lease_pending",
      label: "approval recorded",
      detail: "Lease/desktop owner pending; waiting_for_input stays active.",
      expiresAtMs,
    };
  }

  if (mode === "require_human_approval" || mode === "approved_for_task") {
    return {
      state: "approval_required",
      label: "approval required",
      detail: "Waiting for durable approval; desktop runtime remains the owner.",
      expiresAtMs: null,
    };
  }

  return {
    state: "desktop_owner_required",
    label: "desktop owner required",
    detail: "Waiting for desktop owner; dry-run status only.",
    expiresAtMs: null,
  };
}

export function runtimeFactsFromSubagents(
  sources: LoopRuntimeFactSource[],
  loopTaskId = firstLoopTaskId(sources),
): LoopRuntimeFact[] {
  if (!loopTaskId) return [];
  const facts: LoopRuntimeFact[] = [];
  for (const source of sources) {
    if (source.loop_task_id !== loopTaskId) continue;
    const fact = runtimeFactFromSource(source);
    if (fact) facts.push(fact);
  }
  return facts;
}

export function runtimeFactsForSubagentTask(
  sources: LoopRuntimeFactSource[],
  taskId: string,
): LoopRuntimeFact[] {
  return sources
    .filter((source) => source.task_id === taskId)
    .map(runtimeFactFromSource)
    .filter((fact): fact is LoopRuntimeFact => fact != null);
}

function runtimeFactFromSource(source: LoopRuntimeFactSource): LoopRuntimeFact | null {
  const event = source.latest_event;
  if (event.type === "file_io") {
    return {
      id: `file:${source.task_id}:${event.path}:${event.operation}`,
      kind: "file_io",
      label: event.operation,
      detail: event.path,
    };
  }
  if (event.type === "usage_recorded") {
    const model = event.model?.trim() || "unknown model";
    const reason = usageReasonLabel(event.reason);
    const inputTokens = finiteNumberOrNull(event.input_tokens);
    const outputTokens = finiteNumberOrNull(event.output_tokens);
    const estimatedCostMicros = finiteNumberOrNull(event.estimated_cost_micros);
    const detail = [
      `input ${numberOrUnknown(inputTokens)}`,
      `output ${numberOrUnknown(outputTokens)}`,
      `cost ${costOrUnknown(estimatedCostMicros)}`,
    ];
    if (reason) detail.push(reason);
    return {
      id: `usage:${source.task_id}:${model}`,
      kind: "usage",
      label: model,
      detail: detail.join(" / "),
      model,
      source: event.source?.trim() || null,
      reason: event.reason ?? null,
      inputTokens,
      outputTokens,
      estimatedCostMicros,
      inputTokensUnknown: inputTokens == null,
      outputTokensUnknown: outputTokens == null,
      costUnknown: estimatedCostMicros == null,
    };
  }
  return null;
}

function usageReasonLabel(reason: string | null | undefined): string | null {
  if (reason === "provider_omitted") return "provider omitted";
  if (reason === "pricing_unknown") return "pricing unknown";
  return null;
}

function labelForStatus(
  status: string,
  completionReasons: string[],
  reviewStatus: string | null,
  commitEligible: boolean,
): string {
  if (commitEligible && reviewStatus === "approved") return "commit eligible after human review";
  if (reviewStatus === "rejected") return "blocked by review";
  if (reviewStatus === "ready_for_review") return "ready for human review";
  if (status === "waiting_for_review") return "等待验证";
  if (status === "waiting_for_input") return "等待输入";
  if (status === "interrupted") return "已中断";
  if (status === "running") return "运行中";
  if (status === "completed") return "完成";
  if (status === "failed") return "失败";
  if (status === "canceled") return "已取消";
  if (completionReasons.length > 0) return "等待验证";
  return "等待中";
}

function toneForStatus(
  status: string,
  completionReasons: string[],
  reviewStatus: string | null,
  commitEligible: boolean,
): LoopRuntimeTone {
  if (status === "completed") return "success";
  if (status === "failed" || status === "canceled" || status === "interrupted") return "failed";
  if (commitEligible) return "success";
  if (reviewStatus === "rejected") return "failed";
  if (reviewStatus === "ready_for_review" || status === "waiting_for_review" || completionReasons.length > 0) return "review";
  if (status === "waiting_for_input") return "waiting";
  if (status === "running") return "running";
  return "muted";
}

function detailFor(
  status: string,
  completionReasons: string[],
  outcomeMessage: string | null,
  usageDetail: string | null,
  reviewStatus: string | null,
  commitEligible: boolean,
  commitBlockers: string[],
): string {
  if (commitEligible && reviewStatus === "approved") return "commit remains human-gated";
  if (reviewStatus === "ready_for_review") return "commit remains human-gated";
  if (reviewStatus === "rejected") {
    return readableReason(commitBlockers[0] ?? completionReasons[0] ?? "review_rejected");
  }
  if (completionReasons.length > 0) return readableReason(completionReasons[0]);
  if (outcomeMessage) return outcomeMessage;
  if (usageDetail) return usageDetail;
  if (status === "waiting_for_input") return "等待用户或桌面运行时输入";
  if (status === "waiting_for_review") return "等待人工审阅";
  if (status === "running") return "运行中";
  return "暂无运行细节";
}

function budgetWarningFor(snapshot: Record<string, unknown> | null): string | null {
  if (!snapshot) return null;
  if (snapshot.budget_exceeded !== true) return null;
  return snapshot.has_unknown_cost === true ? "预算已触发，成本未知" : "预算已触发";
}

function usageDetailFor(snapshot: Record<string, unknown> | null): string | null {
  if (!snapshot) return null;
  const rounds = numberValue(snapshot.model_rounds_used);
  const tools = numberValue(snapshot.tool_calls_used);
  const elapsed = numberValue(snapshot.elapsed_ms);
  const parts = [];
  if (rounds != null) parts.push(`${rounds} 轮模型`);
  if (tools != null) parts.push(`${tools} 次工具`);
  if (elapsed != null) parts.push(formatDuration(elapsed));
  return parts.length > 0 ? parts.join(" / ") : null;
}

function readableReason(reason: string): string {
  if (reason.startsWith("missing_required_check:")) {
    return `缺少检查 ${reason.slice("missing_required_check:".length)}`;
  }
  if (reason.startsWith("review_rejected:")) {
    return `blocked by review: ${reason.slice("review_rejected:".length)}`;
  }
  if (reason === "review_rejected") return "blocked by review";
  if (reason === "missing_human_review") return "commit remains human-gated";
  if (reason === "commit_missing_human_gate") return "commit evidence missing human gate";
  if (reason.startsWith("commit_without_approved_human_gate:")) {
    return `commit evidence lacks approved gate ${reason.slice("commit_without_approved_human_gate:".length)}`;
  }
  if (reason === "task_waiting_for_input") return "等待用户或桌面运行时输入";
  if (reason === "task_waiting_for_review") return "等待人工审阅";
  return reason;
}

function formatDuration(ms: number): string {
  if (ms <= 0) return "<1s";
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  if (minutes < 60) return `${minutes}m ${secs}s`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return `${hours}h ${mins}m`;
}

function firstLoopTaskId(sources: LoopRuntimeFactSource[]): string | null {
  return sources.find((source) => source.loop_task_id)?.loop_task_id ?? null;
}

function readRecord(value: unknown): Record<string, unknown> | null {
  if (value == null || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function finiteNumberOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function numberOrUnknown(value: number | null | undefined): string {
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "unknown";
}

function costOrUnknown(value: number | null | undefined): string {
  return typeof value === "number" && Number.isFinite(value) ? `${value} micros` : "unknown";
}
