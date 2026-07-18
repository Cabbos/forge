import { test, expect } from "@playwright/test";
import { setup, holdSendInput, expectHeldSendInput, releaseHeldSendInput, openWorkPanel, workPanel } from "./fixtures/app";
import { simulateStream, fullConversation } from "./mock-ipc";

async function forceDarkWorkbench(page: import("@playwright/test").Page) {
  await page.addInitScript(() => {
    const apply = () => {
      document.querySelectorAll<HTMLElement>("[data-conversation-theme='light']").forEach((el) => {
        el.setAttribute("data-conversation-theme", "dark");
      });
      document.querySelectorAll<HTMLElement>(".forge-app-shell[data-theme='light']").forEach((el) => {
        el.setAttribute("data-theme", "dark");
      });
    };

    new MutationObserver(apply).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-conversation-theme", "data-theme"],
      childList: true,
      subtree: true,
    });
    window.addEventListener("DOMContentLoaded", apply);
    apply();
  });
}

async function completeTurn(
  page: import("@playwright/test").Page,
  sessionId: string,
  content = "已经完成。",
) {
  const blockId = `result-${crypto.randomUUID()}`;
  await simulateStream(page, sessionId, [
    { event_type: "text_start", session_id: sessionId, block_id: blockId },
    { event_type: "text_chunk", session_id: sessionId, block_id: blockId, content },
    { event_type: "text_end", session_id: sessionId, block_id: blockId },
  ], 1);
}

async function openLatestProcess(page: import("@playwright/test").Page) {
  const disclosure = page.getByTestId("conversation-process-disclosure").last();
  const trigger = disclosure.getByTestId("conversation-process-trigger");
  await expect(trigger).toBeVisible();
  if (await trigger.getAttribute("aria-expanded") !== "true") await trigger.click();
  return disclosure;
}

async function revealLatestProcessDetails(page: import("@playwright/test").Page) {
  const disclosure = await openLatestProcess(page);
  const detailTriggers = disclosure.getByRole("button", { name: /^查看 .* 详情$/ });
  for (let remaining = await detailTriggers.count(); remaining > 0; remaining -= 1) {
    await detailTriggers.first().click();
  }
  return disclosure;
}

test.beforeEach(async ({ page }) => {
  await setup(page);
  await forceDarkWorkbench(page);
  await page.goto("http://localhost:1420");
  await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
});


  test("structured conversation blocks stay compact while collapsed", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, fullConversation(sessionId), 10);

    const disclosure = page.getByTestId("conversation-process-disclosure").first();
    const processSummary = disclosure.getByTestId("conversation-process-trigger");
    await expect(page.getByTestId("thinking-trigger")).toHaveCount(0);
    await expect(page.getByTestId("tool-activity-summary")).toHaveCount(0);
    await expect(processSummary).toBeVisible();
    await expect(processSummary).toHaveAttribute("aria-expanded", "false");
    await expect(processSummary).toContainText("已完成 · 2 项操作");

    const widths = await page.evaluate(() => {
      const process = document.querySelector("[data-testid='conversation-process-trigger']")?.getBoundingClientRect();
      const processNode = document.querySelector<HTMLElement>("[data-testid='conversation-process-trigger']");
      const processStyle = processNode ? getComputedStyle(processNode) : null;
      return process
        ? {
          process: Math.round(process.width),
          processBorderTop: processStyle ? Math.round(Number.parseFloat(processStyle.borderTopWidth)) : -1,
          processBackground: processStyle?.backgroundColor ?? "",
        }
        : null;
    });
    expect(widths).not.toBeNull();
    expect(widths!.process).toBeLessThanOrEqual(520);
    expect(widths!.processBorderTop).toBe(0);
    expect(widths!.processBackground).toBe("rgba(0, 0, 0, 0)");

    await processSummary.click();
    await expect(disclosure.getByTestId("conversation-process-item")).toHaveCount(3);
    const detailTriggers = disclosure.getByRole("button", { name: /^查看 .* 详情$/ });
    for (let remaining = await detailTriggers.count(); remaining > 0; remaining -= 1) await detailTriggers.first().click();
    const toolTrigger = disclosure.getByTestId("tool-card-trigger").first();
    const shellTrigger = disclosure.getByTestId("shell-card-trigger").first();
    await expect(toolTrigger).toBeVisible();
    await expect(shellTrigger).toBeVisible();
    await toolTrigger.click();
    await expect(page.getByRole("button", { name: "复制工具输出" }).first()).toBeVisible();
    await shellTrigger.click();
    await expect(page.getByRole("button", { name: "复制命令输出" }).first()).toBeVisible();
  });

  test("tool-heavy turns render as a quiet desktop work trail", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, fullConversation(sessionId), 10);
    const evidenceTurn = page.locator("[data-testid='conversation-turn'][data-turn-shape='with-evidence']");
    await expect(evidenceTurn).toHaveCount(1);
    await expect(evidenceTurn.first()).toHaveCSS("border-left-width", "0px");
    await expect(evidenceTurn.first()).toHaveCSS("padding-left", "0px");
  });

  test("composer aligns with the conversation lane and keeps pending state quiet", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const messageLane = page.getByTestId("message-lane");
    const composerLane = page.getByTestId("composer-lane");
    await expect(messageLane).toBeVisible();
    await expect(composerLane).toBeVisible();

    const layout = await page.evaluate(() => {
      const message = document.querySelector("[data-testid='message-lane']")?.getBoundingClientRect();
      const composer = document.querySelector("[data-testid='composer-lane']")?.getBoundingClientRect();
      return message && composer
        ? {
            messageX: Math.round(message.x),
            composerX: Math.round(composer.x),
            messageWidth: Math.round(message.width),
            composerWidth: Math.round(composer.width),
          }
        : null;
    });
    expect(layout).not.toBeNull();
    expect(layout!.messageWidth).toBeLessThanOrEqual(860);
    expect(layout!.composerWidth).toBeLessThanOrEqual(860);
    expect(Math.abs(layout!.messageX - layout!.composerX)).toBeLessThanOrEqual(4);
    await expect(composerLane.getByText("引用文件", { exact: true })).toHaveCount(0);
    await expect(composerLane.getByText("常用请求", { exact: true })).toHaveCount(0);
    await expect(composerLane.getByText("上下文 1M", { exact: true })).toHaveCount(0);
    await expect(composerLane.getByText("已启用能力")).toHaveCount(0);
    await expect(composerLane.getByRole("button", { name: /DeepSeek V4 Flash 1M/ })).toBeVisible();
    await expect(composerLane.getByText("DeepSeek V4 Flash 1M", { exact: true })).toBeVisible();

    await holdSendInput(page);

    await page.locator("textarea").fill("继续把对话区域靠近 Codex。");
    await page.locator("textarea").press("Enter");
    await expectHeldSendInput(page, "继续把对话区域靠近 Codex。");

    const pending = page.getByTestId("conversation-progress");
    await expect(pending).toBeVisible();
    await expect(pending).toHaveText(/正在理解任务/);
    await expect(pending).toHaveCSS("border-top-width", "0px");
    await expect(page.getByTestId("composer-surface")).toHaveAttribute("data-state", "running");
    await releaseHeldSendInput(page);
  });

  test("conversation shell keeps transcript rhythm while composer floats", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    const events = Array.from({ length: 24 }, (_, index) => ([
      { event_type: "text_start" as const, session_id: sessionId, block_id: `rhythm-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `rhythm-${index}`,
        content: `第 ${index + 1} 条输出，用来撑开滚动区域。这里保持足够长度，让对话区出现滚动。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `rhythm-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, events, 1);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.dispatchEvent(new WheelEvent("wheel", { deltaY: -160, bubbles: true }));
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();

    const rhythm = await page.evaluate(() => {
      const root = document.documentElement;
      const scroll = document.querySelector("[data-testid='conversation-scroll']");
      const composerFrame = document.querySelector("[data-testid='composer-frame']");
      const scrollButton = document.querySelector("[data-testid='scroll-to-bottom']");
      if (!scroll || !composerFrame || !scrollButton) return null;
      const scrollRect = scroll.getBoundingClientRect();
      const buttonRect = scrollButton.getBoundingClientRect();
      const token = getComputedStyle(root).getPropertyValue("--forge-conversation-gutter-y").trim();
      const scrollStyle = getComputedStyle(scroll);
      const composerStyle = getComputedStyle(composerFrame);

      return {
        token,
        scrollTop: Math.round(Number.parseFloat(scrollStyle.paddingTop)),
        scrollBottom: Math.round(Number.parseFloat(scrollStyle.paddingBottom)),
        composerTop: Math.round(Number.parseFloat(composerStyle.paddingTop)),
        composerBottom: Math.round(Number.parseFloat(composerStyle.paddingBottom)),
        scrollButtonBottom: Math.round(scrollRect.bottom - buttonRect.bottom),
      };
    });

    expect(rhythm).not.toBeNull();
    expect(rhythm!.token).toBe("18px");
    expect(rhythm!.scrollTop).toBe(18);
    expect(rhythm!.scrollBottom).toBe(18);
    expect(rhythm!.composerTop).toBe(14);
    expect(rhythm!.composerBottom).toBe(24);
    expect(rhythm!.scrollButtonBottom).toBe(18);
  });

  test("tool and shell logs share compact row rhythm", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call_start",
        session_id: sessionId,
        block_id: "compact-tool",
        tool_name: "read_file",
        tool_input: { path: "src/App.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "compact-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
      { event_type: "shell_start", session_id: sessionId, block_id: "compact-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "compact-shell", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "compact-shell", exit_code: 0 },
    ], 1);

    await completeTurn(page, sessionId);
    const disclosure = await openLatestProcess(page);
    await expect(disclosure.getByTestId("conversation-process-item")).toHaveCount(2);
    await expect(disclosure.getByText("已查看 App.tsx", { exact: true })).toBeVisible();
    await expect(disclosure.getByText("已验证构建", { exact: true })).toBeVisible();
    await expect(disclosure.getByTestId("tool-card-trigger")).toHaveCount(0);
    await expect(disclosure.getByTestId("shell-card-trigger")).toHaveCount(0);

    const metrics = await disclosure.evaluate((node) => {
      const rows = [...node.querySelectorAll<HTMLElement>(".forge-process-digest-row")];
      const content = node.querySelector<HTMLElement>(".forge-process-disclosure-content");
      return {
        rowHeights: rows.map((row) => Math.round(row.getBoundingClientRect().height)),
        rowBackgrounds: rows.map((row) => getComputedStyle(row).backgroundColor),
        contentWidth: content ? Math.round(content.getBoundingClientRect().width) : 0,
        detailCount: node.querySelectorAll(".forge-process-detail-trigger").length,
      };
    });
    expect(metrics.rowHeights.every((height) => height <= 24)).toBe(true);
    expect(metrics.rowBackgrounds.every((value) => value === "rgba(0, 0, 0, 0)")).toBe(true);
    expect(metrics.contentWidth).toBeLessThanOrEqual(620);
    expect(metrics.detailCount).toBe(2);
  });

  test("process rows keep long evidence and status anchored", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.setViewportSize({ width: 900, height: 680 });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call_start",
        session_id: sessionId,
        block_id: "anchored-tool",
        tool_name: "read_file",
        tool_input: {
          path: "src/components/messages/process-feedback/very-long-local-evidence-path-that-should-not-push-status-out-of-view.tsx",
        },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "anchored-tool",
        result: "ok",
        is_error: false,
        duration_ms: 1328,
      },
      {
        event_type: "shell_start",
        session_id: sessionId,
        block_id: "anchored-shell",
        command: "npm run build -- --mode production --workspace src/components/messages/process-feedback/very-long-command-name-with-flags",
      },
      { event_type: "shell_output", session_id: sessionId, block_id: "anchored-shell", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "anchored-shell", exit_code: 0 },
    ], 1);

    await completeTurn(page, sessionId);
    await revealLatestProcessDetails(page);

    const metrics = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const tool = document.querySelector<HTMLElement>("[data-testid='tool-card-trigger']");
      const shell = document.querySelector<HTMLElement>("[data-testid='shell-card-trigger']");
      const toolInput = tool?.querySelector<HTMLElement>(".forge-log-line-input");
      const toolDuration = tool?.querySelector<HTMLElement>(".forge-log-line-duration");
      const toolStatus = tool?.querySelector<HTMLElement>(".forge-log-line-status");
      const shellCommand = shell?.querySelector<HTMLElement>(".forge-log-line-command");
      const shellStatus = shell?.querySelector<HTMLElement>(".forge-log-line-status");
      if (!lane || !tool || !shell || !toolInput || !toolDuration || !toolStatus || !shellCommand || !shellStatus) return null;

      const toolInputStyle = getComputedStyle(toolInput);
      const shellCommandStyle = getComputedStyle(shellCommand);
      const laneWidth = Math.round(lane.getBoundingClientRect().width);
      const toolRect = tool.getBoundingClientRect();
      const shellRect = shell.getBoundingClientRect();
      const toolStatusRect = toolStatus.getBoundingClientRect();
      const shellStatusRect = shellStatus.getBoundingClientRect();

      return {
        laneWidth,
        toolWidth: Math.round(toolRect.width),
        shellWidth: Math.round(shellRect.width),
        toolInputOverflow: toolInputStyle.overflow,
        toolInputTextOverflow: toolInputStyle.textOverflow,
        toolInputWhiteSpace: toolInputStyle.whiteSpace,
        shellCommandOverflow: shellCommandStyle.overflow,
        shellCommandTextOverflow: shellCommandStyle.textOverflow,
        shellCommandWhiteSpace: shellCommandStyle.whiteSpace,
        toolStatusRightGap: Math.round(toolRect.right - toolStatusRect.right),
        shellStatusRightGap: Math.round(shellRect.right - shellStatusRect.right),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.toolWidth).toBeLessThanOrEqual(metrics!.laneWidth);
    expect(metrics!.shellWidth).toBeLessThanOrEqual(metrics!.laneWidth);
    expect(metrics!.toolInputOverflow).toBe("hidden");
    expect(metrics!.toolInputTextOverflow).toBe("ellipsis");
    expect(metrics!.toolInputWhiteSpace).toBe("nowrap");
    expect(metrics!.shellCommandOverflow).toBe("hidden");
    expect(metrics!.shellCommandTextOverflow).toBe("ellipsis");
    expect(metrics!.shellCommandWhiteSpace).toBe("nowrap");
    expect(metrics!.toolStatusRightGap).toBeLessThanOrEqual(12);
    expect(metrics!.shellStatusRightGap).toBeLessThanOrEqual(12);
  });

  test("delegate task trace uses shared warm process material", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call_start",
        session_id: sessionId,
        block_id: "delegate-trace",
        tool_name: "delegate_task",
        tool_input: { prompt: "检查消息渲染和过程反馈材料一致性" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "delegate-trace",
        result: JSON.stringify({
          result: "完成检查：SubAgentTrace 使用共享过程材料，长结果保持在消息 lane 内部滚动。",
          steps: [
            {
              round: 0,
              thinking: "先确认现有样式是否仍有硬编码冷色。",
              text: "找到了一个可以收进 token 系统的 trace 片段。",
              tool_calls: [
                {
                  name: "read_file",
                  input: "src/components/messages/SubAgentTrace.tsx",
                  result: "SubAgentTrace contains a long delegate result that should wrap quietly inside the raised process material without using cold debug text colors.",
                },
              ],
            },
          ],
        }),
        is_error: false,
        duration_ms: 864,
      },
    ], 1);

    await completeTurn(page, sessionId);
    await revealLatestProcessDetails(page);
    await page.getByTestId("tool-card-trigger").click();
    await expect(page.getByTestId("sub-agent-trace")).toBeVisible();
    await page.getByTestId("sub-agent-round-trigger").click();
    await page.getByTestId("sub-agent-tool-trigger").click();

    const metrics = await page.evaluate(() => {
      const resolveColor = (color: string) => {
        const probe = document.createElement("span");
        probe.style.color = color;
        document.body.append(probe);
        const resolved = getComputedStyle(probe).color;
        probe.remove();
        return resolved;
      };
      const trace = document.querySelector<HTMLElement>("[data-testid='sub-agent-trace']");
      const rounds = document.querySelector<HTMLElement>(".forge-sub-agent-rounds");
      const result = document.querySelector<HTMLElement>("[data-testid='sub-agent-result']");
      const toolResult = document.querySelector<HTMLElement>("[data-testid='sub-agent-tool-result']");
      if (!trace || !rounds || !result || !toolResult) return null;

      const rootStyle = getComputedStyle(trace);
      const traceStyle = getComputedStyle(trace);
      const roundsStyle = getComputedStyle(rounds);
      const resultStyle = getComputedStyle(result);
      const toolResultStyle = getComputedStyle(toolResult);

      return {
        materialRaised: resolveColor(rootStyle.getPropertyValue("--forge-material-raised").trim()),
        materialSurface: resolveColor(rootStyle.getPropertyValue("--forge-material-surface").trim()),
        materialBorder: resolveColor(rootStyle.getPropertyValue("--forge-material-border").trim()),
        traceBackground: traceStyle.backgroundColor,
        traceBorder: traceStyle.borderTopColor,
        traceRadius: Number.parseFloat(traceStyle.borderTopLeftRadius),
        roundsBorder: roundsStyle.borderBottomColor,
        resultOverflow: resultStyle.overflow,
        resultOverflowWrap: resultStyle.overflowWrap,
        resultMaxHeight: resultStyle.maxHeight,
        toolResultBackground: toolResultStyle.backgroundColor,
        toolResultOverflowWrap: toolResultStyle.overflowWrap,
        inlineStyleCount: trace.querySelectorAll("[style]").length + (trace.hasAttribute("style") ? 1 : 0),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.traceBackground).toBe(metrics!.materialRaised);
    expect(metrics!.traceBorder).toBe(metrics!.materialBorder);
    expect(metrics!.traceRadius).toBeLessThanOrEqual(8);
    expect(metrics!.roundsBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.resultOverflow).toBe("auto");
    expect(metrics!.resultOverflowWrap).toBe("anywhere");
    expect(metrics!.resultMaxHeight).toBe("200px");
    expect(metrics!.toolResultBackground).toBe(metrics!.materialSurface);
    expect(metrics!.toolResultOverflowWrap).toBe("anywhere");
    expect(metrics!.inlineStyleCount).toBe(0);
  });

  test("tool activity summary keeps dense process evidence on one quiet line", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.setViewportSize({ width: 860, height: 720 });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-read-a", tool_name: "read_file", tool_input: { path: "src/components/session/InputBar.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-read-a", result: "ok", is_error: false, duration_ms: 31 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-read-b", tool_name: "read_file", tool_input: { path: "src/styles/globals.css" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-read-b", result: "ok", is_error: false, duration_ms: 38 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-search", tool_name: "search_content", tool_input: { pattern: "forge-composer", path: "src" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-search", result: "ok", is_error: false, duration_ms: 48 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-edit", tool_name: "edit", tool_input: { path: "src/components/messages/ToolActivityGroup.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-edit", result: "ok", is_error: false, duration_ms: 82 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-web", tool_name: "web_fetch", tool_input: { url: "https://example.com/very/long/reference/path" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-web", result: "ok", is_error: false, duration_ms: 93 },
      { event_type: "shell_start", session_id: sessionId, block_id: "dense-check", command: "npm run build -- --mode production" },
      { event_type: "shell_output", session_id: sessionId, block_id: "dense-check", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "dense-check", exit_code: 0 },
    ], 1);

    await completeTurn(page, sessionId);
    const disclosure = page.getByTestId("conversation-process-disclosure").last();
    const summary = disclosure.getByTestId("conversation-process-trigger");
    await expect(summary).toContainText("✓ 已完成 · 6 项操作");
    await expect(summary).toHaveAttribute("aria-expanded", "false");
    await expect(disclosure.getByTestId("conversation-process-timeline")).toHaveCount(0);

    const collapsedMetrics = await summary.evaluate((node) => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const style = getComputedStyle(node);
      return {
        height: Math.round(node.getBoundingClientRect().height),
        width: Math.round(node.getBoundingClientRect().width),
        laneWidth: lane ? Math.round(lane.getBoundingClientRect().width) : 0,
        background: style.backgroundColor,
      };
    });
    expect(collapsedMetrics.height).toBeLessThanOrEqual(28);
    expect(collapsedMetrics.width).toBeLessThanOrEqual(collapsedMetrics.laneWidth);
    expect(collapsedMetrics.background).toBe("rgba(0, 0, 0, 0)");

    await summary.click();
    await expect(disclosure.getByTestId("conversation-process-item")).toHaveCount(6);
    const labels = disclosure.locator(".forge-process-digest-label");
    await expect(labels).toHaveCount(6);
    for (let index = 0; index < await labels.count(); index += 1) {
      const style = await labels.nth(index).evaluate((node) => ({
        overflow: getComputedStyle(node).overflow,
        textOverflow: getComputedStyle(node).textOverflow,
        whiteSpace: getComputedStyle(node).whiteSpace,
      }));
      expect(style).toEqual({ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" });
    }
  });

  test("expanded logs share one detail surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call_start",
        session_id: sessionId,
        block_id: "detail-tool",
        tool_name: "read_file",
        tool_input: { path: "src/App.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "detail-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
      { event_type: "shell_start", session_id: sessionId, block_id: "detail-shell", command: "npm run build" },
      {
        event_type: "shell_output",
        session_id: sessionId,
        block_id: "detail-shell",
        content: "stdout:\n/Users/cabbos/project/forge-test-app/src/components/really-long-output-path-with-build-artifacts.tsx:42: done",
      },
      { event_type: "shell_end", session_id: sessionId, block_id: "detail-shell", exit_code: 0 },
    ], 1);

    await completeTurn(page, sessionId);
    await revealLatestProcessDetails(page);
    await page.getByTestId("tool-card-trigger").click();
    await page.getByTestId("shell-card-trigger").click();

    const surfaces = await page.evaluate(() => {
      const root = document.documentElement;
      return [...document.querySelectorAll("[data-testid='log-detail-surface']")].map((surface) => {
        const style = getComputedStyle(surface);
        const header = surface.querySelector("[data-testid='log-detail-header']");
        const output = surface.querySelector("[data-testid='log-detail-output']");
        const shellPre = surface.querySelector(".forge-shell-output-section pre");
        const outputStyle = output ? getComputedStyle(output) : null;
        const shellPreStyle = shellPre ? getComputedStyle(shellPre) : null;
        return {
          maxHeightToken: getComputedStyle(root).getPropertyValue("--forge-log-output-max-height").trim(),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          headerHeight: header ? Math.round(header.getBoundingClientRect().height) : 0,
          detailBackground: style.backgroundColor,
          detailBorder: style.borderColor,
          outputMaxHeight: outputStyle?.maxHeight ?? "",
          outputPaddingTop: outputStyle ? Math.round(Number.parseFloat(outputStyle.paddingTop)) : 0,
          outputFontSize: outputStyle ? Number.parseFloat(outputStyle.fontSize) : 0,
          outputWordBreak: outputStyle?.wordBreak ?? "",
          shellPreWordBreak: shellPreStyle?.wordBreak ?? "",
          shellPreOverflowWrap: shellPreStyle?.overflowWrap ?? "",
        };
      });
    });

    expect(surfaces).toHaveLength(2);
    expect(surfaces.every((surface) => surface.maxHeightToken === "220px")).toBeTruthy();
    expect(surfaces.every((surface) => surface.radius <= 8)).toBeTruthy();
    expect(surfaces.every((surface) => surface.headerHeight === 32)).toBeTruthy();
    expect(surfaces.every((surface) => surface.detailBackground !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(surfaces.every((surface) => surface.detailBorder !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputMaxHeight === "220px")).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputPaddingTop === 7)).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputFontSize <= 11.5)).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputWordBreak === "normal")).toBeTruthy();
    const shellSurface = surfaces.find((surface) => surface.shellPreWordBreak);
    expect(shellSurface?.shellPreWordBreak).toBe("normal");
    expect(shellSurface?.shellPreOverflowWrap).toBe("anywhere");
  });

  test("desktop material baseline covers composer, process detail, popover, and work panel", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      { event_type: "shell_start", session_id: sessionId, block_id: "material-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "material-shell", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "material-shell", exit_code: 0 },
    ], 1);
    await completeTurn(page, sessionId);
    await revealLatestProcessDetails(page);
    await page.getByTestId("shell-card-trigger").click();
    await openWorkPanel(page);
    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    await expect(page.getByRole("menu")).toBeVisible();
    await expect.poll(async () => page.evaluate(() => {
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      if (!composer) return false;
      const root = getComputedStyle(composer);
      const materialBorderFocus = root.getPropertyValue("--forge-material-border-focus").trim();
      const colorProbe = document.createElement("span");
      document.body.append(colorProbe);
      colorProbe.style.color = materialBorderFocus;
      const expected = getComputedStyle(colorProbe).color;
      colorProbe.remove();
      return getComputedStyle(composer).borderTopColor === expected;
    })).toBe(true);

    const metrics = await page.evaluate(() => {
      const shell = document.querySelector<HTMLElement>(".forge-app-shell") ?? document.documentElement;
      const root = getComputedStyle(shell);
      const resolveColor = (color: string) => {
        const colorProbe = document.createElement("span");
        colorProbe.style.color = color;
        document.body.append(colorProbe);
        const resolved = getComputedStyle(colorProbe).color;
        colorProbe.remove();
        return resolved;
      };
      const materialBorder = root.getPropertyValue("--forge-material-border").trim();
      const materialBorderColor = resolveColor(materialBorder);
      const materialBorderFocus = root.getPropertyValue("--forge-material-border-focus").trim();
      const materialSurface = resolveColor(root.getPropertyValue("--forge-material-surface").trim());
      const materialSurfaceFocus = resolveColor(root.getPropertyValue("--forge-material-surface-focus").trim());
      const materialRaised = resolveColor(root.getPropertyValue("--forge-material-raised").trim());
      const materialPopover = resolveColor(root.getPropertyValue("--forge-material-popover").trim());
      const materialOverlay = root.getPropertyValue("--forge-material-overlay").trim();
      const materialShadow = root.getPropertyValue("--forge-material-shadow").trim();
      const materialShadowStrong = root.getPropertyValue("--forge-material-shadow-strong").trim();
      const composerSurfaceFocus = resolveColor(root.getPropertyValue("--forge-composer-surface-focus").trim());
      const composerShadowFocus = root.getPropertyValue("--forge-composer-shadow-focus").trim();
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const detail = document.querySelector<HTMLElement>("[data-testid='log-detail-surface']");
      const menu = document.querySelector<HTMLElement>(".forge-composer-model-menu");
      const panel = document.querySelector<HTMLElement>("aside.forge-work-panel");
      const panelUtilities = panel?.querySelector<HTMLElement>(".forge-work-panel-launcher-utilities");
      const separator = document.querySelector<HTMLElement>(".forge-work-panel-separator");
      if (!composer || !detail || !menu || !panel || !panelUtilities || !separator) return null;
      const composerStyle = getComputedStyle(composer);
      const detailStyle = getComputedStyle(detail);
      const menuStyle = getComputedStyle(menu);
      const panelStyle = getComputedStyle(panel);
      const panelUtilitiesStyle = getComputedStyle(panelUtilities);
      return {
        materialBorder,
        materialBorderColor,
        materialBorderFocus,
        materialSurface,
        materialSurfaceFocus,
        materialRaised,
        materialPopover,
        materialOverlay,
        materialShadow,
        materialShadowStrong,
        composerSurfaceFocus,
        composerShadowFocus,
        composerBorder: composerStyle.borderTopColor,
        composerBackground: composerStyle.backgroundColor,
        composerShadow: composerStyle.boxShadow,
        detailBorder: detailStyle.borderTopColor,
        detailBackground: detailStyle.backgroundColor,
        menuBorder: menuStyle.borderTopColor,
        menuBackground: menuStyle.backgroundColor,
        panelBackground: panelStyle.backgroundColor,
        panelSheet: resolveColor(root.getPropertyValue("--forge-work-panel-sheet").trim()),
        panelRadius: panelStyle.borderTopLeftRadius,
        panelShadow: panelStyle.boxShadow,
        panelUtilitiesHeight: Math.round(panelUtilities.getBoundingClientRect().height),
        panelUtilitiesBorder: panelUtilitiesStyle.borderBottomWidth,
        separatorWidth: Math.round(separator.getBoundingClientRect().width),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.composerBorder).toBe(metrics!.materialBorderFocus);
    expect(metrics!.composerBackground).toBe(metrics!.composerSurfaceFocus);
    expect(metrics!.materialShadowStrong).toContain("0 14px 32px");
    expect(metrics!.composerShadowFocus).toContain("0 14px 32px");
    expect(metrics!.composerShadow).toContain("14px 32px");
    expect(metrics!.detailBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.detailBackground).toBe(metrics!.materialRaised);
    expect(metrics!.menuBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.menuBackground).toBe(metrics!.materialPopover);
    expect(metrics!.panelBackground).toBe(metrics!.panelSheet);
    expect(metrics!.panelRadius).toBe("0px");
    expect(metrics!.panelShadow).toBe("none");
    expect(metrics!.panelUtilitiesHeight).toBeLessThanOrEqual(40);
    expect(metrics!.panelUtilitiesBorder).toBe("0px");
    expect(metrics!.separatorWidth).toBe(1);
  });

  test("core shell surfaces keep a restrained product radius", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await openWorkPanel(page);
    await expect(workPanel(page).getByTestId("work-panel-launcher")).toBeVisible();

    const radii = await page.evaluate(() => {
      const composer = document.querySelector("[data-testid='composer-lane'] > div:last-child");
      const launcherAction = document.querySelector(".forge-work-panel-launcher-action");
      return [composer, launcherAction]
        .filter(Boolean)
        .map((node) => Number.parseFloat(getComputedStyle(node as Element).borderTopLeftRadius));
    });

    expect(radii.length).toBeGreaterThanOrEqual(2);
    expect(radii.every((radius) => radius <= 8)).toBeTruthy();
  });

  test("write confirmation shows project boundary before approving", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "confirm-write-boundary",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          title: "准备修改项目",
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["src/App.tsx"],
          impact: "将修改 1 个文件",
          risk: "high",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: "这一步会覆盖现有文件，请确认路径与恢复点。",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(card.getByText("准备修改项目")).toBeVisible();
    await expect(card.getByText("目标项目", { exact: true })).toBeVisible();
    await expect(card.getByText("forge")).toBeVisible();
    await expect(card).not.toContainText("/Users/cabbos/project/forge");
    await expect(card.getByText("写入文件")).toBeVisible();
    await expect(card.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "继续" })).toBeVisible();
    await expect(card.getByRole("button", { name: "取消" })).toBeVisible();
    await expect(card.getByTestId("confirm-boundary-grid")).toBeVisible();
    await expect(card.getByTestId("confirm-boundary-row")).toHaveCount(5);
    await expect(card.getByTestId("confirm-warning")).toContainText("覆盖现有文件");
    await expect(card.getByTestId("confirm-action-bar")).toBeVisible();
    const confirmMetrics = await card.evaluate((node) => {
      const rows = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='confirm-boundary-row']"));
      const actionBar = node.querySelector<HTMLElement>("[data-testid='confirm-action-bar']");
      const primary = node.querySelector<HTMLElement>("[data-testid='confirm-approve']");
      const secondary = node.querySelector<HTMLElement>("[data-testid='confirm-cancel']");
      const warning = node.querySelector<HTMLElement>("[data-testid='confirm-warning']");
      const warningStyle = warning ? getComputedStyle(warning) : null;
      const before = getComputedStyle(node, "::before");
      const after = getComputedStyle(node, "::after");
      return {
        panelRadius: Number.parseFloat(getComputedStyle(node).borderTopLeftRadius),
        ticketWrappers: document.querySelectorAll(".permission-ticket").length,
        panelBefore: before.content,
        panelAfter: after.content,
        gridGap: Number.parseFloat(getComputedStyle(node.querySelector("[data-testid='confirm-boundary-grid']") as Element).rowGap),
        rowDisplay: rows[0] ? getComputedStyle(rows[0]).display : "",
        rowHeight: rows[0] ? Math.round(rows[0].getBoundingClientRect().height) : 0,
        warningRole: warning?.getAttribute("role") ?? "",
        warningHeight: warning ? Math.round(warning.getBoundingClientRect().height) : 0,
        warningRadius: warningStyle ? Number.parseFloat(warningStyle.borderTopLeftRadius) : 0,
        warningBackground: warningStyle?.backgroundColor ?? "",
        actionHeight: actionBar ? Math.round(actionBar.getBoundingClientRect().height) : 0,
        primaryHeight: primary ? Math.round(primary.getBoundingClientRect().height) : 0,
        secondaryHeight: secondary ? Math.round(secondary.getBoundingClientRect().height) : 0,
      };
    });
    expect(confirmMetrics.panelRadius).toBeLessThanOrEqual(8);
    expect(confirmMetrics.ticketWrappers).toBe(0);
    expect(confirmMetrics.panelBefore).toBe("none");
    expect(confirmMetrics.panelAfter).toBe("none");
    expect(confirmMetrics.gridGap).toBeLessThanOrEqual(2);
    expect(confirmMetrics.rowDisplay).toBe("grid");
    expect(confirmMetrics.rowHeight).toBeLessThanOrEqual(42);
    expect(confirmMetrics.warningRole).toBe("note");
    expect(confirmMetrics.warningHeight).toBeLessThanOrEqual(36);
    expect(confirmMetrics.warningRadius).toBeLessThanOrEqual(8);
    expect(confirmMetrics.warningBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(confirmMetrics.actionHeight).toBeLessThanOrEqual(42);
    expect(confirmMetrics.primaryHeight).toBe(28);
    expect(confirmMetrics.secondaryHeight).toBe(28);
  });

  test("resolved write confirmations collapse into quiet audit summaries", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "resolved-confirm-boundary",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          title: "准备修改项目",
          workspace_name: "forge-live-ops",
          workspace_path: "/Users/cabbos/project/forge-live-ops",
          operation: "write_file",
          affected_files: ["index.html"],
          impact: "1 个文件 · index.html",
          risk: "caution",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: "继续前确认改动范围。",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    const pendingHeight = await card.evaluate((node) => Math.round(node.getBoundingClientRect().height));
    await card.getByRole("button", { name: "继续" }).click();

    await expect(card).toHaveCount(0);
    await expect(page.getByText("已继续", { exact: true })).toHaveCount(0);
    await completeTurn(page, sessionId);

    const disclosure = page.getByTestId("conversation-process-disclosure").last();
    const trigger = disclosure.getByTestId("conversation-process-trigger");
    const collapsedHeight = await trigger.evaluate((node) => Math.round(node.getBoundingClientRect().height));
    expect(collapsedHeight).toBeLessThan(pendingHeight - 80);
    expect(collapsedHeight).toBeLessThanOrEqual(28);
    await trigger.click();
    await expect(disclosure.getByText("已处理确认", { exact: true })).toBeVisible();
    await expect(disclosure.getByText("Allow write_file?", { exact: true })).toHaveCount(0);
  });

  test("write confirmation bounds long file paths and commands", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "confirm-long-command-boundary",
        question: "Allow run_shell?",
        kind: "shell_cmd",
        boundary: {
          title: "准备执行命令",
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "run_shell",
          affected_files: [
            "src/features/conversation/surfaces/really-long-generated-output-path-without-natural-breaks/ExtremelyLongComponentNameForRegressionCoverage.tsx",
          ],
          impact: "将检查构建输出",
          risk: "medium",
          recovery: "继续前会保留可检查的交付状态",
          command: "npm run build -- --filter=src/features/conversation/surfaces/really-long-generated-output-path-without-natural-breaks/ExtremelyLongComponentNameForRegressionCoverage.tsx",
          warning: "确认命令仍在当前项目内执行。",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备执行命令" });
    await expect(card.getByTestId("confirm-boundary-grid").getByText("执行命令", { exact: true })).toBeVisible();
    const metrics = await card.evaluate((node) => {
      const command = node.querySelector<HTMLElement>(".forge-confirm-command");
      const chip = node.querySelector<HTMLElement>(".forge-confirm-file-chip");
      const commandStyle = command ? getComputedStyle(command) : null;
      const chipStyle = chip ? getComputedStyle(chip) : null;
      return {
        panelScrollWidth: Math.round((node as HTMLElement).scrollWidth),
        panelClientWidth: Math.round((node as HTMLElement).clientWidth),
        commandOverflowX: commandStyle?.overflowX ?? "",
        commandWhiteSpace: commandStyle?.whiteSpace ?? "",
        commandScrollbarWidth: commandStyle?.scrollbarWidth ?? "",
        commandOverscrollX: commandStyle?.overscrollBehaviorX ?? "",
        chipOverflow: chipStyle?.overflowX ?? "",
        chipTextOverflow: chipStyle?.textOverflow ?? "",
        chipMaxWidth: chipStyle?.maxWidth ?? "",
      };
    });

    expect(metrics.panelScrollWidth).toBeLessThanOrEqual(metrics.panelClientWidth + 1);
    expect(metrics.commandOverflowX).toBe("auto");
    expect(metrics.commandWhiteSpace).toBe("pre");
    expect(metrics.commandScrollbarWidth).toBe("thin");
    expect(metrics.commandOverscrollX).toBe("contain");
    expect(metrics.chipOverflow).toBe("hidden");
    expect(metrics.chipTextOverflow).toBe("ellipsis");
    expect(metrics.chipMaxWidth).toBe("100%");
  });

  test("pending background records stay hidden from the delivery surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "record-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：交付状态可以继续验收。",
          record_label: "建议更新项目记录",
          record_status: "pending",
          record_target_pages: ["tasks.md", "log.md"],
        },
      },
    ], 5);

    await completeTurn(page, sessionId);
    const disclosure = page.getByTestId("conversation-process-disclosure").last();
    await expect(page.getByTestId("delivery-summary-grid")).toHaveCount(0);
    await expect(disclosure.getByText("自动记录")).toHaveCount(0);
    await expect(disclosure.getByText("建议更新项目记录")).toHaveCount(0);
    const nextAction = disclosure.getByTestId("conversation-next-action");
    await expect(nextAction).toHaveText("下一步：交付状态可以继续验收。");
    await nextAction.click();
    await expect(page.locator("textarea")).toHaveValue("下一步：交付状态可以继续验收。");
  });

  test("diff views read like a professional patch surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    const diffLines = [
      "diff --git a/src/components/App.tsx b/src/components/App.tsx",
      "index 1111111..2222222 100644",
      "--- a/src/components/App.tsx",
      "+++ b/src/components/App.tsx",
      "@@ -10,8 +10,32 @@ export function App() {",
      "-  return <div>demo</div>;",
      "+  return <main className=\"forge-shell\">",
      "+    <h1>Forge</h1>",
      "+  </main>;",
      " }",
      ...Array.from({ length: 34 }, (_, index) => `+  const line${index + 1} = ${index + 1};`),
    ].join("\n");

    await simulateStream(page, sessionId, [
      {
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "reader-diff",
        file_path: "src/components/App.tsx",
        old_content: "",
        new_content: diffLines,
      },
    ], 1);

    await completeTurn(page, sessionId);
    await revealLatestProcessDetails(page);
    const diff = page.getByTestId("diff-card");
    await expect(diff).toBeVisible();
    await expect(diff.getByTestId("diff-file-path")).toHaveText("src/components/App.tsx");
    await expect(diff.getByTestId("diff-stat")).toContainText("+37");
    await expect(diff.getByTestId("diff-stat")).toContainText("-1");
    await expect(diff.getByTestId("diff-summary")).toContainText("1 个变更块");
    await expect(diff.getByTestId("diff-summary")).toContainText("首处第 10 行");
    await expect(diff.getByTestId("diff-summary")).toContainText("44 行");
    await expect(diff.getByRole("button", { name: "复制 diff" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "打开文件" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "定位首处改动" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "查看改动" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "展开完整改动" })).toHaveCount(0);
    await expect(diff.getByText("line34")).toHaveCount(0);

    const collapsedMetrics = await diff.evaluate((node) => {
      const panel = node.querySelector<HTMLElement>("[data-testid='message-panel']");
      const summary = node.querySelector<HTMLElement>("[data-testid='diff-summary']");
      const toggle = node.querySelector<HTMLElement>("[data-testid='diff-body-toggle']");
      const body = node.querySelector(".forge-diff-body");
      if (!panel || !summary || !toggle) return null;
      const panelStyle = getComputedStyle(panel);
      const summaryStyle = getComputedStyle(summary);
      return {
        openState: panel.dataset.diffOpen,
        bodyVisible: Boolean(body),
        summaryBorderBottom: Math.round(Number.parseFloat(summaryStyle.borderBottomWidth)),
        toggleHeight: Math.round(toggle.getBoundingClientRect().height),
        background: panelStyle.backgroundColor,
      };
    });

    expect(collapsedMetrics).not.toBeNull();
    expect(collapsedMetrics!.openState).toBe("false");
    expect(collapsedMetrics!.bodyVisible).toBe(false);
    expect(collapsedMetrics!.summaryBorderBottom).toBe(0);
    expect(collapsedMetrics!.toggleHeight).toBe(24);

    await diff.getByRole("button", { name: "查看改动" }).click();
    await expect(diff.getByRole("button", { name: "隐藏改动" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "展开完整改动" })).toBeVisible();

    const metrics = await diff.evaluate((node) => {
      const panel = node.querySelector("[data-testid='message-panel']");
      const added = node.querySelector("[data-testid='diff-line-added']");
      const removed = node.querySelector("[data-testid='diff-line-removed']");
      const hunk = node.querySelector("[data-testid='diff-line-hunk']");
      const oldNo = node.querySelector("[data-testid='diff-line-old-number']");
      const newNo = node.querySelector("[data-testid='diff-line-new-number']");
      const body = node.querySelector(".forge-diff-body");
      const summary = node.querySelector("[data-testid='diff-summary']");
      const code = node.querySelector(".forge-diff-line-code");
      if (!panel || !added || !removed || !hunk || !oldNo || !newNo || !body || !summary || !code) return null;
      const panelStyle = getComputedStyle(panel);
      const addedStyle = getComputedStyle(added);
      const removedStyle = getComputedStyle(removed);
      const hunkStyle = getComputedStyle(hunk);
      const bodyStyle = getComputedStyle(body);
      const summaryStyle = getComputedStyle(summary);
      const codeStyle = getComputedStyle(code);
      const cardStyle = getComputedStyle(node);
      const expectedBackgroundProbe = document.createElement("span");
      expectedBackgroundProbe.style.background = cardStyle.getPropertyValue("--forge-material-raised").trim();
      document.body.append(expectedBackgroundProbe);
      const expectedBackground = getComputedStyle(expectedBackgroundProbe).backgroundColor;
      expectedBackgroundProbe.remove();
      return {
        openState: (panel as HTMLElement).dataset.diffOpen,
        cardWidth: Math.round(panel.getBoundingClientRect().width),
        cardBackground: cardStyle.backgroundColor,
        expectedBackground,
        perfRows: node.querySelectorAll(".diff-filmstrip-perf").length,
        grid: getComputedStyle(added).display,
        oldNumberWidth: Math.round(oldNo.getBoundingClientRect().width),
        newNumberWidth: Math.round(newNo.getBoundingClientRect().width),
        maxWidth: panelStyle.maxWidth,
        bodyMaxHeight: bodyStyle.maxHeight,
        summaryHeight: Math.round(summary.getBoundingClientRect().height),
        lineMinHeight: Math.round(Number.parseFloat(addedStyle.minHeight)),
        codePaddingLeft: Math.round(Number.parseFloat(codeStyle.paddingLeft)),
        addedBackground: addedStyle.backgroundColor,
        addedBorderLeft: addedStyle.borderLeftWidth,
        removedBackground: removedStyle.backgroundColor,
        removedBorderLeft: removedStyle.borderLeftWidth,
        hunkBorderTop: hunkStyle.borderTopWidth,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.openState).toBe("true");
    expect(metrics!.cardWidth).toBeLessThanOrEqual(760);
    expect(metrics!.cardBackground).toBe(metrics!.expectedBackground);
    expect(metrics!.perfRows).toBe(0);
    expect(metrics!.maxWidth).not.toBe("none");
    expect(metrics!.bodyMaxHeight).toBe("320px");
    expect(metrics!.summaryHeight).toBe(26);
    expect(metrics!.lineMinHeight).toBe(18);
    expect(metrics!.codePaddingLeft).toBeLessThanOrEqual(10);
    expect(metrics!.grid).toBe("grid");
    expect(metrics!.oldNumberWidth).toBeGreaterThanOrEqual(36);
    expect(metrics!.newNumberWidth).toBeGreaterThanOrEqual(36);
    expect(metrics!.addedBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.addedBorderLeft).toBe("0px");
    expect(metrics!.removedBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removedBorderLeft).toBe("0px");
    expect(metrics!.hunkBorderTop).toBe("1px");

    await diff.getByRole("button", { name: "展开完整改动" }).click();
    await expect(diff.getByText("line34")).toBeVisible();
  });

  test("diff file actions stay scoped to the active workspace", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const projectPath = "/Users/cabbos/project/forge";
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.evaluate(async () => {
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "workspace-bound-diff",
        file_path: "src/DemoApp.tsx",
        old_content: "",
        new_content: [
          "diff --git a/src/DemoApp.tsx b/src/DemoApp.tsx",
          "--- a/src/DemoApp.tsx",
          "+++ b/src/DemoApp.tsx",
          "@@ -2,1 +2,1 @@",
          "-old",
          "+new",
        ].join("\n"),
      },
    ], 1);

    await completeTurn(page, sessionId);
    await revealLatestProcessDetails(page);
    const diff = page.getByTestId("diff-card");
    await diff.getByRole("button", { name: "打开文件" }).click();
    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastOpenFileArgs;
    })).toMatchObject({
      path: "src/DemoApp.tsx",
      sessionId,
      workingDir: projectPath,
    });

    await diff.getByRole("button", { name: "定位首处改动" }).click();
    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastPreviewFileArgs;
    })).toMatchObject({
      path: "src/DemoApp.tsx",
      line: 2,
      sessionId,
      workingDir: projectPath,
    });
  });

  test("consecutive tool activity becomes one process evidence group", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "tool-story-a" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "tool-story-a", content: "我先看一下项目结构。" },
      { event_type: "text_end", session_id: sessionId, block_id: "tool-story-a" },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "tool-read", tool_name: "read_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "tool-read", result: "export function App() {}", is_error: false, duration_ms: 32 },
      { event_type: "shell_start", session_id: sessionId, block_id: "tool-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "tool-shell", content: "stdout:\nBuild started\nstderr:\nError: Cannot find module\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "tool-shell", exit_code: 1 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "tool-write", tool_name: "write_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "tool-write", result: "权限不足：无法写入 src/App.tsx", is_error: true, duration_ms: 45 },
      { event_type: "text_start", session_id: sessionId, block_id: "tool-story-b" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "tool-story-b", content: "问题在依赖解析上。" },
      { event_type: "text_end", session_id: sessionId, block_id: "tool-story-b" },
    ], 1);

    const group = await revealLatestProcessDetails(page);
    await expect(group.getByTestId("conversation-process-item")).toHaveCount(3);
    await expect(group.getByText("已查看 App.tsx", { exact: true })).toBeVisible();
    await expect(group.getByText("已验证构建", { exact: true })).toBeVisible();
    await expect(group.getByText("已调整 App.tsx", { exact: true })).toBeVisible();
    await expect(group.locator(".forge-process-digest-node[data-outcome='failed']")).toHaveCount(2);

    const failedShell = group.getByTestId("shell-card-trigger");
    await expect(failedShell).toHaveAttribute("aria-expanded", "true");
    const failedTool = group.getByTestId("tool-card-trigger").last();
    await expect(failedTool).toHaveAttribute("aria-expanded", "true");
    await expect(group.getByTestId("shell-exit-code")).toHaveText("exit 1");
    await expect(group.getByTestId("tool-result-summary")).toContainText("权限不足");
    await expect(group.getByTestId("shell-output-section").filter({ hasText: "stderr" })).toContainText("Cannot find module");

    const failureMetrics = await group.evaluate((node) => {
      const detail = node.querySelector<HTMLElement>("[data-testid='log-detail-surface']");
      const stderr = node.querySelector<HTMLElement>("[data-testid='shell-output-section'][data-tone='error']");
      const detailStyle = detail ? getComputedStyle(detail) : null;
      const stderrStyle = stderr ? getComputedStyle(stderr) : null;
      return {
        detailTone: detail?.getAttribute("data-tone") ?? "",
        detailBorder: detailStyle?.borderTopColor ?? "",
        stderrBackground: stderrStyle?.backgroundColor ?? "",
        stderrBorderLeft: stderrStyle ? Math.round(Number.parseFloat(stderrStyle.borderLeftWidth)) : 0,
        stderrPaddingLeft: stderrStyle ? Math.round(Number.parseFloat(stderrStyle.paddingLeft)) : 0,
        stderrRadius: stderrStyle ? Number.parseFloat(stderrStyle.borderTopLeftRadius) : 0,
      };
    });
    expect(failureMetrics.detailTone).toBe("error");
    expect(failureMetrics.detailBorder).not.toBe("rgba(210, 204, 190, 0.14)");
    expect(failureMetrics.stderrBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(failureMetrics.stderrBorderLeft).toBe(1);
    expect(failureMetrics.stderrPaddingLeft).toBeGreaterThanOrEqual(8);
    expect(failureMetrics.stderrRadius).toBeLessThanOrEqual(8);
  });

  test("successful tool activity collapses into one handled summary", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await simulateStream(page, sessionId, [
      { event_type: "tool_call_start", session_id: sessionId, block_id: "success-read", tool_name: "read_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "success-read", result: "export function App() {}", is_error: false, duration_ms: 22 },
      { event_type: "shell_start", session_id: sessionId, block_id: "success-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "success-shell", content: "stdout:\nBuild complete\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "success-shell", exit_code: 0 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "success-write", tool_name: "write_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "success-write", result: "ok", is_error: false, duration_ms: 31 },
    ], 1);

    await completeTurn(page, sessionId);
    const group = page.getByTestId("conversation-process-disclosure").last();
    const summary = group.getByTestId("conversation-process-trigger");
    await expect(summary).toBeVisible();
    await expect(summary).toHaveAttribute("aria-expanded", "false");
    await expect(summary).toContainText("✓ 已完成 · 3 项操作");
    await expect(group.getByTestId("conversation-process-timeline")).toHaveCount(0);
    await expect(group.getByText("已查看 App.tsx", { exact: true })).toHaveCount(0);
    await expect(group.getByText("已验证构建", { exact: true })).toHaveCount(0);

    const metrics = await summary.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        height: Math.round(node.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
      };
    });
    expect(metrics.height).toBeLessThanOrEqual(28);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.background).toBe("rgba(0, 0, 0, 0)");

    await summary.click();
    await expect(summary).toHaveAttribute("aria-expanded", "true");
    await expect(group.getByText("已查看 App.tsx", { exact: true })).toBeVisible();
    await expect(group.getByText("已验证构建", { exact: true })).toBeVisible();
    await expect(group.getByText("已调整 App.tsx", { exact: true })).toBeVisible();
  });

  test("waiting and thinking states stay quiet but specific", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    await holdSendInput(page);

    await page.locator("textarea").fill("继续优化等待状态");
    await page.locator("textarea").press("Enter");
    await expectHeldSendInput(page, "继续优化等待状态");

    const pending = page.getByTestId("conversation-progress");
    await expect(pending).toHaveText(/正在理解任务/);
    await expect(pending).toHaveAttribute("role", "status");
    await expect(pending).toHaveAttribute("aria-live", "polite");
    await expect(pending.locator(".forge-turn-progress-dot")).toBeVisible();
    await expect(pending).toHaveCSS("border-top-width", "0px");
    const pendingMetrics = await pending.evaluate((node) => ({
      height: Math.round(node.getBoundingClientRect().height),
      minHeight: Math.round(Number.parseFloat(getComputedStyle(node).minHeight)),
      color: getComputedStyle(node).color,
      background: getComputedStyle(node).backgroundColor,
      borderTop: Math.round(Number.parseFloat(getComputedStyle(node).borderTopWidth)),
      fontSize: Number.parseFloat(getComputedStyle(node).fontSize),
      gap: Math.round(Number.parseFloat(getComputedStyle(node).columnGap)),
    }));

    await simulateStream(page, sessionId, [
      { event_type: "thinking_start", session_id: sessionId, block_id: "quiet-thinking" },
      { event_type: "thinking_chunk", session_id: sessionId, block_id: "quiet-thinking", content: "Need to inspect the failure before editing." },
    ], 1);

    const thinking = page.getByTestId("conversation-progress");
    await expect(thinking).toHaveText(/正在理解任务/);
    await expect(page.getByText("Need to inspect the failure before editing.")).toHaveCount(0);

    const metrics = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='conversation-progress']");
      if (!thinking) return null;
      return {
        thinkingHeight: Math.round(thinking.getBoundingClientRect().height),
        thinkingMinHeight: Math.round(Number.parseFloat(getComputedStyle(thinking).minHeight)),
        thinkingColor: getComputedStyle(thinking).color,
        thinkingBackground: getComputedStyle(thinking).backgroundColor,
        thinkingBorderTop: Math.round(Number.parseFloat(getComputedStyle(thinking).borderTopWidth)),
        thinkingFontSize: Number.parseFloat(getComputedStyle(thinking).fontSize),
        thinkingGap: Math.round(Number.parseFloat(getComputedStyle(thinking).columnGap)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(pendingMetrics.minHeight).toBe(24);
    expect(metrics!.thinkingMinHeight).toBe(24);
    expect(pendingMetrics.height).toBeGreaterThanOrEqual(24);
    expect(metrics!.thinkingHeight).toBeGreaterThanOrEqual(24);
    expect(pendingMetrics.fontSize).toBeCloseTo(12);
    expect(metrics!.thinkingFontSize).toBeCloseTo(12);
    expect(pendingMetrics.gap).toBe(8);
    expect(metrics!.thinkingGap).toBe(8);
    expect(pendingMetrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.thinkingBackground).toBe("rgba(0, 0, 0, 0)");
    expect(pendingMetrics.borderTop).toBe(0);
    expect(metrics!.thinkingBorderTop).toBe(0);
    expect(pendingMetrics.color).toBe(metrics!.thinkingColor);
    await releaseHeldSendInput(page);
  });

  test("completed thinking stays private and folds into the process digest", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    const thinkingId = crypto.randomUUID();
    const answerId = crypto.randomUUID();
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "thinking_start", session_id: sessionId, block_id: thinkingId },
      { event_type: "thinking_chunk", session_id: sessionId, block_id: thinkingId, content: "I need to analyze the auth system first." },
      { event_type: "thinking_end", session_id: sessionId, block_id: thinkingId },
      { event_type: "text_start", session_id: sessionId, block_id: answerId },
      { event_type: "text_chunk", session_id: sessionId, block_id: answerId, content: "Done analyzing." },
      { event_type: "text_end", session_id: sessionId, block_id: answerId },
    ], 30);

    await expect(page.getByTestId("assistant-message")).toContainText("Done analyzing.");
    await expect(page.getByTestId("thinking-trigger")).toHaveCount(0);
    await expect(page.getByText("I need to analyze the auth system first.")).toHaveCount(0);
    const disclosure = await openLatestProcess(page);
    await expect(disclosure.getByText("已理解任务", { exact: true })).toBeVisible();
  });

  test("tool work moves from one live progress row into completed evidence", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    const toolId = crypto.randomUUID();
    // Send tool_start first (running state)
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "tool_call_start", session_id: sessionId, block_id: toolId, tool_name: "read_file", tool_input: { path: "test.rs" } },
    ], 30);

    const runningTool = page.getByTestId("conversation-progress");
    await expect(runningTool).toBeVisible({ timeout: 3000 });
    await expect(runningTool).toHaveText("正在查看 test.rs");
    await expect(runningTool).toHaveAttribute("data-progress-id", "read:test.rs");
    await expect(page.getByText("进行中", { exact: true })).toHaveCount(0);

    // Send tool_result (done state)
    await simulateStream(page, sessionId, [
      { event_type: "tool_call_result", session_id: sessionId, block_id: toolId, result: "fn main() {}", is_error: false, duration_ms: 100 },
    ], 30);

    await completeTurn(page, sessionId);
    const disclosure = await openLatestProcess(page);
    await expect(disclosure.getByText("已查看 test.rs", { exact: true })).toBeVisible();
    await disclosure.getByRole("button", { name: "查看 已查看 test.rs 详情" }).click();
    const doneTool = disclosure.getByTestId("tool-card-trigger");
    await expect(doneTool).toBeVisible({ timeout: 3000 });
    await expect(doneTool).toHaveAttribute("data-state", "done");
    await expect(doneTool).toContainText("100ms");
    await expect(disclosure.getByText("完成", { exact: true })).toHaveCount(1);
  });

  test("shell work moves from one live progress row into completed evidence", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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

    const shellId = crypto.randomUUID();
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "shell_start", session_id: sessionId, block_id: shellId, command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: shellId, content: "stdout:\nbuilding..." },
    ], 30);

    const runningShell = page.getByTestId("conversation-progress");
    await expect(runningShell).toBeVisible({ timeout: 3000 });
    await expect(runningShell).toHaveText("正在验证构建");
    await expect(runningShell).toHaveAttribute("data-progress-id", "verify:build");
    const runningMetrics = await runningShell.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        minHeight: Math.round(Number.parseFloat(getComputedStyle(node).minHeight)),
        background: style.backgroundColor,
        borderTop: Math.round(Number.parseFloat(style.borderTopWidth)),
      };
    });
    expect(runningMetrics.minHeight).toBe(24);
    expect(runningMetrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(runningMetrics.borderTop).toBe(0);

    await simulateStream(page, sessionId, [
      { event_type: "shell_end", session_id: sessionId, block_id: shellId, exit_code: 0 },
    ], 30);

    await completeTurn(page, sessionId);
    const disclosure = await openLatestProcess(page);
    await expect(disclosure.getByText("已验证构建", { exact: true })).toBeVisible();
    await disclosure.getByRole("button", { name: "查看 已验证构建 详情" }).click();
    const doneShell = disclosure.getByTestId("shell-card-trigger");
    await expect(doneShell).toHaveAttribute("data-state", "done");
    const doneTone = await doneShell.evaluate((node) =>
      node.querySelector<HTMLElement>(".forge-log-status")?.getAttribute("data-tone"),
    );
    expect(doneTone).toBe("success");
  });
