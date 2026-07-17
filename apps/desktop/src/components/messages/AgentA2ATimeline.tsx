import { useState } from "react";
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
import { reviewAgentA2ATasks, type AgentA2AReviewDecision } from "@/lib/ipc/a2a";
import {
  runtimeFactsForSubagentTask,
  type LoopRuntimeFact,
  type LoopRuntimeFactSource,
} from "@/lib/loopRuntime";
import {
  deriveWorkbenchFileView,
  deriveWorkbenchReviewView,
  deriveWorkbenchSummary,
  normalizeA2ATaskProjection,
  type WorkbenchFileView,
  type WorkbenchReviewItem,
  type WorkbenchReviewView,
} from "@/lib/workbenchSummary";
import { runtimeFactSourcesForSubagentTasks } from "@/store/runtime-projections";
import { useStore } from "@/store";
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
  const reviewDecision = task.review_decision ?? null;
  const reviewBadge = isReviewRequired
    ? "需要人工审阅"
    : reviewDecision === "approved"
      ? "审阅通过"
      : reviewDecision === "rejected"
        ? "审阅拒绝"
        : "审阅状态未知";

  return (
    <div className="forge-a2a-worktree-panel" data-review-required={isReviewRequired}>
      <div className="forge-a2a-worktree-header">
        <ShieldAlert className="size-3" />
        <span className="forge-a2a-worktree-badge">
          {reviewBadge}
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

function ReviewWorkbenchItem({
  item,
  tone,
  busyKey,
  onReview,
}: {
  item: WorkbenchReviewItem;
  tone: "queue" | "history";
  busyKey?: string | null;
  onReview?: (taskIds: string[], decision: AgentA2AReviewDecision) => Promise<void>;
}) {
  const disabled = busyKey != null;

  return (
    <li className="forge-a2a-review-item" data-tone={tone}>
      <div className="forge-a2a-review-item-main">
        <span className="forge-a2a-review-item-label">{item.label}</span>
        <span className="forge-a2a-review-item-title">{item.title}</span>
        <span className="forge-a2a-review-item-role">{item.role}</span>
      </div>
      {item.detail && (
        <p className="forge-a2a-review-item-detail">{item.detail}</p>
      )}
      {item.changedFiles.length > 0 && (
        <div className="forge-a2a-review-files" aria-label={`${item.title} 变更文件`}>
          {item.changedFiles.slice(0, 4).map((file) => (
            <span key={file} className="forge-a2a-review-file" title={file}>
              {file}
            </span>
          ))}
        </div>
      )}
      {item.suggestedAction && (
        <p className="forge-a2a-review-action">{item.suggestedAction}</p>
      )}
      {tone === "queue" && onReview && (
        <div className="forge-a2a-review-actions" aria-label={`${item.title} 审阅操作`}>
          <ButtonPrimitive
            type="button"
            className="forge-a2a-review-action-button"
            aria-label={`通过审阅 ${item.title}`}
            disabled={disabled}
            onClick={() => { void onReview([item.taskId], "approve"); }}
          >
            <CheckCircle2 className="size-3" />
            <span>通过</span>
          </ButtonPrimitive>
          <ButtonPrimitive
            type="button"
            className="forge-a2a-review-action-button"
            data-tone="reject"
            aria-label={`拒绝审阅 ${item.title}`}
            disabled={disabled}
            onClick={() => { void onReview([item.taskId], "reject"); }}
          >
            <XCircle className="size-3" />
            <span>拒绝</span>
          </ButtonPrimitive>
        </div>
      )}
    </li>
  );
}

function WorkbenchReviewSummary({
  view,
  busyKey,
  onReview,
}: {
  view: WorkbenchReviewView;
  busyKey?: string | null;
  onReview?: (taskIds: string[], decision: AgentA2AReviewDecision) => Promise<void>;
}) {
  if (view.queue.length === 0 && view.history.length === 0) return null;
  const queueIds = view.queue.map((item) => item.taskId);
  const disabled = busyKey != null;

  return (
    <div className="forge-a2a-review-summary" aria-label="审阅摘要">
      {view.queue.length > 0 && (
        <section className="forge-a2a-review-section" aria-label="审阅队列">
          <div className="forge-a2a-review-section-header">
            <span className="forge-a2a-review-section-title">审阅队列</span>
            <div className="forge-a2a-review-section-meta">
              <span className="forge-a2a-review-section-count">{view.queue.length} 个待审阅</span>
              {onReview && (
                <div className="forge-a2a-review-bulk-actions" aria-label="批量审阅操作">
                  <ButtonPrimitive
                    type="button"
                    className="forge-a2a-review-action-button"
                    aria-label="全部通过审阅"
                    disabled={disabled}
                    onClick={() => { void onReview(queueIds, "approve"); }}
                  >
                    <CheckCircle2 className="size-3" />
                    <span>全部通过</span>
                  </ButtonPrimitive>
                  <ButtonPrimitive
                    type="button"
                    className="forge-a2a-review-action-button"
                    data-tone="reject"
                    aria-label="全部拒绝审阅"
                    disabled={disabled}
                    onClick={() => { void onReview(queueIds, "reject"); }}
                  >
                    <XCircle className="size-3" />
                    <span>全部拒绝</span>
                  </ButtonPrimitive>
                </div>
              )}
            </div>
          </div>
          <ul className="forge-a2a-review-list">
            {view.queue.map((item) => (
              <ReviewWorkbenchItem
                key={item.taskId}
                item={item}
                tone="queue"
                busyKey={busyKey}
                onReview={onReview}
              />
            ))}
          </ul>
        </section>
      )}

      {view.history.length > 0 && (
        <section className="forge-a2a-review-section" aria-label="审阅历史">
          <div className="forge-a2a-review-section-header">
            <span className="forge-a2a-review-section-title">审阅历史</span>
            <span className="forge-a2a-review-section-count">{view.history.length} 条记录</span>
          </div>
          <ul className="forge-a2a-review-list">
            {view.history.map((item) => (
              <ReviewWorkbenchItem key={item.taskId} item={item} tone="history" />
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function WorkbenchFileSummary({ view }: { view: WorkbenchFileView }) {
  if (view.files.length === 0) return null;

  return (
    <section className="forge-a2a-file-summary" aria-label="文件视图">
      <div className="forge-a2a-file-summary-header">
        <span className="forge-a2a-file-summary-title">文件视图</span>
        <span className="forge-a2a-file-summary-count">
          {view.visibleFileCount} 可见 / {view.reportedFileCount} 报告
        </span>
        {view.hiddenFileCount > 0 && (
          <span className="forge-a2a-file-summary-hidden">
            {view.hiddenFileCount} 未展开
          </span>
        )}
      </div>
      <div className="forge-a2a-file-summary-list">
        {view.files.slice(0, 10).map((item) => (
          <div key={item.file} className="forge-a2a-file-summary-row">
            <FileCode className="size-3" />
            <code className="forge-a2a-file-summary-path" title={item.file}>
              {item.file}
            </code>
            <span className="forge-a2a-file-summary-task-count">
              {item.taskIds.length} 任务
            </span>
          </div>
        ))}
      </div>
    </section>
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

function RuntimeFactSections({ facts }: { facts: LoopRuntimeFact[] }) {
  if (facts.length === 0) return null;
  const fileFacts = facts.filter((fact) => fact.kind === "file_io");
  const usageFacts = facts.filter((fact) => fact.kind === "usage");

  return (
    <div className="forge-a2a-runtime-facts" data-testid="a2a-runtime-facts">
      {fileFacts.length > 0 && (
        <section className="forge-a2a-runtime-fact-section" aria-label="Runtime file IO">
          <span className="forge-a2a-runtime-fact-title">文件 IO</span>
          <div className="forge-a2a-runtime-fact-list">
            {fileFacts.map((fact) => (
              <div key={fact.id} className="forge-a2a-runtime-fact-row">
                <span>{fact.label}</span>
                <code>{fact.detail}</code>
              </div>
            ))}
          </div>
        </section>
      )}
      {usageFacts.length > 0 && (
        <section className="forge-a2a-runtime-fact-section" aria-label="Runtime usage">
          <span className="forge-a2a-runtime-fact-title">用量</span>
          <div className="forge-a2a-runtime-fact-list">
            {usageFacts.map((fact) => (
              <div key={fact.id} className="forge-a2a-runtime-fact-row">
                <span>{fact.label}</span>
                <span>{fact.detail}</span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

function TaskRow({
  task: rawTask,
  mode = "compact",
  runtimeFacts = [],
}: {
  task: AgentA2ATaskProjection;
  mode?: "compact" | "panel";
  runtimeFacts?: LoopRuntimeFact[];
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
  const childTaskCount = task.child_task_ids.length;

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
        {childTaskCount > 0 && (
          <span
            className="forge-a2a-task-lineage forge-a2a-task-lineage--children"
            title={`Children: ${task.child_task_ids.join(", ")}`}
            aria-label={`子任务 ${childTaskCount} 个: ${task.child_task_ids.join(", ")}`}
          >
            <GitBranch className="size-3" />
            {childTaskCount}
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
      <RuntimeFactSections facts={runtimeFacts} />
      {mode === "panel" && <TaskProcess task={task} />}
      {hasMeta && <WorktreeReviewPanel task={task} />}
    </div>
  );
}

export function AgentA2AFocusedTask({
  task,
  runtimeFacts = [],
}: {
  task: AgentA2ATaskProjection;
  runtimeFacts?: LoopRuntimeFact[];
}) {
  return <TaskRow task={task} mode="panel" runtimeFacts={runtimeFacts} />;
}

export function AgentA2AInlineSummary({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) return null;

  const openWorkPanel = () => {
    window.dispatchEvent(new Event("open-work-panel"));
  };
  const statusText = state.running_count > 0
    ? `${state.running_count} 个子任务运行中`
    : `${state.completed_count} 个子任务已完成`;

  return (
    <ButtonPrimitive
      type="button"
      className="forge-a2a-inline-summary"
      data-running={state.running_count > 0}
      onClick={openWorkPanel}
      title="打开工作面板查看子任务过程"
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

export function AgentA2AWorkspace({
  state,
  sessionId,
}: {
  state: AgentA2AProjection | null;
  sessionId?: string | null;
}) {
  const [reviewBusyKey, setReviewBusyKey] = useState<string | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const subagentRuntimeByTask = useStore((s) => s.subagentRuntimeByTask);

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
  const reviewView = deriveWorkbenchReviewView(state);
  const fileView = deriveWorkbenchFileView(state);
  const taskIds = new Set(state.tasks.map((task) => task.task_id));
  const runtimeSources = runtimeFactSourcesForSubagentTasks({
    entries: subagentRuntimeByTask,
    taskIds,
    sessionId,
  });
  const handleReview = async (reviewTaskIds: string[], decision: AgentA2AReviewDecision) => {
    if (!sessionId || reviewTaskIds.length === 0) return;
    const key = reviewTaskIds.length === 1 ? `${reviewTaskIds[0]}:${decision}` : `bulk:${decision}`;
    setReviewBusyKey(key);
    setReviewError(null);
    try {
      const next = await reviewAgentA2ATasks({
        sessionId,
        taskIds: reviewTaskIds,
        decision,
        message: null,
        loopTaskId: loopTaskIdForReview(runtimeSources, reviewTaskIds),
      });
      useStore.setState((current) => {
        const agentA2ABySession = new Map(current.agentA2ABySession);
        agentA2ABySession.set(next.session_id, next.state);
        return { agentA2ABySession };
      });
    } catch (error) {
      setReviewError(error instanceof Error ? error.message : String(error));
    } finally {
      setReviewBusyKey(null);
    }
  };

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

      <WorkbenchReviewSummary
        view={reviewView}
        busyKey={reviewBusyKey}
        onReview={sessionId ? handleReview : undefined}
      />
      <WorkbenchFileSummary view={fileView} />
      {reviewError && (
        <p className="forge-a2a-review-error" role="alert">
          {reviewError}
        </p>
      )}

      <div className="forge-a2a-workspace-task-list">
        {state.tasks.map((task) => (
          <TaskRow
            key={task.task_id}
            task={task}
            mode="panel"
            runtimeFacts={runtimeFactsForSubagentTask(runtimeSources, task.task_id)}
          />
        ))}
      </div>
    </section>
  );
}

function loopTaskIdForReview(
  runtimeSources: LoopRuntimeFactSource[],
  taskIds: string[],
): string | null {
  const loopTaskIds = new Set<string>();
  for (const taskId of taskIds) {
    const idsForTask = runtimeSources
      .filter((source) => source.task_id === taskId)
      .map((source) => source.loop_task_id?.trim() || null)
      .filter((loopTaskId): loopTaskId is string => loopTaskId != null);
    if (idsForTask.length === 0) return null;
    for (const loopTaskId of idsForTask) loopTaskIds.add(loopTaskId);
  }
  return loopTaskIds.size === 1 ? [...loopTaskIds][0] : null;
}

export function AgentA2ATimeline({
  state,
  sessionId,
}: {
  state: AgentA2AProjection | null;
  sessionId?: string | null;
}) {
  const subagentRuntimeByTask = useStore((s) => s.subagentRuntimeByTask);
  if (!state || state.tasks.length === 0) return null;
  const taskIds = new Set(state.tasks.map((task) => task.task_id));
  const runtimeSources = sessionId
    ? runtimeFactSourcesForSubagentTasks({
        entries: subagentRuntimeByTask,
        taskIds,
        sessionId,
      })
    : [];

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
          <TaskRow
            key={task.task_id}
            task={task}
            runtimeFacts={runtimeFactsForSubagentTask(runtimeSources, task.task_id)}
          />
        ))}
      </div>
    </div>
  );
}
