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

/**
 * Pure helper: apply defaults to an AgentA2ATaskProjection from a possibly-sparse
 * JSON payload (e.g., from older Rust backend that doesn't emit Phase 4-A fields).
 * Always safe to call — mutates nothing, returns a new object.
 */
export function normalizeA2ATaskProjection(task: AgentA2ATaskProjection): AgentA2ATaskProjection {
  return {
    ...task,
    parent_task_id: task.parent_task_id ?? null,
    created_at_ms: task.created_at_ms ?? 0,
    started_at_ms: task.started_at_ms ?? null,
    ended_at_ms: task.ended_at_ms ?? null,
    duration_ms: task.duration_ms ?? null,
    retryable: task.retryable ?? null,
    failure_kind: task.failure_kind ?? null,
    resume_note: task.resume_note ?? null,
    latest_progress: task.latest_progress ?? null,
    // Phase 4-B fields
    diff_available: task.diff_available ?? null,
    changed_file_count: task.changed_file_count ?? null,
    changed_files: task.changed_files ?? [],
    test_report_excerpt: task.test_report_excerpt ?? null,
  };
}
