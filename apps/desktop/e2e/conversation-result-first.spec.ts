import { expect, test } from "@playwright/test";
import { setup } from "./fixtures/app";
import { simulateStream } from "./mock-ipc";
import type { AgentTurnProjection } from "../src/lib/protocol";

function turnProjection(
  sessionId: string,
  status: AgentTurnProjection["status"] = "completed",
  overrides: Partial<AgentTurnProjection> = {},
): AgentTurnProjection {
  return {
    session_id: sessionId,
    status,
    step_label: status,
    workspace_path: "/repo/private",
    compact_count: 0,
    verification_status: status === "failed" ? "failed" : "passed",
    model_rounds: 1,
    tool_call_count: 0,
    failed_tool_count: status === "failed" ? 1 : 0,
    compact_saved_tokens: 0,
    ...overrides,
  };
}

async function waitForCollapsibleOpen(
  trigger: import("@playwright/test").Locator,
  panel: import("@playwright/test").Locator,
) {
  await expect(trigger).toHaveAttribute("aria-expanded", "true");
  await expect(panel).toHaveAttribute("data-open", "");
  await expect(panel).toBeVisible();
  await expect.poll(async () => panel.evaluate((element) => !element
    .getAnimations({ subtree: true })
    .some((animation) => animation.playState === "pending" || animation.playState === "running"))).toBe(true);
}

async function waitForPointerTarget(target: import("@playwright/test").Locator) {
  await expect(target).toBeVisible();
  await target.scrollIntoViewIfNeeded();
  await expect.poll(async () => target.evaluate((element) => {
    const rect = element.getBoundingClientRect();
    const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
    return getComputedStyle(element).pointerEvents !== "none"
      && Boolean(hit && (hit === element || element.contains(hit)));
  })).toBe(true);
}

async function openProcessDisclosure(disclosure: import("@playwright/test").Locator) {
  const trigger = disclosure.getByTestId("conversation-process-trigger");
  await expect(trigger).toBeVisible();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await waitForPointerTarget(trigger);
    await trigger.click();
  }
  await waitForCollapsibleOpen(
    trigger,
    disclosure.locator(":scope > [data-slot='collapsible-content']"),
  );
  return disclosure;
}

async function revealProcessDetails(disclosure: import("@playwright/test").Locator) {
  await openProcessDisclosure(disclosure);
  const evidenceTrigger = disclosure.getByTestId("conversation-evidence-trigger");
  if (await evidenceTrigger.count()) {
    await waitForPointerTarget(evidenceTrigger);
    await evidenceTrigger.click();
    const evidenceRoot = evidenceTrigger.locator("..");
    await waitForCollapsibleOpen(
      evidenceTrigger,
      evidenceRoot.locator(":scope > [data-slot='collapsible-content']"),
    );
  }
  const processItems = disclosure.getByTestId("conversation-process-item");
  for (let index = 0, count = await processItems.count(); index < count; index += 1) {
    const processItem = processItems.nth(index);
    const stageRoot = processItem.locator(":scope > [data-slot='collapsible']");
    const stageTrigger = stageRoot.locator(":scope > [data-slot='collapsible-trigger']");
    if (!await stageTrigger.count()) continue;
    await processItem.hover();
    await waitForPointerTarget(stageTrigger);
    await stageTrigger.click();
    await waitForCollapsibleOpen(
      stageTrigger,
      stageRoot.locator(":scope > [data-slot='collapsible-content']"),
    );
  }
  return disclosure;
}

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
  await expect(progress).toHaveText("正在查找相关内容");
  await expect(progress).toHaveAttribute("data-progress-id", "discovering");
  await expect(progress).toHaveAttribute("role", "status");
  await expect(progress).not.toContainText("AppShell.tsx");

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
    {
      event_type: "agent_turn_updated",
      session_id: sessionId,
      state: turnProjection(sessionId, "completed", { tool_call_count: 2 }),
    },
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
  await expect(turn).not.toContainText("private reasoning");

  const disclosure = turn.getByTestId("conversation-process-disclosure");
  const trigger = disclosure.getByTestId("conversation-process-trigger");
  await expect(trigger).toHaveAccessibleName(
    /^已完成 · (?:<1 秒|\d+ 秒) · 3 项操作，查看运行过程$/,
  );
  await expect(trigger).toHaveAttribute("aria-expanded", "false");
  await expect(disclosure.getByTestId("conversation-process-timeline")).toHaveCount(0);
  await expect(disclosure.getByTestId("conversation-process-item")).toHaveCount(0);
  await expect(disclosure.getByTestId("conversation-next-action")).toHaveCount(0);
  await expect(turn.getByRole("button", { name: /工作面板|打开.*文件/ })).toHaveCount(0);

  await openProcessDisclosure(disclosure);
  await expect(trigger).toHaveAccessibleName(
    /^已完成 · (?:<1 秒|\d+ 秒) · 3 项操作，收起运行过程$/,
  );
  await expect(disclosure.getByTestId("conversation-process-item")).toHaveCount(2);
  await expect(disclosure.getByText("分析需求", { exact: true })).toBeVisible();
  await expect(disclosure.getByText("验证结果", { exact: true })).toBeVisible();
  await expect(disclosure).not.toContainText("private reasoning");
  await expect(disclosure.getByTestId("tool-card-trigger")).toHaveCount(0);
  await expect(disclosure.getByTestId("shell-card-trigger")).toHaveCount(0);
  await expect(disclosure.getByTestId("diff-card")).toHaveCount(0);

  await revealProcessDetails(disclosure);
  await expect(disclosure.getByTestId("tool-card-trigger")).toContainText("AppShell.tsx");
  await expect(disclosure.getByTestId("shell-card-trigger")).toContainText("npm run build");
  await expect(disclosure.getByTestId("diff-card")).toHaveCount(1);
  await expect(disclosure.getByTestId("provider-usage-card")).toContainText("deepseek-v4");
  await expect(disclosure.getByTestId("conversation-delivery-metadata")).toContainText(
    "构建通过 · 检查点已就绪",
  );
  await expect(disclosure).not.toContainText("private reasoning");
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
    { event_type: "text_start", session_id: sessionId, block_id: "confirmation-answer" },
    {
      event_type: "text_chunk",
      session_id: sessionId,
      block_id: "confirmation-answer",
      content: "设置已安全更新。",
    },
    { event_type: "text_end", session_id: sessionId, block_id: "confirmation-answer" },
    {
      event_type: "agent_turn_updated",
      session_id: sessionId,
      state: turnProjection(sessionId),
    },
  ], 1);

  await expect(turn.getByText("允许修改设置？", { exact: true })).toHaveCount(0);
  await expect(turn.getByTestId("assistant-message")).toContainText("设置已安全更新");
  const disclosure = turn.getByTestId("conversation-process-disclosure");
  await expect(disclosure.getByTestId("conversation-process-trigger")).toHaveAccessibleName(
    /^已完成 · (?:<1 秒|\d+ 秒)，查看运行过程$/,
  );
  await openProcessDisclosure(disclosure);
  await expect(disclosure.getByText("确认已处理", { exact: true })).toBeVisible();
  await expect(disclosure).not.toContainText("允许修改设置？");
});

test("keeps a partial result and terminal error visible together", async ({ page }) => {
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
    { event_type: "text_start", session_id: sessionId, block_id: "partial-result" },
    {
      event_type: "text_chunk",
      session_id: sessionId,
      block_id: "partial-result",
      content: "已经完成配置检查，但构建尚未完成。",
    },
    { event_type: "text_end", session_id: sessionId, block_id: "partial-result" },
    {
      event_type: "error",
      session_id: sessionId,
      block_id: "build-error",
      message: "构建没有完成，请检查配置。",
      code: "build_failed",
    },
    {
      event_type: "agent_turn_updated",
      session_id: sessionId,
      state: turnProjection(sessionId, "failed"),
    },
  ], 1);

  const turn = page.getByTestId("conversation-turn").last();
  await expect(turn.getByTestId("assistant-message")).toContainText("已经完成配置检查");
  await expect(turn.getByTestId("error-card-body")).toContainText("构建没有完成");
  await expect(turn.getByTestId("conversation-progress")).toHaveCount(0);
  const disclosure = turn.getByTestId("conversation-process-disclosure");
  await expect(disclosure.getByTestId("conversation-process-trigger")).toHaveAccessibleName(
    /^未完成 · (?:<1 秒|\d+ 秒)，查看运行过程$/,
  );
  await openProcessDisclosure(disclosure);
  await expect(disclosure.getByText("处理异常", { exact: true })).toBeVisible();
});
