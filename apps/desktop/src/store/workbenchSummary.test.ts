import { describe, it } from "node:test";
import assert from "node:assert";
import {
  deriveWorkbenchFileView,
  deriveWorkbenchReviewView,
  deriveWorkbenchSummary,
  normalizeA2ATaskProjection,
} from "../lib/workbenchSummary.ts";
import type { AgentA2AProjection, AgentA2ATaskProjection } from "../lib/protocol.ts";

function task(overrides: Partial<AgentA2ATaskProjection> = {}): AgentA2ATaskProjection {
  return {
    task_id: overrides.task_id ?? "task-1",
    agent_id: overrides.agent_id ?? "agent-1",
    role: overrides.role ?? "implementer",
    execution_mode: overrides.execution_mode ?? "read_only",
    status: overrides.status ?? "completed",
    title: overrides.title ?? "Test task",
    messages: overrides.messages ?? [],
    latest_message: overrides.latest_message ?? null,
    failure_message: overrides.failure_message ?? null,
    updated_at_ms: overrides.updated_at_ms ?? 0,
    artifact_count: overrides.artifact_count ?? 0,
    latest_artifact_kind: overrides.latest_artifact_kind ?? null,
    latest_artifact_title: overrides.latest_artifact_title ?? null,
    needs_human_review: overrides.needs_human_review ?? null,
    reason_codes: overrides.reason_codes ?? [],
    tests_passed: overrides.tests_passed ?? null,
    diff_truncated: overrides.diff_truncated ?? null,
    worktree_path: overrides.worktree_path ?? null,
    cleaned_up: overrides.cleaned_up ?? null,
    suggested_action: overrides.suggested_action ?? null,
    // Phase 4-A fields
    parent_task_id: overrides.parent_task_id ?? null,
    created_at_ms: overrides.created_at_ms ?? 0,
    started_at_ms: overrides.started_at_ms ?? null,
    ended_at_ms: overrides.ended_at_ms ?? null,
    duration_ms: overrides.duration_ms ?? null,
    retryable: overrides.retryable ?? null,
    failure_kind: overrides.failure_kind ?? null,
    resume_note: overrides.resume_note ?? null,
    latest_progress: overrides.latest_progress ?? null,
    // Phase 4-C fields
    lease_owner: overrides.lease_owner ?? null,
    lease_acquired_at_ms: overrides.lease_acquired_at_ms ?? null,
    lease_expires_at_ms: overrides.lease_expires_at_ms ?? null,
    last_heartbeat_at_ms: overrides.last_heartbeat_at_ms ?? null,
    attempt_count: overrides.attempt_count ?? 0,
    max_attempts: overrides.max_attempts ?? 3,
    // Phase 4-B fields
    diff_available: overrides.diff_available ?? null,
    changed_file_count: overrides.changed_file_count ?? null,
    changed_files: overrides.changed_files ?? [],
    test_report_excerpt: overrides.test_report_excerpt ?? null,
  };
}

function projection(tasks: AgentA2ATaskProjection[]): AgentA2AProjection {
  let running = 0;
  let completed = 0;
  let failed = 0;
  let interrupted = 0;
  for (const t of tasks) {
    if (t.status === "running") running++;
    else if (t.status === "completed") completed++;
    else if (t.status === "failed") failed++;
    else if (t.status === "interrupted") interrupted++;
  }
  return { running_count: running, completed_count: completed, failed_count: failed, interrupted_count: interrupted, tasks };
}

describe("deriveWorkbenchSummary", () => {
  it("returns zeroed summary for null input", () => {
    const result = deriveWorkbenchSummary(null);
    assert.strictEqual(result.total, 0);
    assert.strictEqual(result.reviewNeeded, 0);
    assert.strictEqual(result.failed, 0);
    assert.strictEqual(result.interrupted, 0);
    assert.strictEqual(result.retainedWorktrees, 0);
  });

  it("returns zeroed summary for empty tasks", () => {
    const result = deriveWorkbenchSummary(projection([]));
    assert.strictEqual(result.total, 0);
    assert.strictEqual(result.reviewNeeded, 0);
  });

  it("counts review-needed tasks", () => {
    const tasks = [
      task({ task_id: "t1", needs_human_review: true }),
      task({ task_id: "t2", needs_human_review: false }),
      task({ task_id: "t3", needs_human_review: null }),
      task({ task_id: "t4", needs_human_review: true }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.reviewNeeded, 2);
    assert.strictEqual(result.total, 4);
  });

  it("counts failed and interrupted from projection-level counts", () => {
    const tasks = [
      task({ task_id: "t1", status: "failed" }),
      task({ task_id: "t2", status: "failed" }),
      task({ task_id: "t3", status: "interrupted" }),
      task({ task_id: "t4", status: "completed" }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.failed, 2);
    assert.strictEqual(result.interrupted, 1);
    assert.strictEqual(result.total, 4);
  });

  it("counts retained worktrees (not cleaned up, has path)", () => {
    const tasks = [
      task({ task_id: "t1", cleaned_up: false, worktree_path: "/tmp/wt1", execution_mode: "worktree_worker" }),
      task({ task_id: "t2", cleaned_up: true, worktree_path: "/tmp/wt2", execution_mode: "worktree_worker" }),
      task({ task_id: "t3", cleaned_up: false, worktree_path: "/tmp/wt3", execution_mode: "worktree_worker" }),
      task({ task_id: "t4", cleaned_up: false, worktree_path: null, execution_mode: "worktree_worker" }),
      task({ task_id: "t5", cleaned_up: null, worktree_path: "/tmp/wt5", execution_mode: "worktree_worker" }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.retainedWorktrees, 2); // t1 and t3 only
  });

  it("retained worktrees only count when cleaned_up is explicitly false", () => {
    const tasks = [
      task({ task_id: "t1", cleaned_up: false, worktree_path: "/tmp/a" }),
      task({ task_id: "t2", cleaned_up: null, worktree_path: "/tmp/b" }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.retainedWorktrees, 1);
  });

  it("returns zero review-needed when all tasks have no review flag", () => {
    const tasks = [
      task({ task_id: "t1", needs_human_review: null }),
      task({ task_id: "t2", needs_human_review: false }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.reviewNeeded, 0);
  });

  // ── Phase 4-B: diff-derived summary fields ──

  it("counts tasksWithDiff and visible changedFiles from tasks with diff summaries", () => {
    const tasks = [
      task({ task_id: "t1", changed_file_count: 3, changed_files: ["a.rs", "b.rs", "c.rs"] }),
      task({ task_id: "t2", changed_file_count: 2, changed_files: ["d.rs", "e.rs"] }),
      task({ task_id: "t3", changed_file_count: null, changed_files: [] }),
      task({ task_id: "t4", changed_file_count: 2, changed_files: ["a.rs", "f.rs"] }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.tasksWithDiff, 3); // t1, t2, t4
    assert.strictEqual(result.changedFiles, 6); // visible paths a, b, c, d, e, f (deduped)
  });

  it("returns zero changedFiles and tasksWithDiff when no tasks have diffs", () => {
    const tasks = [
      task({ task_id: "t1", changed_file_count: null, changed_files: [] }),
      task({ task_id: "t2", changed_file_count: 0, changed_files: [] }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.tasksWithDiff, 0);
    assert.strictEqual(result.changedFiles, 0);
  });

  it("returns zero changedFiles when tasks have count 0", () => {
    const tasks = [
      task({ task_id: "t1", changed_file_count: 0, changed_files: [] }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.tasksWithDiff, 0);
    assert.strictEqual(result.changedFiles, 0);
  });

  it("deduplicates visible changedFiles across tasks", () => {
    const tasks = [
      task({ task_id: "t1", changed_file_count: 2, changed_files: ["shared.rs", "only_t1.rs"] }),
      task({ task_id: "t2", changed_file_count: 2, changed_files: ["shared.rs", "only_t2.rs"] }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.tasksWithDiff, 2);
    assert.strictEqual(result.changedFiles, 3); // shared, only_t1, only_t2
  });

  it("counts only projected paths when a task reports a larger hidden total", () => {
    const tasks = [
      task({
        task_id: "t1",
        changed_file_count: 12,
        changed_files: ["visible_1.rs", "visible_2.rs"],
      }),
    ];
    const result = deriveWorkbenchSummary(projection(tasks));
    assert.strictEqual(result.tasksWithDiff, 1);
    assert.strictEqual(result.changedFiles, 2);
  });

  it("returns zeroed summary (including Phase 4-B fields) for null input", () => {
    const result = deriveWorkbenchSummary(null);
    assert.strictEqual(result.total, 0);
    assert.strictEqual(result.changedFiles, 0);
    assert.strictEqual(result.tasksWithDiff, 0);
  });

  it("returns zeroed summary for empty tasks array", () => {
    const result = deriveWorkbenchSummary(projection([]));
    assert.strictEqual(result.total, 0);
    assert.strictEqual(result.changedFiles, 0);
    assert.strictEqual(result.tasksWithDiff, 0);
  });

  it("handles sparse legacy task projections without Phase 4 fields", () => {
    const legacyTask = {
      task_id: "legacy",
      agent_id: "agent",
      role: "implementer",
      execution_mode: "worktree_worker",
      status: "completed",
      title: "Legacy task",
      messages: [],
      latest_message: null,
      failure_message: null,
      updated_at_ms: 0,
      artifact_count: 0,
      latest_artifact_kind: null,
      latest_artifact_title: null,
      needs_human_review: null,
      reason_codes: [],
      tests_passed: null,
      diff_truncated: null,
      worktree_path: null,
      cleaned_up: null,
      suggested_action: null,
    } as unknown as AgentA2ATaskProjection;

    const result = deriveWorkbenchSummary(projection([legacyTask]));
    assert.strictEqual(result.total, 1);
    assert.strictEqual(result.changedFiles, 0);
    assert.strictEqual(result.tasksWithDiff, 0);
  });

  it("normalizes missing lease fields on sparse legacy task projections", () => {
    const legacyTask = {
      task_id: "legacy",
      agent_id: "agent",
      role: "implementer",
      execution_mode: "worktree_worker",
      status: "running",
      title: "Legacy worker",
      messages: [],
      latest_message: null,
      failure_message: null,
      updated_at_ms: 0,
      artifact_count: 0,
      latest_artifact_kind: null,
      latest_artifact_title: null,
      needs_human_review: null,
      reason_codes: [],
      tests_passed: null,
      diff_truncated: null,
      worktree_path: null,
      cleaned_up: null,
      suggested_action: null,
    } as unknown as AgentA2ATaskProjection;

    const normalized = normalizeA2ATaskProjection(legacyTask);

    assert.strictEqual(normalized.lease_owner, null);
    assert.strictEqual(normalized.lease_acquired_at_ms, null);
    assert.strictEqual(normalized.lease_expires_at_ms, null);
    assert.strictEqual(normalized.last_heartbeat_at_ms, null);
    assert.strictEqual(normalized.attempt_count, 0);
    assert.strictEqual(normalized.max_attempts, 3);
  });
});

describe("deriveWorkbenchFileView", () => {
  it("groups visible changed files by file path and task", () => {
    const tasks = [
      task({
        task_id: "t1",
        title: "Worker one",
        changed_file_count: 2,
        changed_files: ["shared.ts", "only-one.ts"],
      }),
      task({
        task_id: "t2",
        title: "Worker two",
        changed_file_count: 2,
        changed_files: ["shared.ts", "only-two.ts"],
      }),
    ];

    const result = deriveWorkbenchFileView(projection(tasks));

    assert.strictEqual(result.visibleFileCount, 3);
    assert.strictEqual(result.reportedFileCount, 4);
    assert.strictEqual(result.hiddenFileCount, 0);
    assert.strictEqual(result.tasksWithFiles, 2);
    assert.deepStrictEqual(result.files[0], {
      file: "shared.ts",
      taskIds: ["t1", "t2"],
      taskTitles: ["Worker one", "Worker two"],
    });
  });

  it("counts hidden files from per-task projection truncation", () => {
    const result = deriveWorkbenchFileView(projection([
      task({
        task_id: "t1",
        changed_file_count: 3,
        changed_files: ["visible-one.ts"],
      }),
      task({
        task_id: "t2",
        changed_file_count: 2,
        changed_files: ["visible-two.ts", "visible-two.ts"],
      }),
    ]));

    assert.strictEqual(result.visibleFileCount, 2);
    assert.strictEqual(result.reportedFileCount, 5);
    assert.strictEqual(result.hiddenFileCount, 3);
    assert.strictEqual(result.tasksWithFiles, 2);
  });

  it("returns an empty view for null or no diff tasks", () => {
    const empty = deriveWorkbenchFileView(null);
    assert.strictEqual(empty.visibleFileCount, 0);
    assert.strictEqual(empty.reportedFileCount, 0);
    assert.strictEqual(empty.hiddenFileCount, 0);
    assert.deepStrictEqual(empty.files, []);

    const noDiff = deriveWorkbenchFileView(projection([
      task({ task_id: "t1", changed_file_count: 0, changed_files: [] }),
      task({ task_id: "t2", changed_file_count: null, changed_files: ["ignored.ts"] }),
    ]));
    assert.strictEqual(noDiff.visibleFileCount, 0);
    assert.strictEqual(noDiff.tasksWithFiles, 0);
  });
});

describe("deriveWorkbenchReviewView", () => {
  it("separates pending review queue from review rejection history", () => {
    const tasks = [
      task({
        task_id: "pending",
        title: "Pending review task",
        execution_mode: "worktree_worker",
        needs_human_review: true,
        changed_files: ["apps/desktop/src/App.tsx"],
        suggested_action: "Review and merge.",
      }),
      task({
        task_id: "rejected",
        title: "Rejected review task",
        execution_mode: "worktree_worker",
        status: "failed",
        needs_human_review: false,
        failure_kind: "review_rejection",
        failure_message: "Scope too broad",
        changed_files: ["apps/desktop/src-tauri/src/executor/permissions.rs"],
        suggested_action: "Do not merge.",
      }),
      task({
        task_id: "read-only",
        title: "Read-only summary",
        execution_mode: "read_only",
        needs_human_review: true,
      }),
    ];

    const result = deriveWorkbenchReviewView(projection(tasks));

    assert.deepStrictEqual(result.queue.map((item) => item.taskId), ["pending"]);
    assert.strictEqual(result.queue[0].title, "Pending review task");
    assert.deepStrictEqual(result.queue[0].changedFiles, ["apps/desktop/src/App.tsx"]);
    assert.strictEqual(result.queue[0].suggestedAction, "Review and merge.");

    assert.deepStrictEqual(result.history.map((item) => item.taskId), ["rejected"]);
    assert.strictEqual(result.history[0].label, "审阅拒绝");
    assert.strictEqual(result.history[0].detail, "Scope too broad");
    assert.deepStrictEqual(result.history[0].changedFiles, ["apps/desktop/src-tauri/src/executor/permissions.rs"]);
  });

  it("keeps approved review decisions in review history", () => {
    const approved = task({
      task_id: "approved",
      title: "Approved worker task",
      execution_mode: "worktree_worker",
      status: "completed",
      needs_human_review: false,
      latest_message: "Review approved: Looks good",
      changed_files: ["apps/desktop/src/components/settings/RecoveryPanel.tsx"],
    }) as AgentA2ATaskProjection & { review_decision: string; reviewed_at_ms: number };
    approved.review_decision = "approved";
    approved.reviewed_at_ms = 50;

    const result = deriveWorkbenchReviewView(projection([approved]));

    assert.deepStrictEqual(result.queue, []);
    assert.deepStrictEqual(result.history.map((item) => item.taskId), ["approved"]);
    assert.strictEqual(result.history[0].label, "审阅通过");
    assert.strictEqual(result.history[0].detail, "Review approved: Looks good");
  });
});
