import { AlertTriangle, CircleDashed, FileCheck2, ShieldAlert, TimerReset } from "lucide-react";
import type { LoopTaskRecord } from "@/lib/protocol";
import type { LoopRuntimeSummary } from "@/lib/loopRuntime";

export function LoopTaskPanel({
  task,
  summary,
}: {
  task: LoopTaskRecord;
  summary: LoopRuntimeSummary;
}) {
  const blockers = completionBlockers(task);
  const budget = budgetSnapshot(task);
  const latestEvent = task.latest_event_id?.trim() || "暂无事件";
  const reviewRequired = task.status === "waiting_for_review" || blockers.length > 0;

  return (
    <div className="forge-loop-task-panel" data-tone={summary.tone} data-testid={`loop-task-panel-${task.id}`}>
      <div className="forge-loop-task-panel-header">
        <CircleDashed className="size-3.5" />
        <span className="forge-loop-task-panel-status">{summary.label}</span>
        {summary.needsHumanDecision && (
          <span className="forge-loop-task-panel-decision">等待人工决定</span>
        )}
      </div>
      <div className="forge-loop-task-panel-row">
        <FileCheck2 className="size-3" />
        <span>最新事件</span>
        <code>{latestEvent}</code>
      </div>
      <div className="forge-loop-task-panel-row">
        <TimerReset className="size-3" />
        <span>预算</span>
        <span>{budget}</span>
      </div>
      {blockers.length > 0 && (
        <div className="forge-loop-task-panel-blockers" aria-label="Loop completion blockers">
          <AlertTriangle className="size-3" />
          <span>完成阻塞</span>
          <div className="forge-loop-task-panel-tags">
            {blockers.map((blocker) => (
              <span key={blocker} className="forge-loop-task-panel-tag">
                {readableReason(blocker)}
              </span>
            ))}
          </div>
        </div>
      )}
      {reviewRequired && (
        <div className="forge-loop-task-panel-review" data-testid="loop-review-required">
          <ShieldAlert className="size-3" />
          <span>需要人工审阅，提交仍由人确认</span>
        </div>
      )}
    </div>
  );
}

function completionBlockers(task: LoopTaskRecord): string[] {
  const completion = record(task.completion_result);
  const reasons = completion?.reasons;
  return Array.isArray(reasons) ? reasons.filter((reason): reason is string => typeof reason === "string") : [];
}

function budgetSnapshot(task: LoopTaskRecord): string {
  const snapshot = record(task.latest_budget_snapshot);
  if (!snapshot) return "未知";
  const parts = [];
  const rounds = numberValue(snapshot.model_rounds_used);
  const tools = numberValue(snapshot.tool_calls_used);
  const elapsed = numberValue(snapshot.elapsed_ms);
  if (rounds != null) parts.push(`${rounds} 轮模型`);
  if (tools != null) parts.push(`${tools} 次工具`);
  if (elapsed != null) parts.push(formatDuration(elapsed));
  if (snapshot.has_unknown_cost === true) parts.push("成本未知");
  if (snapshot.budget_exceeded === true) parts.push("预算触发");
  return parts.length > 0 ? parts.join(" / ") : "未知";
}

function readableReason(reason: string): string {
  if (reason.startsWith("missing_required_check:")) return `缺少检查 ${reason.slice("missing_required_check:".length)}`;
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

function record(value: unknown): Record<string, unknown> | null {
  if (value == null || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}
