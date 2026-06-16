import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  Clock3,
  FileCode,
  GitBranch,
  PanelRightOpen,
  PauseCircle,
  RefreshCw,
  ShieldAlert,
  TestTube,
  XCircle,
} from "lucide-react";
import type { AgentA2AProjection, AgentA2ATaskProjection } from "@/lib/protocol";
import { deriveWorkbenchSummary, normalizeA2ATaskProjection } from "@/lib/workbenchSummary";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

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

function statusLabel(status: string) {
  if (status === "completed") return "完成";
  if (status === "failed") return "失败";
  if (status === "interrupted") return "已中断";
  if (status === "running") return "运行中";
  if (status === "pending") return "等待中";
  return status;
}

function failureKindLabel(kind: string): string {
  switch (kind) {
    case "tool_error": return "工具错误";
    case "smoke_failure": return "冒烟测试失败";
    case "review_rejection": return "审阅拒绝";
    case "arbitration_timeout": return "仲裁超时";
    case "user_cancelled": return "用户取消";
    default: return kind;
  }
}

function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "";
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

function messageKindLabel(kind: string) {
  if (kind === "task_assigned") return "已分派";
  if (kind === "started") return "已启动";
  if (kind === "progress") return "进展";
  if (kind === "artifact_created") return "产物";
  if (kind === "final_result") return "结果";
  if (kind === "failed") return "失败";
  if (kind === "interrupted") return "中断";
  if (kind === "cancelled") return "取消";
  return "记录";
}

function WorktreeReviewPanel({ task: rawTask }: { task: AgentA2ATaskProjection }) {
  const task = normalizeA2ATaskProjection(rawTask);
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

      {task.changed_files.length > 0 && (
        <div className="forge-a2a-worktree-files">
          <span className="forge-a2a-worktree-label">
            <FileCode className="size-3" />
            Diff 变更文件
            {task.changed_file_count != null && task.changed_file_count > task.changed_files.length && (
              <span className="forge-a2a-worktree-files-total">({task.changed_file_count} 总计)</span>
            )}
          </span>
          <div className="forge-a2a-worktree-file-chips">
            {task.changed_files.map((file) => (
              <span key={file} className="forge-a2a-worktree-file-chip" title={file}>
                {file}
              </span>
            ))}
          </div>
        </div>
      )}

      {task.test_report_excerpt && (
        <div className="forge-a2a-worktree-test-excerpt">
          <TestTube className="size-3" />
          <span>{task.test_report_excerpt}</span>
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

function TaskProcess({ task }: { task: AgentA2ATaskProjection }) {
  if (task.messages.length === 0) return null;

  const messages = task.messages.slice(-8);

  return (
    <ol className="forge-a2a-process-list" aria-label={`${task.title} 过程`}>
      {messages.map((message) => (
        <li key={message.message_id} className="forge-a2a-process-step" data-kind={message.kind}>
          <span className="forge-a2a-process-dot" />
          <span className="forge-a2a-process-kind">{messageKindLabel(message.kind)}</span>
          <span className="forge-a2a-process-content">{message.content}</span>
        </li>
      ))}
    </ol>
  );
}

function TaskRow({
  task: rawTask,
  mode = "compact",
}: {
  task: AgentA2ATaskProjection;
  mode?: "compact" | "panel";
}) {
  const task = normalizeA2ATaskProjection(rawTask);
  const Icon = iconFor(task.status);
  const isWorktree = task.execution_mode === "worktree_worker";
  const hasMeta =
    isWorktree &&
    (task.needs_human_review !== null ||
      task.reason_codes.length > 0 ||
      task.tests_passed !== null ||
      task.diff_available !== null ||
      task.changed_files.length > 0 ||
      task.test_report_excerpt !== null);
  const duration = formatDuration(task.duration_ms);
  const runningElapsed = task.status === "running" && task.started_at_ms != null
    ? formatDuration(Date.now() - task.started_at_ms)
    : null;
  const hasRetryable = task.retryable === true && task.status === "failed";

  return (
    <div className="forge-a2a-task-row-wrapper" data-mode={mode}>
      <div className="forge-a2a-task-row" data-status={task.status}>
        <Icon className="size-3" style={{ color: statusColor(task.status) }} />
        <span className="forge-a2a-task-title">{task.title}</span>
        <span className="forge-a2a-task-role">{task.role}</span>
        <span className="forge-a2a-task-status">{statusLabel(task.status)}</span>
        {task.parent_task_id && (
          <span className="forge-a2a-task-lineage" title={`Parent: ${task.parent_task_id}`}>
            <GitBranch className="size-3" />
          </span>
        )}
        {task.artifact_count > 0 && (
          <span
            className="forge-a2a-task-artifact"
            data-kind={task.latest_artifact_kind ?? undefined}
          >
            <FileCode className="size-3" />
            {task.artifact_count}
          </span>
        )}
        {duration && task.status !== "running" && (
          <span className="forge-a2a-task-duration">{duration}</span>
        )}
        {runningElapsed && (
          <span className="forge-a2a-task-duration forge-a2a-task-duration--running">
            {runningElapsed}
          </span>
        )}
        {task.latest_progress && task.status === "running" && (
          <span className="forge-a2a-task-progress">{task.latest_progress}</span>
        )}
        {task.latest_message && (
          <span className="forge-a2a-task-message">{task.latest_message}</span>
        )}
        {task.failure_message && (
          <span className="forge-a2a-task-failure">
            {task.failure_kind && (
              <span className="forge-a2a-task-failure-kind">
                {failureKindLabel(task.failure_kind)}
              </span>
            )}
            {task.failure_message}
            {hasRetryable && (
              <span className="forge-a2a-task-retryable" title="可重试">
                <RefreshCw className="size-3" />
              </span>
            )}
          </span>
        )}
      </div>
      {task.resume_note && (
        <div className="forge-a2a-task-resume-note" title={task.resume_note}>
          <PauseCircle className="size-3" />
          <span>{task.resume_note}</span>
        </div>
      )}
      {mode === "panel" && <TaskProcess task={task} />}
      {hasMeta && <WorktreeReviewPanel task={task} />}
    </div>
  );
}

export function AgentA2AInlineSummary({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) return null;

  const openHub = () => {
    window.dispatchEvent(new CustomEvent("open-hub", { detail: { section: "agents" } }));
  };
  const statusText = state.running_count > 0
    ? `${state.running_count} 个子任务运行中`
    : `${state.completed_count} 个子任务已完成`;

  return (
    <ButtonPrimitive
      type="button"
      className="forge-a2a-inline-summary"
      data-running={state.running_count > 0}
      onClick={openHub}
      title="打开项目档案查看子任务过程"
    >
      <span className="forge-a2a-inline-icon">
        {state.running_count > 0 ? <CircleDashed className="size-3.5" /> : <CheckCircle2 className="size-3.5" />}
      </span>
      <span className="forge-a2a-inline-copy">
        <span className="forge-a2a-inline-title">子任务</span>
        <span className="forge-a2a-inline-detail">{statusText}，查看过程与审阅材料</span>
      </span>
      <PanelRightOpen className="size-3.5" />
    </ButtonPrimitive>
  );
}

export function AgentA2AWorkspace({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) {
    return (
      <section className="forge-a2a-workspace" aria-label="子任务">
        <div className="forge-a2a-workspace-empty">
          <Clock3 className="size-4" />
          <span>本轮还没有子任务。</span>
        </div>
      </section>
    );
  }

  const summary = deriveWorkbenchSummary(state);

  return (
    <section className="forge-a2a-workspace" aria-label="子任务">
      <div className="forge-a2a-workspace-header">
        <div className="forge-a2a-workspace-title-block">
          <span className="forge-a2a-workspace-kicker">Agent Workbench</span>
          <h2 className="forge-a2a-workspace-title">子任务</h2>
        </div>
        <div className="forge-a2a-workspace-stats" aria-label="子任务统计">
          <span data-tone={state.running_count > 0 ? "running" : "idle"}>{state.running_count} 运行</span>
          <span>{state.completed_count} 完成</span>
          {summary.failed > 0 && <span data-tone="failed">{summary.failed} 失败</span>}
          {summary.interrupted > 0 && <span data-tone="interrupted">{summary.interrupted} 中断</span>}
          {summary.reviewNeeded > 0 && <span data-tone="review">{summary.reviewNeeded} 待审阅</span>}
          {summary.retainedWorktrees > 0 && <span data-tone="retained">{summary.retainedWorktrees} 保留工作树</span>}
          {summary.tasksWithDiff > 0 && <span data-tone="diff">{summary.tasksWithDiff} 有变更</span>}
        </div>
      </div>

      <div className="forge-a2a-workspace-task-list">
        {state.tasks.map((task) => (
          <TaskRow key={task.task_id} task={task} mode="panel" />
        ))}
      </div>
    </section>
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
