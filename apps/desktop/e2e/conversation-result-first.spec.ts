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
