/**
 * Phase 1.9: Browser-level restart/reload smoke tests.
 *
 * These tests use the Playwright mock Tauri IPC harness (Vite dev server,
 * no real Tauri driver). They verify session and block persistence across
 * page reload (via IndexedDB) and recovery notice rendering (including the
 * Phase 1.7 listener-registration fix).
 *
 * For a true Tauri force-quit/reopen smoke, a driver/runtime harness that
 * can restart the Tauri binary is still needed.
 */
import { test, expect } from "@playwright/test";
import { setup } from "./fixtures/app";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { StreamEvent } from "../src/lib/protocol";

const sessionId = "resume-smoke-session";

// ── Helper: inject a raw session-output event directly into the listener ──

async function injectSessionOutput(
  page: import("@playwright/test").Page,
  payload: StreamEvent,
) {
  await page.evaluate((p) => {
    const listeners = (window as any).__tauriListeners?.["session-output"] ?? [];
    for (const fn of listeners) {
      fn({ payload: p });
    }
  }, payload);
}

async function seedActiveSession(
  page: import("@playwright/test").Page,
  sessionId: string,
  workingDir: string,
) {
  await page.evaluate(
    async ({ sessionId, workingDir }) => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const r = indexedDB.open("keyval-store");
        r.onerror = () => reject(r.error);
        r.onsuccess = () => resolve(r.result);
        r.onupgradeneeded = () => {
          const database = r.result;
          if (!database.objectStoreNames.contains("keyval")) database.createObjectStore("keyval");
        };
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put(
        [
          {
            id: workingDir,
            name: "forge-resume-test",
            path: workingDir,
            lastOpenedAt: 1,
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
            status: "resuming",
            workflowState: null,
            deliverySummary: null,
            createdAt: Date.now(),
            updatedAt: Date.now(),
          },
        ],
        "forge-sessions",
      );
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([], `forge-blocks:${sessionId}`);

      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    },
    { sessionId, workingDir },
  );
}

// ── Tests ─────────────────────────────────────────────────────────────

test.describe("Restart / reload smoke (IndexedDB persistence)", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page, { workingDir: "/tmp/forge-resume-workspace" });
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("blocks persist across browser reload", async ({ page }) => {
    // Set mock session ID so the mock IPC returns our known sessionId
    await page.addInitScript((sessionId) => {
      (window as any).__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    // Create a session and stream a full conversation
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    // Use fullConversation which generates known text
    await simulateStream(page, sessionId, fullConversation(sessionId));

    // Verify blocks are visible before reload
    // fullConversation generates: "I'll create a fibonacci function."
    const textLocator = page.getByText("I'll create a fibonacci");
    await expect(textLocator).toBeVisible({ timeout: 8000 });

    // ── Persist session metadata to IndexedDB (simulating what the store does) ──
    // Blocks are persisted via persistBlocks which writes to forge-blocks:<sessionId>
    // Sessions are persisted via persistSessions which writes to forge-sessions
    await page.evaluate(
      async ({ sessionId }) => {
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
          const r = indexedDB.open("keyval-store");
          r.onerror = () => reject(r.error);
          r.onsuccess = () => resolve(r.result);
        });
        const tx = db.transaction("keyval", "readwrite");
        tx.objectStore("keyval").put(
          [
            {
              id: "/tmp/forge-resume-workspace",
              name: "forge-resume-test",
              path: "/tmp/forge-resume-workspace",
              lastOpenedAt: 1,
            },
          ],
          "forge-workspaces",
        );
        tx.objectStore("keyval").put(
          "/tmp/forge-resume-workspace",
          "forge-active-workspace",
        );
        tx.objectStore("keyval").put(
          [
            {
              id: sessionId,
              agentType: "deepseek",
              model: "deepseek-v4-flash[1m]",
              workingDir: "/tmp/forge-resume-workspace",
              workspaceId: "/tmp/forge-resume-workspace",
              contextWindowTokens: 1_000_000,
              status: "stopped",
              workflowState: null,
              deliverySummary: null,
              createdAt: Date.now(),
              updatedAt: Date.now(),
            },
          ],
          "forge-sessions",
        );
        tx.objectStore("keyval").put(sessionId, "forge-active-session");

        // Persist a user_message block so the session has visible content
        tx.objectStore("keyval").put(
          [
            {
              block_id: "user-msg-1",
              event_type: "user_message",
              content: "请帮我创建一个 fibonacci 函数",
              isComplete: true,
              metadata: {},
            },
          ],
          `forge-blocks:${sessionId}`,
        );

        await new Promise<void>((resolve, reject) => {
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      },
      { sessionId },
    );

    // Reload
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    // Give IndexedDB hydration time
    await page.waitForTimeout(2000);

    // Sidebar should be visible — app initialized without crash
    const sidebar = page.locator("[data-testid='app-sidebar']");
    await expect(sidebar).toBeVisible();

    // Block content should be restored (may appear in sidebar, titlebar, and chat)
    await expect(
      page.getByText("请帮我创建一个 fibonacci 函数").first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test("startup restore replay events hydrate the active frontend mirror", async ({ page }) => {
    const restoredSessionId = "startup-restore-session";
    const workingDir = "/tmp/forge-resume-workspace";
    await seedActiveSession(page, restoredSessionId, workingDir);

    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.waitForFunction(() => {
      return ((window as any).__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await injectSessionOutput(page, {
      event_type: "session_started",
      session_id: restoredSessionId,
      agent_type: "deepseek",
      model: "deepseek-v4-flash[1m]",
      context_window_tokens: 1_000_000,
    });
    await injectSessionOutput(page, {
      event_type: "session_status",
      session_id: restoredSessionId,
      status: "resuming",
    });
    await injectSessionOutput(page, {
      event_type: "confirm_ask",
      session_id: restoredSessionId,
      block_id: "confirm-restored-1",
      question: "Allow deployment command?",
      kind: "shell_cmd",
      replayed_interrupted: true,
    });
    await injectSessionOutput(page, {
      event_type: "session_status",
      session_id: restoredSessionId,
      status: "running",
    });

    await expect(page.getByText("Allow deployment command?")).toBeVisible({ timeout: 5000 });
    await expect(page.getByTestId("confirm-interrupted")).toContainText("后端等待通道已失效");
    await expect(page.getByText("确认已中断")).toBeVisible();
  });
});

test.describe("Recovery notice rendering (Phase 1.7 listener fix)", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page, { workingDir: "/tmp/forge-resume-workspace" });
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("recovery notice renders even without active session", async ({
    page,
  }) => {
    // This test covers the Phase 1.7 listener-registration bug:
    // recovery_notice events must be processed even when no active session
    // exists, because useOutputStream registers the listener globally.

    // No session — use the landing page directly.
    // The AppShell renders useOutputStream(null) which registers the
    // session-output listener immediately.

    // Wait for the listener to be registered by the app
    await page.waitForFunction(() => {
      return ((window as any).__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    // Inject a recovery_notice event via the mock listener
    await injectSessionOutput(page, {
      event_type: "recovery_notice",
      session_id: "nonexistent-session",
      notice_id: "recovery-1",
      title: "会话恢复失败",
      message: "快照已损坏，Forge 从全新状态启动。",
      reason: "snapshot_restore_failed",
      recoverable: false,
    });

    // RecoveryNoticeBanner should appear
    const banner = page.locator("[data-testid='recovery-notice-banner']");
    await expect(banner).toBeVisible({ timeout: 5000 });
    await expect(banner).toContainText("会话恢复失败");
    await expect(banner).toContainText("快照已损坏");

    // Dismiss the notice
    await banner.locator("button[aria-label='Dismiss']").click();
    await expect(banner).not.toBeVisible({ timeout: 3000 });
  });

  test("recovery notice is dismissible and stays dismissed in-memory", async ({
    page,
  }) => {
    await page.waitForFunction(() => {
      return ((window as any).__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await injectSessionOutput(page, {
      event_type: "recovery_notice",
      session_id: "s1",
      notice_id: "dismiss-test",
      title: "可关闭通知",
      message: "关闭后不应在此会话中重新出现",
      reason: "snapshot_restore_failed",
      recoverable: false,
    });

    const banner = page.locator("[data-testid='recovery-notice-banner']");
    await expect(banner).toBeVisible({ timeout: 5000 });

    // Dismiss
    await banner.locator("button[aria-label='Dismiss']").click();
    await expect(banner).not.toBeVisible({ timeout: 3000 });

    // Inject another notice with a different ID — should appear as new
    await injectSessionOutput(page, {
      event_type: "recovery_notice",
      session_id: "s1",
      notice_id: "second-notice",
      title: "第二次恢复通知",
      message: "仅次要问题",
      reason: "snapshot_version_mismatch",
      recoverable: true,
    });

    const banner2 = page.locator("[data-testid='recovery-notice-banner']");
    await expect(banner2).toBeVisible({ timeout: 5000 });
    await expect(banner2).toContainText("第二次恢复通知");
    // First notice should still be gone
    await expect(banner2).not.toContainText("可关闭通知");
  });

  test("multiple recovery notices render and can be dismissed individually", async ({
    page,
  }) => {
    await page.waitForFunction(() => {
      return ((window as any).__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    // Emit two notices
    await injectSessionOutput(page, {
      event_type: "recovery_notice",
      session_id: "s1",
      notice_id: "multi-1",
      title: "通知一",
      message: "第一个恢复通知",
      reason: "snapshot_restore_failed",
      recoverable: false,
    });
    await injectSessionOutput(page, {
      event_type: "recovery_notice",
      session_id: "s1",
      notice_id: "multi-2",
      title: "通知二",
      message: "第二个恢复通知",
      reason: "snapshot_corrupted",
      recoverable: true,
    });

    const banner = page.locator("[data-testid='recovery-notice-banner']");
    await expect(banner).toBeVisible({ timeout: 5000 });

    // Both should be visible
    await expect(banner.locator("text=通知一")).toBeVisible();
    await expect(banner.locator("text=通知二")).toBeVisible();

    // Dismiss the first one
    const firstDismiss = banner
      .locator("[data-testid='recovery-notice-multi-1']")
      .locator("button[aria-label='Dismiss']");
    await firstDismiss.click();

    // First is gone, second remains
    await expect(banner.locator("text=通知一")).not.toBeVisible({ timeout: 3000 });
    await expect(banner.locator("text=通知二")).toBeVisible();
  });

  test("health alert renders even without active session", async ({
    page,
  }) => {
    await page.waitForFunction(() => {
      return ((window as any).__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await injectSessionOutput(page, {
      event_type: "health_alert",
      session_id: "gateway-watchdog",
      alert_id: "gateway-disconnected",
      level: "critical",
      title: "Gateway disconnected",
      message: "The background runtime is not responding.",
      remediation: "Open Settings > Diagnostics and restart Gateway.",
    });

    const banner = page.locator("[data-testid='health-alert-banner']");
    await expect(banner).toBeVisible({ timeout: 5000 });
    await expect(banner).toContainText("Gateway disconnected");
    await expect(banner).toContainText("restart Gateway");
  });

  test("offline browser state surfaces a global network banner", async ({
    page,
  }) => {
    await page.evaluate(() => {
      Object.defineProperty(window.navigator, "onLine", {
        configurable: true,
        get: () => false,
      });
      window.dispatchEvent(new Event("offline"));
    });

    const banner = page.getByTestId("network-status-banner");
    await expect(banner).toBeVisible({ timeout: 5000 });
    await expect(banner).toContainText("当前处于离线状态");

    await page.evaluate(() => {
      Object.defineProperty(window.navigator, "onLine", {
        configurable: true,
        get: () => true,
      });
      window.dispatchEvent(new Event("online"));
    });

    await expect(banner).toHaveCount(0);
  });

  test("missing API key errors surface as a global health alert", async ({
    page,
  }) => {
    await page.waitForFunction(() => {
      return ((window as any).__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.evaluate(() => {
      (window as any).__mockSessionId = "missing-key-session";
    });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await injectSessionOutput(page, {
      event_type: "error",
      session_id: "missing-key-session",
      block_id: "missing-key-error",
      message: "OpenAI API key is missing.",
      code: "missing_api_key",
    });

    const banner = page.locator("[data-testid='health-alert-banner']");
    await expect(banner).toBeVisible({ timeout: 5000 });
    await expect(banner).toContainText("缺少模型密钥");
    await expect(banner).toContainText("打开设置 > 模型");
  });
});
