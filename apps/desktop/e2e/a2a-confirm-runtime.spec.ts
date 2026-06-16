import { test, expect } from "@playwright/test";
import { setup, openProjectArchive } from "./fixtures/app";
import { simulateStream } from "./mock-ipc";
import type { StreamEvent } from "../src/lib/protocol";

const sessionId = "a2a-confirm-runtime-session";

function a2aEvents(sessionId: string): StreamEvent[] {
  return [
    {
      event_type: "session_started",
      session_id: sessionId,
      agent_type: "deepseek",
      model: "deepseek-v4-flash",
    },
    {
      event_type: "agent_a2a_updated",
      session_id: sessionId,
      state: {
        running_count: 1,
        completed_count: 0,
        failed_count: 0,
        interrupted_count: 0,
        tasks: [
          {
            task_id: "task-1",
            agent_id: "agent-1",
            role: "worker",
            execution_mode: "worktree_worker",
            status: "running",
            title: "Refactor auth module",
            messages: [
              {
                message_id: "msg-1",
                kind: "started",
                content: "Starting worktree worker",
                created_at_ms: Date.now(),
              },
            ],
            latest_message: "Starting worktree worker",
            failure_message: null,
            updated_at_ms: Date.now(),
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
          },
        ],
      },
    },
  ];
}

function confirmThenStopEvents(sessionId: string): StreamEvent[] {
  return [
    {
      event_type: "confirm_ask",
      session_id: sessionId,
      block_id: "confirm-interrupted-1",
      question: "Allow overwriting src/App.tsx?",
      kind: "write_file",
      boundary: {
        title: "写入文件确认",
        target_label: "src/App.tsx",
        workspace_name: "forge",
        workspace_path: "/tmp/forge-a2a-workspace",
        operation: "write_file",
        affected_files: ["src/App.tsx"],
        command: null,
        impact: "覆盖现有文件",
        risk: "high",
        recovery: "可通过 git 恢复",
        checkpoint_status: "ready",
        warning: null,
      },
    },
    {
      event_type: "session_stopped",
      session_id: sessionId,
      reason: "user_request",
    },
  ];
}

function reviewQueueEvents(sessionId: string): StreamEvent[] {
  const now = Date.now();
  return [
    {
      event_type: "session_started",
      session_id: sessionId,
      agent_type: "deepseek",
      model: "deepseek-v4-flash",
    },
    {
      event_type: "agent_a2a_updated",
      session_id: sessionId,
      state: {
        running_count: 0,
        completed_count: 1,
        failed_count: 1,
        interrupted_count: 0,
        tasks: [
          {
            task_id: "review-task-1",
            agent_id: "agent-review-1",
            role: "worker",
            execution_mode: "worktree_worker",
            status: "completed",
            title: "Implement settings recovery polish",
            messages: [
              {
                message_id: "review-msg-1",
                kind: "final_result",
                content: "Patch ready for controller review",
                created_at_ms: now,
              },
            ],
            latest_message: "Patch ready for controller review",
            failure_message: null,
            updated_at_ms: now,
            artifact_count: 1,
            latest_artifact_kind: "diff_summary",
            latest_artifact_title: "Settings recovery diff",
            needs_human_review: true,
            reason_codes: ["tests_passed", "diff_available"],
            tests_passed: true,
            diff_truncated: false,
            worktree_path: "/tmp/forge-review-task-1",
            cleaned_up: false,
            suggested_action: "Review and merge after checking settings recovery.",
            diff_available: true,
            changed_file_count: 2,
            changed_files: ["apps/desktop/src/components/settings/RecoveryPanel.tsx"],
            test_report_excerpt: "1 e2e passed",
          },
          {
            task_id: "review-task-2",
            agent_id: "agent-review-2",
            role: "reviewer",
            execution_mode: "worktree_worker",
            status: "failed",
            title: "Review rejected unsafe permission edit",
            messages: [
              {
                message_id: "review-msg-2",
                kind: "failed",
                content: "Rejected because permission edits were too broad",
                created_at_ms: now,
              },
            ],
            latest_message: "Rejected because permission edits were too broad",
            failure_message: "Permission surface changed outside requested scope",
            updated_at_ms: now,
            artifact_count: 1,
            latest_artifact_kind: "review_report",
            latest_artifact_title: "Permission review",
            needs_human_review: false,
            reason_codes: ["scope_too_broad"],
            tests_passed: false,
            diff_truncated: false,
            worktree_path: "/tmp/forge-review-task-2",
            cleaned_up: true,
            suggested_action: "Do not merge this worktree.",
            failure_kind: "review_rejection",
            retryable: false,
            diff_available: true,
            changed_file_count: 1,
            changed_files: ["apps/desktop/src-tauri/src/executor/permissions.rs"],
            test_report_excerpt: "review gate failed",
          },
        ],
      },
    },
  ];
}

test.beforeEach(async ({ page }) => {
  await setup(page, { workingDir: "/tmp/forge-a2a-workspace" });
  await page.goto("http://localhost:1420");
  await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
});

test.describe("A2A runtime surfaces", () => {
  test("chat shows lightweight inline summary and not the full timeline", async ({ page }) => {
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, a2aEvents(sessionId));

    const inlineSummary = page.locator(".forge-a2a-inline-summary");
    await expect(inlineSummary).toBeVisible();
    await expect(inlineSummary).toContainText("子任务");
    await expect(inlineSummary).toContainText("1 个子任务运行中");

    await expect(page.locator("[data-testid='agent-a2a-timeline']")).toHaveCount(0);
  });

  test("hub panel renders detailed A2A workspace with worktree review area", async ({ page }) => {
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, a2aEvents(sessionId));

    await openProjectArchive(page, "agents");

    const workspace = page.locator(".forge-a2a-workspace");
    await expect(workspace).toBeVisible();
    await expect(workspace).toContainText("Agent Workbench");
    await expect(workspace).toContainText("子任务");
    await expect(workspace).toContainText("1 运行");
  });

  test("hub panel summarizes review queue and review history", async ({ page }) => {
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, reviewQueueEvents(sessionId));
    await openProjectArchive(page, "agents");

    const workspace = page.locator(".forge-a2a-workspace");
    await expect(workspace).toContainText("审阅队列");
    await expect(workspace).toContainText("1 个待审阅");
    await expect(workspace).toContainText("Implement settings recovery polish");
    await expect(workspace).toContainText("apps/desktop/src/components/settings/RecoveryPanel.tsx");
    await expect(workspace).toContainText("审阅历史");
    await expect(workspace).toContainText("审阅拒绝");
    await expect(workspace).toContainText("Review rejected unsafe permission edit");
  });

  test("hub panel can approve a review queue item", async ({ page }) => {
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, reviewQueueEvents(sessionId));
    await openProjectArchive(page, "agents");

    const workspace = page.locator(".forge-a2a-workspace");
    await workspace.getByRole("button", { name: "通过审阅 Implement settings recovery polish" }).click();

    await expect(workspace).toContainText("审阅历史");
    await expect(workspace).toContainText("审阅通过");
    await expect(workspace).toContainText("Review approved");
    await expect(workspace).not.toContainText("1 个待审阅");
    await expect.poll(() => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastReviewAgentA2ATasksArgs;
    })).toMatchObject({
      sessionId,
      taskIds: ["review-task-1"],
      decision: "approve",
    });
  });

  test("global background status bar opens the agent task list", async ({ page }) => {
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      window.__mockScheduledTasks = [
        {
          id: "acceptance-schedule",
          title: "Daily acceptance check",
          text: "Run smoke checks",
          enabled: true,
          interval_seconds: 3600,
          next_run_at_ms: Date.now() + 3600_000,
          last_run_at_ms: null,
          created_at_ms: Date.now(),
          updated_at_ms: Date.now(),
          tags: [],
          profile_id: null,
          last_error: null,
        },
      ];
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, reviewQueueEvents(sessionId));

    const statusBar = page.getByTestId("background-task-status");
    await expect(statusBar).toBeVisible();
    await expect(statusBar).toContainText("1 待审阅");
    await expect(statusBar).toContainText("1 调度任务");

    await statusBar.getByRole("button", { name: "展开后台任务列表" }).click();
    const notificationList = page.getByTestId("background-notification-list");
    await expect(notificationList).toBeVisible();
    await expect(notificationList).toContainText("通知");
    await expect(notificationList).toContainText("需要审阅");
    await expect(notificationList).toContainText("调度已启用");

    const taskList = page.getByTestId("background-task-list");
    await expect(taskList).toBeVisible();
    await expect(taskList).toContainText("Implement settings recovery polish");
    await expect(taskList).toContainText("Daily acceptance check");

    await statusBar.getByRole("button", { name: "打开后台任务面板" }).click();
    await expect(page.locator(".forge-a2a-workspace")).toBeVisible();
    await expect(page.locator(".forge-a2a-workspace")).toContainText("审阅队列");
  });
});

test.describe("confirm_interrupted rendering", () => {
  test("interrupted confirm card hides action buttons and shows explicit notice", async ({ page }) => {
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const events: StreamEvent[] = [
      ...a2aEvents(sessionId),
      ...confirmThenStopEvents(sessionId),
    ];
    await simulateStream(page, sessionId, events);

    const card = page.locator(".forge-confirm-card[data-confirm-state='interrupted']").first();
    await expect(card).toBeVisible();

    await expect(card.locator("[data-testid='confirm-action-bar']")).toHaveCount(0);
    await expect(card.locator("[data-testid='confirm-approve']")).toHaveCount(0);
    await expect(card.locator("[data-testid='confirm-cancel']")).toHaveCount(0);

    const notice = card.locator("[data-testid='confirm-interrupted']");
    await expect(notice).toBeVisible();
    await expect(notice).toContainText("会话已经停止");

    const status = card.locator(".forge-confirm-interrupted");
    await expect(status).toBeVisible();
    await expect(status).toContainText("确认已中断");
  });
});
