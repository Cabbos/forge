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
