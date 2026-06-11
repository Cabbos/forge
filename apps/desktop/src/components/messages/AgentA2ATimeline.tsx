import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  FileCode,
  GitBranch,
  PauseCircle,
  ShieldAlert,
  TestTube,
  XCircle,
} from "lucide-react";
import type { AgentA2AProjection, AgentA2ATaskProjection } from "@/lib/protocol";

function iconFor(status: string) {
  if (status === "completed") return CheckCircle2;
  if (status === "failed") return XCircle;
  if (status === "interrupted") return PauseCircle;
  return CircleDashed;
}

function statusColor(status: string) {
  if (status === "completed") return "var(--forge-success-muted)";
  if (status === "failed") return "var(--forge-danger-muted)";
  if (status === "interrupted") return "var(--forge-amber-muted)";
  return "var(--forge-text-faint)";
}

function WorktreeReviewPanel({ task }: { task: AgentA2ATaskProjection }) {
  const isReviewRequired = task.needs_human_review === true;

  return (
    <div className="forge-a2a-worktree-panel" data-review-required={isReviewRequired}>
      <div className="forge-a2a-worktree-header">
        <ShieldAlert className="size-3" />
        <span className="forge-a2a-worktree-badge">
          {isReviewRequired ? "需要人工审阅" : "审阅状态未知"}
        </span>
        <span className="forge-a2a-worktree-not-merged">未自动合并</span>
      </div>

      <div className="forge-a2a-worktree-grid">
        {task.tests_passed !== null && (
          <div className="forge-a2a-worktree-cell">
            <TestTube className="size-3" />
            <span>测试: {task.tests_passed ? "通过" : "失败"}</span>
          </div>
        )}
        {task.diff_truncated !== null && (
          <div className="forge-a2a-worktree-cell">
            <AlertTriangle className="size-3" />
            <span>Diff: {task.diff_truncated ? "已截断" : "完整"}</span>
          </div>
        )}
        {task.cleaned_up !== null && (
          <div className="forge-a2a-worktree-cell">
            <GitBranch className="size-3" />
            <span>Worktree: {task.cleaned_up ? "已清理" : "已保留"}</span>
          </div>
        )}
      </div>

      {task.reason_codes.length > 0 && (
        <div className="forge-a2a-worktree-reasons">
          <span className="forge-a2a-worktree-label">原因:</span>
          {task.reason_codes.map((code) => (
            <span key={code} className="forge-a2a-worktree-reason-tag">
              {code}
            </span>
          ))}
        </div>
      )}

      {task.worktree_path && (
        <div className="forge-a2a-worktree-path">
          <span className="forge-a2a-worktree-label">路径:</span>
          <code className="forge-a2a-worktree-path-code">{task.worktree_path}</code>
        </div>
      )}

      {task.suggested_action && (
        <div className="forge-a2a-worktree-action">
          <span className="forge-a2a-worktree-label">建议:</span>
          <span className="forge-a2a-worktree-action-text">{task.suggested_action}</span>
        </div>
      )}
    </div>
  );
}

function TaskRow({ task }: { task: AgentA2ATaskProjection }) {
  const Icon = iconFor(task.status);
  const isWorktree = task.execution_mode === "worktree_worker";
  const hasMeta =
    isWorktree &&
    (task.needs_human_review !== null ||
      task.reason_codes.length > 0 ||
      task.tests_passed !== null);

  return (
    <div className="forge-a2a-task-row-wrapper">
      <div className="forge-a2a-task-row" data-status={task.status}>
        <Icon className="size-3" style={{ color: statusColor(task.status) }} />
        <span className="forge-a2a-task-title">{task.title}</span>
        <span className="forge-a2a-task-role">{task.role}</span>
        {task.artifact_count > 0 && (
          <span
            className="forge-a2a-task-artifact"
            data-kind={task.latest_artifact_kind ?? undefined}
          >
            <FileCode className="size-3" />
            {task.artifact_count}
          </span>
        )}
        {task.latest_message && (
          <span className="forge-a2a-task-message">{task.latest_message}</span>
        )}
        {task.failure_message && (
          <span className="forge-a2a-task-failure">{task.failure_message}</span>
        )}
      </div>
      {hasMeta && <WorktreeReviewPanel task={task} />}
    </div>
  );
}

export function AgentA2ATimeline({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) return null;

  return (
    <div className="forge-a2a-timeline" data-testid="agent-a2a-timeline">
      <div className="forge-a2a-summary">
        <span>子任务</span>
        <span>{state.running_count} 运行中</span>
        <span>{state.completed_count} 完成</span>
        {state.failed_count > 0 && <span>{state.failed_count} 失败</span>}
      </div>
      <div className="forge-a2a-task-list">
        {state.tasks.map((task) => (
          <TaskRow key={task.task_id} task={task} />
        ))}
      </div>
    </div>
  );
}
