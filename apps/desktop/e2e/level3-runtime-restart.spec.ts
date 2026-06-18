/**
 * Level 3 runtime restart smoke.
 *
 * The current desktop e2e contract is Playwright + Vite with mocked Tauri IPC,
 * not tauri-driver/WebDriver. This spec therefore closes the app page and
 * reopens a fresh page through the shared mocked IPC harness. It deliberately
 * is not the final proof for a real Tauri binary force-quit/reopen sequence.
 */
import { test, expect, type Page } from "@playwright/test";
import {
  expectNoSendInput,
  persistMockRuntimeReplayEvents,
  quitApp,
  reopenApp,
  setup,
} from "./fixtures/app";
import { simulateStream } from "./mock-ipc";
import type { StreamEvent } from "../src/lib/protocol";

const APP_URL = "http://localhost:1420";
const SESSION_ID = "level3-runtime-restart-session";
const LOOP_TASK_ID = "loop-level3-restart";
const WORKING_DIR = "/tmp/forge-level3-runtime-restart";
const USER_PROMPT = "Level 3 restart durable runtime ownership";

test.describe("Level 3 mocked runtime restart harness", () => {
  test("restores durable loop, session, A2A, and gateway facts after app close and reopen", async ({ page }) => {
    await setup(page, { workingDir: WORKING_DIR });
    await page.addInitScript((sessionId) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = sessionId;
    }, SESSION_ID);
    await page.goto(APP_URL);
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const now = Date.now();
    const replayEvents = runtimeReplayEvents(now);
    await seedDurableRuntimeState(page, now);
    await persistMockRuntimeReplayEvents(page, replayEvents);
    await simulateStream(page, SESSION_ID, replayEvents, 1);
    await expect(page.getByTestId("background-task-status")).toContainText("1 Loop 任务");

    const context = await quitApp(page);
    const reopened = await reopenApp(context, { workingDir: WORKING_DIR });

    await expect(reopened.getByText(USER_PROMPT).first()).toBeVisible({ timeout: 5000 });

    await reopened.getByRole("button", { name: "历史", exact: true }).click();
    const historyDialog = reopened.getByRole("dialog");
    await expect(historyDialog.getByRole("heading", { name: "历史" })).toBeVisible();
    await expect(historyDialog).toContainText("Level 3 restart durable session snapshot");
    await expect(historyDialog.getByRole("button", { name: `恢复 ${SESSION_ID}` })).toBeVisible();
    await historyDialog.getByRole("button", { name: "关闭" }).click();

    const status = reopened.getByTestId("background-task-status");
    await expect(status).toContainText("1 Loop 任务");
    await status.getByRole("button", { name: "展开后台任务列表" }).click();
    const drawer = reopened.getByTestId("background-task-list");
    await expect(drawer).toContainText("Resume Level 3 runtime ownership after restart");
    await expect(drawer).toContainText("等待输入");
    await expect(drawer).toContainText("成本未知");
    const loopPanel = drawer.getByTestId(`loop-task-panel-${LOOP_TASK_ID}`);
    await expect(loopPanel).toHaveAttribute("data-tone", "waiting");
    await expect(loopPanel).toContainText("evt-runner-waiting-for-input");

    await reopened.getByRole("button", { name: "打开后台任务面板" }).click();
    const workbench = reopened.getByRole("region", { name: "子任务" });
    await expect(workbench).toContainText("Restart runtime worker");
    await expect(workbench).toContainText("1 保留工作树");
    await expect(workbench).toContainText("/tmp/forge-level3-retained-worktree");
    await expect(workbench.locator(".forge-a2a-runtime-facts", { hasText: "文件 IO" })).toContainText("runner.rs");
    await expect(workbench.locator(".forge-a2a-runtime-facts", { hasText: "用量" })).toContainText("input 4200 / output unknown / cost unknown");

    await reopened.getByRole("button", { name: "设置" }).click();
    const settingsDialog = reopened.getByRole("dialog");
    await settingsDialog.getByRole("button", { name: "诊断" }).click();
    await expect(settingsDialog).toContainText("后台运行时 · 有积压");
    await expect(settingsDialog).toContainText("1 pending · 1 inputs · 0 claimed · 0 dead-letter");
    await expect(settingsDialog).toContainText("1 active");
    await expect(settingsDialog).toContainText("gateway loop runner");

    await expectNoSendInput(reopened);
    await expect(reopened.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastResumedSessionId;
    })).resolves.toBeUndefined();
    await expect(reopened.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastCreateSessionArgs;
    })).resolves.toBeUndefined();
  });
});

function runtimeReplayEvents(now: number): StreamEvent[] {
  return [
    {
      event_type: "agent_a2a_updated",
      session_id: SESSION_ID,
      state: {
        running_count: 0,
        completed_count: 2,
        failed_count: 0,
        interrupted_count: 0,
        tasks: [
          {
            task_id: "a2a-restart-worker",
            agent_id: "agent-restart-worker",
            role: "implementer",
            execution_mode: "worktree_worker",
            status: "completed",
            title: "Restart runtime worker",
            messages: [
              {
                message_id: "msg-restart-worker-1",
                kind: "progress",
                content: "Retained worker kept worktree for human review.",
                created_at_ms: now - 60_000,
              },
            ],
            latest_message: "Retained worker kept worktree for human review.",
            failure_message: null,
            updated_at_ms: now,
            artifact_count: 1,
            latest_artifact_kind: "patch_proposal",
            latest_artifact_title: "Runtime restart patch",
            needs_human_review: true,
            reason_codes: ["human_gated_commit"],
            tests_passed: true,
            diff_truncated: false,
            worktree_path: "/tmp/forge-level3-retained-worktree",
            cleaned_up: false,
            suggested_action: "Review retained worktree before merge; no autonomous commit is allowed.",
            parent_task_id: null,
            child_task_ids: ["a2a-restart-usage"],
            created_at_ms: now - 120_000,
            started_at_ms: now - 118_000,
            ended_at_ms: now - 1_000,
            duration_ms: 117_000,
            retryable: null,
            failure_kind: null,
            resume_note: null,
            latest_progress: null,
            lease_owner: null,
            lease_acquired_at_ms: null,
            lease_expires_at_ms: null,
            last_heartbeat_at_ms: null,
            attempt_count: 1,
            max_attempts: 3,
            diff_available: true,
            changed_file_count: 1,
            changed_files: ["apps/desktop/src-tauri/src/loop_runtime/runner.rs"],
            test_report_excerpt: "restart smoke retained durable runtime facts",
          },
          {
            task_id: "a2a-restart-usage",
            agent_id: "agent-restart-usage",
            role: "reviewer",
            execution_mode: "worktree_worker",
            status: "completed",
            title: "Restart usage auditor",
            messages: [],
            latest_message: "Usage facts retained unknown output and cost.",
            failure_message: null,
            updated_at_ms: now,
            artifact_count: 0,
            latest_artifact_kind: null,
            latest_artifact_title: null,
            needs_human_review: false,
            reason_codes: [],
            tests_passed: null,
            diff_truncated: null,
            worktree_path: null,
            cleaned_up: null,
            suggested_action: null,
            parent_task_id: "a2a-restart-worker",
            child_task_ids: [],
            created_at_ms: now - 90_000,
            started_at_ms: now - 88_000,
            ended_at_ms: now - 1_000,
            duration_ms: 87_000,
            retryable: null,
            failure_kind: null,
            resume_note: null,
            latest_progress: null,
            lease_owner: null,
            lease_acquired_at_ms: null,
            lease_expires_at_ms: null,
            last_heartbeat_at_ms: null,
            attempt_count: 1,
            max_attempts: 3,
            diff_available: null,
            changed_file_count: null,
            changed_files: [],
            test_report_excerpt: null,
          },
        ],
      },
    },
    {
      event_type: "subagent_runtime_event",
      session_id: SESSION_ID,
      loop_task_id: LOOP_TASK_ID,
      task_id: "a2a-restart-worker",
      event: {
        type: "file_io",
        operation: "diff_observed",
        path: "apps/desktop/src-tauri/src/loop_runtime/runner.rs",
      },
    },
    {
      event_type: "subagent_runtime_event",
      session_id: SESSION_ID,
      loop_task_id: LOOP_TASK_ID,
      task_id: "a2a-restart-usage",
      event: {
        type: "usage_recorded",
        model: "claude-sonnet",
        input_tokens: 4200,
        output_tokens: null,
        estimated_cost_micros: null,
      },
    },
    {
      event_type: "loop_runtime_updated",
      session_id: SESSION_ID,
      loop_task_id: LOOP_TASK_ID,
      task: {
        id: LOOP_TASK_ID,
        goal: "Resume Level 3 runtime ownership after restart",
        session_id: SESSION_ID,
        profile_id: null,
        workspace_path: WORKING_DIR,
        status: "waiting_for_input",
        owner: { kind: "gateway" },
        policy: {},
        budget: {},
        completion_contract: {},
        created_at_ms: now - 180_000,
        updated_at_ms: now,
        lease: null,
        open_gates: [],
        evidence: [
          {
            kind: "runner_lease_history",
            runner_id: "gateway-loop-runner",
            lease_id: "lease-before-restart",
            owner_pid: 4321,
            acquired_at_ms: now - 120_000,
            heartbeat_at_ms: now - 119_000,
            expires_at_ms: now + 180_000,
            events: ["task_started", "task_waiting_for_input"],
          },
        ],
        policy_decisions: [],
        latest_budget_snapshot: {
          budget_exceeded: true,
          model_rounds_used: 7,
          tool_calls_used: 21,
          elapsed_ms: 145_000,
          has_unknown_cost: true,
        },
        latest_event_id: "evt-runner-waiting-for-input",
        outcome: {
          message: "Gateway loop runner is waiting for existing desktop session level3-runtime-restart-session to accept the next step; autonomous agent resume is disabled.",
        },
        completion_result: {
          status: "blocked",
          reasons: [],
        },
      },
    },
  ];
}

async function seedDurableRuntimeState(page: Page, now: number) {
  await page.evaluate(
    async ({ sessionId, workingDir, now, userPrompt }) => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains("keyval")) database.createObjectStore("keyval");
        };
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put(
        [
          {
            id: workingDir,
            name: "forge-level3-runtime-restart",
            path: workingDir,
            lastOpenedAt: now,
          },
        ],
        "forge-workspaces",
      );
      tx.objectStore("keyval").put(workingDir, "forge-active-workspace");
      tx.objectStore("keyval").put(
        [
          {
            id: sessionId,
            agentType: "deepseek",
            model: "deepseek-v4-flash[1m]",
            workingDir,
            workspaceId: workingDir,
            contextWindowTokens: 1_000_000,
            status: "stopped",
            workflowState: null,
            deliverySummary: null,
            createdAt: now - 180_000,
            updatedAt: now,
          },
        ],
        "forge-sessions",
      );
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put(
        [
          {
            block_id: "level3-restart-user-message",
            event_type: "user_message",
            content: userPrompt,
            isComplete: true,
            metadata: {},
          },
        ],
        `forge-blocks:${sessionId}`,
      );
      tx.objectStore("keyval").put(
        [
          {
            session_id: sessionId,
            provider: "deepseek",
            model: "deepseek-v4-flash[1m]",
            working_dir: workingDir,
            summary: "Level 3 restart durable session snapshot",
            created_at_ms: now - 180_000,
            updated_at_ms: now,
            message_count: 4,
          },
        ],
        "forge-session-store-search-results",
      );
      tx.objectStore("keyval").put(
        {
          ok: true,
          message: "Gateway runtime is healthy after restart.",
          uptime_seconds: 240,
          active_sessions: 1,
          pending_triggers: 1,
          pending_session_inputs: 1,
          claimed_triggers: 0,
          dead_letter_runs: 0,
          recent_runs: [],
          recent_session_inputs: [],
          runtime_tasks: [
            {
              name: "gateway_loop_runner",
              running: true,
              last_started_at_ms: now - 60_000,
              last_error: null,
            },
          ],
        },
        "forge-gateway-runtime-status",
      );

      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    },
    { sessionId: SESSION_ID, workingDir: WORKING_DIR, now, userPrompt: USER_PROMPT },
  );
}
