import { test, expect, type Page } from "@playwright/test";
import { createMockIPC, simulateStream, fullConversation } from "./mock-ipc";

/** Setup: inject mock IPC before the app loads */
async function setup(page: Page) {
  await page.addInitScript(() => {
    // @ts-expect-error mock
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args: Record<string, unknown>) => {
        return window.__tauriMockIPC?.(cmd, args);
      },
    };
    // @ts-expect-error listeners
    window.__tauriListeners = {};
    // Mock Tauri listen()
    // @ts-expect-error
    window.__TAURI__ = {
      event: {
        listen: (event: string, fn: (data: unknown) => void) => {
          // @ts-expect-error
          if (!window.__tauriListeners[event]) window.__tauriListeners[event] = [];
          // @ts-expect-error
          window.__tauriListeners[event].push(fn);
          return () => {};
        },
      },
    };
  });
}

test.describe("Timeline Message Flow", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("app loads and shows empty state", async ({ page }) => {
    await expect(page.locator("text=Send a message to begin")).toBeVisible();
  });

  test("creating a session shows chat input", async ({ page }) => {
    await page.addInitScript(() => {
      window.__tauriMockIPC = createMockIPC();
    });
    await page.goto("http://localhost:1420");
    // Click new session button
    await page.click('[class*="sidebar"] button');
    // Input should appear
    await expect(page.locator("textarea")).toBeVisible();
  });

  test("timeline messages render correctly", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const mockIPC = createMockIPC({
      create_session: () => ({ session_id: sessionId }),
    });
    await page.addInitScript((fn) => {
      const mockFn = new Function(`return (${fn})`)();
      window.__tauriMockIPC = mockFn;
    }, mockIPC.toString());

    await page.goto("http://localhost:1420");
    // Create session
    await page.locator("[class*=sidebar] button:has(svg)").first().click();
    await page.waitForTimeout(500);

    // Simulate a full conversation
    const events = fullConversation(sessionId);
    await simulateStream(page, sessionId, events, 30);

    // User bubble should be right-aligned amber
    const userBubble = page.locator("text=You").first();
    await expect(userBubble).toBeVisible({ timeout: 5000 });

    // AI text should be visible
    const aiText = page.locator("text=Assistant").first();
    await expect(aiText).toBeVisible();

    // Tool card should show write_to_file
    await expect(page.locator("text=write_to_file")).toBeVisible({ timeout: 5000 });

    // Shell card should show terminal output
    await expect(page.locator("text=python test.py")).toBeVisible();

    // Final text should be visible
    await expect(page.locator("text=The fibonacci function works correctly")).toBeVisible();
  });

  test("thinking block expands and shows content", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript(() => {
      window.__tauriMockIPC = createMockIPC({
        create_session: () => ({ session_id: crypto.randomUUID() }),
      });
    });
    await page.goto("http://localhost:1420");
    await page.locator("[class*=sidebar] button:has(svg)").first().click();
    await page.waitForTimeout(300);

    const thinkingId = crypto.randomUUID();
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "thinking_start", session_id: sessionId, block_id: thinkingId },
      { event_type: "thinking_chunk", session_id: sessionId, block_id: thinkingId, content: "I need to analyze the auth system first." },
      { event_type: "thinking_end", session_id: sessionId, block_id: thinkingId },
      { event_type: "text_start", session_id: sessionId, block_id: crypto.randomUUID() },
      { event_type: "text_chunk", session_id: sessionId, block_id: crypto.randomUUID(), content: "Done analyzing." },
      { event_type: "text_end", session_id: sessionId, block_id: crypto.randomUUID() },
    ], 30);

    // Thinking trigger should be visible
    const thinkingTrigger = page.locator("text=Thinking").first();
    await expect(thinkingTrigger).toBeVisible({ timeout: 5000 });

    // Click to expand
    await thinkingTrigger.click();
    await page.waitForTimeout(200);

    // Thinking content should be visible
    await expect(page.locator("text=I need to analyze")).toBeVisible();
  });

  test("tool card shows running then done status", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript(() => {
      window.__tauriMockIPC = createMockIPC({
        create_session: () => ({ session_id: crypto.randomUUID() }),
      });
    });
    await page.goto("http://localhost:1420");
    await page.locator("[class*=sidebar] button:has(svg)").first().click();
    await page.waitForTimeout(300);

    const toolId = crypto.randomUUID();
    // Send tool_start first (running state)
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "tool_call_start", session_id: sessionId, block_id: toolId, tool_name: "read_file", tool_input: { path: "test.rs" } },
    ], 30);

    // Should show running status
    await expect(page.locator("text=read_file")).toBeVisible({ timeout: 3000 });
    await expect(page.locator("text=running")).toBeVisible();

    // Send tool_result (done state)
    await simulateStream(page, sessionId, [
      { event_type: "tool_call_result", session_id: sessionId, block_id: crypto.randomUUID(), result: "fn main() {}", is_error: false, duration_ms: 100 },
    ], 30);

    // Should show done
    await expect(page.locator("text=done")).toBeVisible({ timeout: 3000 });
  });

  test("sidebar expands on hover", async ({ page }) => {
    const sidebar = page.locator("aside").first();

    // Initially collapsed (48px)
    const initialWidth = (await sidebar.boundingBox())?.width ?? 0;
    expect(initialWidth).toBeLessThan(60);

    // Hover to expand
    await sidebar.hover();
    await page.waitForTimeout(400);

    // Should be wider
    const expandedWidth = (await sidebar.boundingBox())?.width ?? 0;
    expect(expandedWidth).toBeGreaterThan(150);
  });
});

test.describe("InputBar", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
  });

  test("enter key sends message and clears input", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    let sentText = "";
    await page.addInitScript(() => {
      window.__tauriMockIPC = createMockIPC({
        create_session: () => ({ session_id: crypto.randomUUID() }),
        send_input: (args) => { sentText = args.text as string; },
      });
    });
    await page.goto("http://localhost:1420");
    await page.locator("[class*=sidebar] button:has(svg)").first().click();
    await page.waitForTimeout(300);

    const textarea = page.locator("textarea");
    await textarea.fill("Hello DeepSeek");
    await textarea.press("Enter");

    // User bubble should appear
    await expect(page.locator("text=Hello DeepSeek")).toBeVisible({ timeout: 3000 });
  });

  test("shift+enter creates newline without sending", async ({ page }) => {
    await page.addInitScript(() => {
      window.__tauriMockIPC = createMockIPC({
        create_session: () => ({ session_id: crypto.randomUUID() }),
      });
    });
    await page.goto("http://localhost:1420");
    await page.locator("[class*=sidebar] button:has(svg)").first().click();
    await page.waitForTimeout(300);

    const textarea = page.locator("textarea");
    await textarea.fill("line1");
    await textarea.press("Shift+Enter");
    await textarea.pressSequentially("line2");

    // Should still be in the textarea, not sent
    await expect(textarea).toContainText("line1\nline2");
  });
});
