import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  Clock3,
  FileCode,
  GitBranch,
  PanelRightOpen,
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

function statusLabel(status: string) {
  if (status === "completed") return "完成";
  if (status === "failed") return "失败";
  if (status === "interrupted") return "已中断";
  if (status === "running") return "运行中";
  if (status === "pending") return "等待中";
  return status;
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
  task,
  mode = "compact",
}: {
  task: AgentA2ATaskProjection;
  mode?: "compact" | "panel";
}) {
  const Icon = iconFor(task.status);
  const isWorktree = task.execution_mode === "worktree_worker";
  const hasMeta =
    isWorktree &&
    (task.needs_human_review !== null ||
      task.reason_codes.length > 0 ||
      task.tests_passed !== null);

  return (
    <div className="forge-a2a-task-row-wrapper" data-mode={mode}>
      <div className="forge-a2a-task-row" data-status={task.status}>
        <Icon className="size-3" style={{ color: statusColor(task.status) }} />
        <span className="forge-a2a-task-title">{task.title}</span>
        <span className="forge-a2a-task-role">{task.role}</span>
        <span className="forge-a2a-task-status">{statusLabel(task.status)}</span>
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
    <button
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
    </button>
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
          {state.failed_count > 0 && <span data-tone="failed">{state.failed_count} 失败</span>}
          {state.interrupted_count > 0 && <span data-tone="interrupted">{state.interrupted_count} 中断</span>}
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
