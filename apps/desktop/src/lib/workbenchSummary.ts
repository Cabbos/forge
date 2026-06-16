import type { AgentA2AProjection, AgentA2ATaskProjection } from "@/lib/protocol";

export interface WorkbenchSummary {
  /** Number of tasks needing human review. */
  reviewNeeded: number;
  /** Number of failed tasks. */
  failed: number;
  /** Number of interrupted tasks. */
  interrupted: number;
  /** Number of retained worktrees (not cleaned up, worktree_path present). */
  retainedWorktrees: number;
  /** Total number of tasks. */
  total: number;
  /** Phase 4-B: unique changed files visible in projected task path lists. */
  changedFiles: number;
  /** Phase 4-B: number of tasks that have a diff summary artifact. */
  tasksWithDiff: number;
}

export interface WorkbenchReviewItem {
  taskId: string;
  title: string;
  role: string;
  status: string;
  label: string;
  detail: string | null;
  changedFiles: string[];
  suggestedAction: string | null;
  worktreePath: string | null;
  reviewDecision: string | null;
  reviewedAtMs: number | null;
}

export interface WorkbenchReviewView {
  queue: WorkbenchReviewItem[];
  history: WorkbenchReviewItem[];
}

export interface WorkbenchFileItem {
  file: string;
  taskIds: string[];
  taskTitles: string[];
}

export interface WorkbenchFileView {
  files: WorkbenchFileItem[];
  visibleFileCount: number;
  reportedFileCount: number;
  hiddenFileCount: number;
  tasksWithFiles: number;
}

/** Pure helper: derive workbench summary counts from an AgentA2AProjection. */
export function deriveWorkbenchSummary(state: AgentA2AProjection | null): WorkbenchSummary {
  if (!state || state.tasks.length === 0) {
    return { reviewNeeded: 0, failed: 0, interrupted: 0, retainedWorktrees: 0, total: 0, changedFiles: 0, tasksWithDiff: 0 };
  }
  let reviewNeeded = 0;
  let retainedWorktrees = 0;
  let tasksWithDiff = 0;
  const allChangedFiles = new Set<string>();
  for (const rawTask of state.tasks) {
    const task = normalizeA2ATaskProjection(rawTask);
    if (task.needs_human_review === true) reviewNeeded += 1;
    if (task.cleaned_up === false && task.worktree_path != null) retainedWorktrees += 1;
    if (task.changed_file_count != null && task.changed_file_count > 0) {
      tasksWithDiff += 1;
      for (const f of task.changed_files) {
        allChangedFiles.add(f);
      }
    }
  }
  return {
    reviewNeeded,
    failed: state.failed_count,
    interrupted: state.interrupted_count,
    retainedWorktrees,
    total: state.tasks.length,
    changedFiles: allChangedFiles.size,
    tasksWithDiff,
  };
}

/** Pure helper: derive review queue/history rows from worktree-worker projections. */
export function deriveWorkbenchReviewView(state: AgentA2AProjection | null): WorkbenchReviewView {
  const queue: WorkbenchReviewItem[] = [];
  const history: WorkbenchReviewItem[] = [];
  if (!state || state.tasks.length === 0) return { queue, history };

  for (const rawTask of state.tasks) {
    const task = normalizeA2ATaskProjection(rawTask);
    if (task.execution_mode !== "worktree_worker") continue;

    const reviewDecision = task.review_decision ?? null;
    const item: WorkbenchReviewItem = {
      taskId: task.task_id,
      title: task.title,
      role: task.role,
      status: task.status,
      label: reviewLabel(reviewDecision, task.failure_kind),
      detail: task.failure_message ?? task.latest_message ?? null,
      changedFiles: task.changed_files,
      suggestedAction: task.suggested_action,
      worktreePath: task.worktree_path,
      reviewDecision,
      reviewedAtMs: task.reviewed_at_ms ?? null,
    };

    if (task.needs_human_review === true) {
      queue.push(item);
    } else if (reviewDecision === "approved" || reviewDecision === "rejected" || task.failure_kind === "review_rejection") {
      history.push(item);
    }
  }

  return { queue, history };
}

/** Pure helper: derive a file-centric workbench view from diff-derived projection fields. */
export function deriveWorkbenchFileView(state: AgentA2AProjection | null): WorkbenchFileView {
  const byFile = new Map<string, WorkbenchFileItem>();
  let reportedFileCount = 0;
  let tasksWithFiles = 0;
  let hiddenFileCount = 0;

  for (const rawTask of state?.tasks ?? []) {
    const task = normalizeA2ATaskProjection(rawTask);
    const reported = task.changed_file_count ?? 0;
    if (reported <= 0) continue;

    tasksWithFiles += 1;
    reportedFileCount += reported;
    const visibleFilesForTask = new Set<string>();
    for (const file of task.changed_files) {
      const key = file.trim();
      if (!key) continue;
      visibleFilesForTask.add(key);
      const existing = byFile.get(key) ?? {
        file: key,
        taskIds: [],
        taskTitles: [],
      };
      if (!existing.taskIds.includes(task.task_id)) {
        existing.taskIds.push(task.task_id);
        existing.taskTitles.push(task.title);
      }
      byFile.set(key, existing);
    }
    hiddenFileCount += Math.max(0, reported - visibleFilesForTask.size);
  }

  const files = [...byFile.values()].sort((a, b) => {
    const taskDelta = b.taskIds.length - a.taskIds.length;
    return taskDelta !== 0 ? taskDelta : a.file.localeCompare(b.file);
  });
  const visibleFileCount = files.length;

  return {
    files,
    visibleFileCount,
    reportedFileCount,
    hiddenFileCount,
    tasksWithFiles,
  };
}

/**
 * Pure helper: apply defaults to an AgentA2ATaskProjection from a possibly-sparse
 * JSON payload (e.g., from older Rust backend that doesn't emit Phase 4-A fields).
 * Always safe to call — mutates nothing, returns a new object.
 */
export function normalizeA2ATaskProjection(task: AgentA2ATaskProjection): AgentA2ATaskProjection {
  return {
    ...task,
    review_decision: task.review_decision ?? null,
    reviewed_at_ms: task.reviewed_at_ms ?? null,
    parent_task_id: task.parent_task_id ?? null,
    created_at_ms: task.created_at_ms ?? 0,
    started_at_ms: task.started_at_ms ?? null,
    ended_at_ms: task.ended_at_ms ?? null,
    duration_ms: task.duration_ms ?? null,
    retryable: task.retryable ?? null,
    failure_kind: task.failure_kind ?? null,
    resume_note: task.resume_note ?? null,
    latest_progress: task.latest_progress ?? null,
    // Phase 4-C fields
    lease_owner: task.lease_owner ?? null,
    lease_acquired_at_ms: task.lease_acquired_at_ms ?? null,
    lease_expires_at_ms: task.lease_expires_at_ms ?? null,
    last_heartbeat_at_ms: task.last_heartbeat_at_ms ?? null,
    attempt_count: task.attempt_count ?? 0,
    max_attempts: task.max_attempts ?? 3,
    // Phase 4-B fields
    diff_available: task.diff_available ?? null,
    changed_file_count: task.changed_file_count ?? null,
    changed_files: task.changed_files ?? [],
    test_report_excerpt: task.test_report_excerpt ?? null,
  };
}

function reviewLabel(reviewDecision: string | null, failureKind: string | null): string {
  if (reviewDecision === "approved") return "审阅通过";
  if (reviewDecision === "rejected" || failureKind === "review_rejection") return "审阅拒绝";
  return "需要审阅";
}
