import { expect, test } from "@playwright/test";
import { setup } from "./fixtures/app";
import { simulateStream } from "./mock-ipc";

test("shows one safe live progress row directly below the user message", async ({ page }) => {
  const sessionId = "result-first-live-progress";
  await setup(page);
  await page.addInitScript((id) => {
    // @ts-expect-error Forge E2E fixture session override
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.locator("textarea").fill("整理 AppShell 的结构");
  await page.locator("textarea").press("Enter");

  await simulateStream(page, sessionId, [
    {
      event_type: "tool_call_start",
      session_id: sessionId,
      block_id: "read-app-shell",
      tool_name: "read_file",
      tool_input: { path: "/Users/demo/project/src/AppShell.tsx" },
    },
  ], 1);

  const turn = page.getByTestId("conversation-turn").last();
  const progress = turn.getByTestId("conversation-progress");
  await expect(progress).toHaveCount(1);
  await expect(progress).toHaveText("正在查看 AppShell.tsx");
  await expect(progress).toHaveAttribute("role", "status");

  const followsUser = await turn.evaluate((node) => {
    const user = node.querySelector("[data-testid='user-message']");
    const live = node.querySelector("[data-testid='conversation-progress']");
    return Boolean(user && live && (user.compareDocumentPosition(live) & Node.DOCUMENT_POSITION_FOLLOWING));
  });
  expect(followsUser).toBe(true);
});

test("keeps completed process evidence out of the primary reading path", async ({ page }) => {
  const sessionId = "result-first-completed-turn";
  await setup(page);
  await page.addInitScript((id) => {
    // @ts-expect-error Forge E2E fixture session override
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.locator("textarea").fill("完成这一轮并验证结果");
  await page.locator("textarea").press("Enter");

  await simulateStream(page, sessionId, [
    { event_type: "thinking_start", session_id: sessionId, block_id: "thinking" },
    { event_type: "thinking_chunk", session_id: sessionId, block_id: "thinking", content: "private reasoning" },
    { event_type: "thinking_end", session_id: sessionId, block_id: "thinking" },
    {
      event_type: "tool_call_start",
      session_id: sessionId,
      block_id: "read-file",
      tool_name: "read_file",
      tool_input: { path: "/Users/demo/project/src/AppShell.tsx" },
    },
    { event_type: "tool_call_end", session_id: sessionId, block_id: "read-file" },
    {
      event_type: "tool_call_result",
      session_id: sessionId,
      block_id: "read-file",
      result: "file content",
      is_error: false,
      duration_ms: 24,
    },
    { event_type: "shell_start", session_id: sessionId, block_id: "build", command: "npm run build" },
    { event_type: "shell_output", session_id: sessionId, block_id: "build", content: "built" },
    { event_type: "shell_end", session_id: sessionId, block_id: "build", exit_code: 0 },
    {
      event_type: "diff_view",
      session_id: sessionId,
      block_id: "diff",
      file_path: "/Users/demo/project/src/AppShell.tsx",
      old_content: "old",
      new_content: "new",
    },
    {
      event_type: "provider_usage",
      session_id: sessionId,
      block_id: "usage",
      provider_id: "deepseek",
      model: "deepseek-v4",
      input_tokens: 100,
      output_tokens: 50,
      estimated_cost_micros: 20,
      reason: "provider_reported",
    },
    {
      event_type: "delivery_summary",
      session_id: sessionId,
      block_id: "delivery",
      summary: {
        project_path: "/Users/demo/project",
        preview_label: "预览未运行",
        checkpoint_label: "检查点已就绪",
        next_action: "检查这版",
        verification_label: "构建通过",
        verification_status: "passed",
      },
    },
    { event_type: "text_start", session_id: sessionId, block_id: "answer" },
    { event_type: "text_chunk", session_id: sessionId, block_id: "answer", content: "已经完成并验证通过。" },
    { event_type: "text_end", session_id: sessionId, block_id: "answer" },
  ], 1);

  const turn = page.getByTestId("conversation-turn").last();
  await expect(turn.getByTestId("assistant-message")).toHaveCount(1);
  await expect(turn.getByTestId("assistant-message")).toContainText("已经完成并验证通过");
  await expect(turn.getByTestId("conversation-progress")).toHaveCount(0);
  await expect(turn.getByTestId("thinking-trigger")).toHaveCount(0);
  await expect(turn.getByTestId("tool-activity-group")).toHaveCount(0);
  await expect(turn.getByTestId("tool-card-trigger")).toHaveCount(0);
  await expect(turn.getByTestId("shell-card-trigger")).toHaveCount(0);
  await expect(turn.getByTestId("diff-card")).toHaveCount(0);
  await expect(turn.getByTestId("provider-usage-card")).toHaveCount(0);
  await expect(turn.getByTestId("delivery-summary-grid")).toHaveCount(0);
  await expect(turn.getByTestId("message-block")).toHaveCount(2);
});

test("shows confirmations only while backend authority says they are unresolved", async ({ page }) => {
  const sessionId = "result-first-confirmation";
  await setup(page);
  await page.addInitScript((id) => {
    // @ts-expect-error Forge E2E fixture session override
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.locator("textarea").fill("修改设置");
  await page.locator("textarea").press("Enter");

  await simulateStream(page, sessionId, [
    {
      event_type: "confirm_ask",
      session_id: sessionId,
      block_id: "confirm-settings",
      question: "允许修改设置？",
      kind: "write",
    },
  ], 1);

  const turn = page.getByTestId("conversation-turn").last();
  await expect(turn.getByText("允许修改设置？", { exact: true })).toBeVisible();

  await simulateStream(page, sessionId, [
    {
      event_type: "confirm_response",
      session_id: sessionId,
      block_id: "confirm-settings",
      question: "允许修改设置？",
      kind: "write",
      approved: true,
      responded_at_ms: Date.now(),
    },
  ], 1);

  await expect(turn.getByText("允许修改设置？", { exact: true })).toHaveCount(0);
});

test("promotes a terminal error when no answer can be produced", async ({ page }) => {
  const sessionId = "result-first-terminal-error";
  await setup(page);
  await page.addInitScript((id) => {
    // @ts-expect-error Forge E2E fixture session override
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.locator("textarea").fill("运行检查");
  await page.locator("textarea").press("Enter");

  await simulateStream(page, sessionId, [
    {
      event_type: "error",
      session_id: sessionId,
      block_id: "build-error",
      message: "构建没有完成，请检查配置。",
      code: "build_failed",
    },
  ], 1);

  const turn = page.getByTestId("conversation-turn").last();
  await expect(turn.getByTestId("error-card-body")).toContainText("构建没有完成");
  await expect(turn.getByTestId("assistant-message")).toHaveCount(0);
  await expect(turn.getByTestId("conversation-progress")).toHaveCount(0);
});
