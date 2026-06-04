import { test, expect } from "@playwright/test";
import { setup, holdSendInput, expectHeldSendInput, releaseHeldSendInput, openProjectArchive, projectArchive } from "./fixtures/app";
import { simulateStream, fullConversation } from "./mock-ipc";


  test.beforeEach(async ({ page }) => {
    await setup(page);
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

    const thinkingTrigger = page.getByTestId("thinking-trigger").first();
    const processSummary = page.getByTestId("tool-activity-summary").first();
    await expect(thinkingTrigger).toBeVisible();
    await expect(processSummary).toBeVisible();
    await expect(processSummary).toHaveAttribute("aria-expanded", "false");

    const widths = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']")?.getBoundingClientRect();
      const process = document.querySelector("[data-testid='tool-activity-summary']")?.getBoundingClientRect();
      const processNode = document.querySelector<HTMLElement>("[data-testid='tool-activity-summary']");
      const processStyle = processNode ? getComputedStyle(processNode) : null;
      return thinking && process
        ? {
          thinking: Math.round(thinking.width),
          process: Math.round(process.width),
          processBorderTop: processStyle ? Math.round(Number.parseFloat(processStyle.borderTopWidth)) : -1,
          processBackground: processStyle?.backgroundColor ?? "",
        }
        : null;
    });
    expect(widths).not.toBeNull();
    expect(widths!.thinking).toBeLessThanOrEqual(220);
    expect(widths!.process).toBeLessThanOrEqual(520);
    await expect(thinkingTrigger).toHaveCSS("border-top-width", "0px");
    expect(widths!.processBorderTop).toBe(0);
    expect(widths!.processBackground).toBe("rgba(0, 0, 0, 0)");

    await processSummary.click();
    const toolTrigger = page.getByTestId("tool-card-trigger").first();
    const shellTrigger = page.getByTestId("shell-card-trigger").first();
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

    const pending = page.getByTestId("pending-block");
    await expect(pending).toBeVisible();
    await expect(pending).toHaveText(/正在组织回答/);
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
        event_type: "tool_call",
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

    await page.getByTestId("tool-activity-summary").click();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const group = document.querySelector("[data-testid='tool-activity-group']");
      const summary = document.querySelector("[data-testid='tool-activity-summary']");
      const list = document.querySelector(".forge-tool-activity-list");
      const tool = document.querySelector("[data-testid='tool-card-trigger']");
      const shell = document.querySelector("[data-testid='shell-card-trigger']");
      if (!group || !summary || !list || !tool || !shell) return null;
      const shellWrapper = shell.closest(".shell-reel");
      const shellBody = shell.closest(".shell-reel-body");
      const groupStyle = getComputedStyle(group);
      const summaryStyle = getComputedStyle(summary);
      const listStyle = getComputedStyle(list);
      const toolStyle = getComputedStyle(tool);
      const shellWrapperStyle = shellWrapper ? getComputedStyle(shellWrapper) : null;
      const shellBodyStyle = shellBody ? getComputedStyle(shellBody) : null;
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-log-row-height").trim(),
        groupWidth: Math.round(group.getBoundingClientRect().width),
        groupBorderLeft: Math.round(Number.parseFloat(groupStyle.borderLeftWidth)),
        summaryHeight: Math.round(summary.getBoundingClientRect().height),
        summaryDisplay: summaryStyle.display,
        summaryBackground: summaryStyle.backgroundColor,
        summaryBorderTop: Math.round(Number.parseFloat(summaryStyle.borderTopWidth)),
        listGap: Math.round(Number.parseFloat(listStyle.gap)),
        toolHeight: Math.round(tool.getBoundingClientRect().height),
        toolRadius: Number.parseFloat(toolStyle.borderTopLeftRadius),
        toolBorder: toolStyle.borderTopColor,
        toolBackground: toolStyle.backgroundColor,
        shellHeight: Math.round(shell.getBoundingClientRect().height),
        toolMargin: getComputedStyle(tool.parentElement as Element).marginBottom,
        shellMargin: getComputedStyle(shell.parentElement as Element).marginBottom,
        toolMeterCount: document.querySelectorAll(".tool-machine-meter").length,
        toolLedCount: document.querySelectorAll(".tool-machine-led").length,
        shellCapCount: document.querySelectorAll(".shell-reel-cap").length,
        shellWrapperMarginTop: shellWrapperStyle ? Math.round(Number.parseFloat(shellWrapperStyle.marginTop)) : -1,
        shellWrapperBackground: shellWrapperStyle?.backgroundColor ?? "",
        shellBodyRadius: shellBodyStyle ? Number.parseFloat(shellBodyStyle.borderTopLeftRadius) : 0,
        shellBodyBorder: shellBodyStyle?.borderTopColor ?? "",
        shellBodyBackground: shellBodyStyle?.backgroundColor ?? "",
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.token).toBe("22px");
    expect(metrics!.groupWidth).toBeLessThanOrEqual(760);
    expect(metrics!.groupBorderLeft).toBe(0);
    expect(metrics!.summaryHeight).toBeLessThanOrEqual(24);
    expect(metrics!.summaryDisplay).toBe("inline-flex");
    expect(metrics!.summaryBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.summaryBorderTop).toBe(0);
    expect(metrics!.listGap).toBe(2);
    expect(metrics!.toolHeight).toBe(44);
    expect(metrics!.toolRadius).toBeLessThanOrEqual(8);
    expect(metrics!.toolBorder).toBe("rgb(216, 201, 184)");
    expect(metrics!.toolBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.shellHeight).toBe(44);
    expect(metrics!.toolMargin).toBe("0px");
    expect(metrics!.shellMargin).toBe("0px");
    expect(metrics!.toolMeterCount).toBe(0);
    expect(metrics!.toolLedCount).toBe(0);
    expect(metrics!.shellCapCount).toBe(0);
    expect(metrics!.shellWrapperMarginTop).toBe(0);
    expect(metrics!.shellWrapperBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.shellBodyRadius).toBeLessThanOrEqual(8);
    expect(metrics!.shellBodyBorder).toBe("rgb(216, 201, 184)");
    expect(metrics!.shellBodyBackground).not.toBe("rgba(0, 0, 0, 0)");
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

    await page.getByTestId("tool-activity-summary").click();

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

    await page.getByTestId("tool-card-trigger").click();
    await expect(page.getByTestId("sub-agent-trace")).toBeVisible();
    await page.getByTestId("sub-agent-round-trigger").click();
    await page.getByTestId("sub-agent-tool-trigger").click();

    const metrics = await page.evaluate(() => {
      const rootStyle = getComputedStyle(document.documentElement);
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

      const traceStyle = getComputedStyle(trace);
      const roundsStyle = getComputedStyle(rounds);
      const resultStyle = getComputedStyle(result);
      const toolResultStyle = getComputedStyle(toolResult);

      return {
        materialRaised: rootStyle.getPropertyValue("--forge-material-raised").trim(),
        materialSurface: rootStyle.getPropertyValue("--forge-material-surface").trim(),
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

    const metrics = await page.getByTestId("tool-activity-summary").evaluate((node) => {
      const summary = node as HTMLElement;
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const items = Array.from(summary.querySelectorAll<HTMLElement>(".forge-tool-activity-summary-item"));
      const summaryStyle = getComputedStyle(summary);
      const itemStyles = items.map((item) => {
        const style = getComputedStyle(item);
        return {
          overflow: style.overflow,
          textOverflow: style.textOverflow,
          whiteSpace: style.whiteSpace,
          width: Math.round(item.getBoundingClientRect().width),
        };
      });
      return {
        itemCount: items.length,
        summaryHeight: Math.round(summary.getBoundingClientRect().height),
        summaryWidth: Math.round(summary.getBoundingClientRect().width),
        laneWidth: lane ? Math.round(lane.getBoundingClientRect().width) : 0,
        overflow: summaryStyle.overflow,
        whiteSpace: summaryStyle.whiteSpace,
        itemStyles,
      };
    });

    expect(metrics.itemCount).toBeGreaterThanOrEqual(5);
    expect(metrics.summaryHeight).toBeLessThanOrEqual(28);
    expect(metrics.summaryWidth).toBeLessThanOrEqual(metrics.laneWidth);
    expect(metrics.overflow).toBe("hidden");
    expect(metrics.whiteSpace).toBe("nowrap");
    expect(metrics.itemStyles.every((item) => item.overflow === "hidden")).toBe(true);
    expect(metrics.itemStyles.every((item) => item.textOverflow === "ellipsis")).toBe(true);
    expect(metrics.itemStyles.every((item) => item.whiteSpace === "nowrap")).toBe(true);
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
        event_type: "tool_call",
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

    await page.getByTestId("tool-activity-summary").click();
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

  test("desktop material baseline covers composer, process detail, popover, and archive", async ({ page }) => {
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
    await page.getByTestId("shell-card-trigger").click();
    await openProjectArchive(page);
    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    await expect(page.getByRole("menu")).toBeVisible();
    await expect.poll(async () => page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      const materialBorderFocus = root.getPropertyValue("--forge-material-border-focus").trim();
      const colorProbe = document.createElement("span");
      document.body.append(colorProbe);
      colorProbe.style.color = materialBorderFocus;
      const expected = getComputedStyle(colorProbe).color;
      colorProbe.remove();
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      return composer ? getComputedStyle(composer).borderTopColor === expected : false;
    })).toBe(true);

    const metrics = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      const materialBorder = root.getPropertyValue("--forge-material-border").trim();
      const colorProbe = document.createElement("span");
      document.body.append(colorProbe);
      colorProbe.style.color = materialBorder;
      const materialBorderColor = getComputedStyle(colorProbe).color;
      colorProbe.remove();
      const materialBorderFocus = root.getPropertyValue("--forge-material-border-focus").trim();
      const materialSurface = root.getPropertyValue("--forge-material-surface").trim();
      const materialSurfaceFocus = root.getPropertyValue("--forge-material-surface-focus").trim();
      const materialRaised = root.getPropertyValue("--forge-material-raised").trim();
      const materialPopover = root.getPropertyValue("--forge-material-popover").trim();
      const materialOverlay = root.getPropertyValue("--forge-material-overlay").trim();
      const materialShadow = root.getPropertyValue("--forge-material-shadow").trim();
      const materialShadowStrong = root.getPropertyValue("--forge-material-shadow-strong").trim();
      const composerSurfaceFocus = root.getPropertyValue("--forge-composer-surface-focus").trim();
      const composerShadowFocus = root.getPropertyValue("--forge-composer-shadow-focus").trim();
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const detail = document.querySelector<HTMLElement>("[data-testid='log-detail-surface']");
      const menu = document.querySelector<HTMLElement>(".forge-composer-model-menu");
      const archive = document.querySelector<HTMLElement>("[data-testid='project-archive-panel']");
      const archiveHeader = archive?.querySelector<HTMLElement>(".forge-inspector-header");
      if (!composer || !detail || !menu || !archive || !archiveHeader) return null;
      const composerStyle = getComputedStyle(composer);
      const detailStyle = getComputedStyle(detail);
      const menuStyle = getComputedStyle(menu);
      const archiveStyle = getComputedStyle(archive);
      const archiveHeaderStyle = getComputedStyle(archiveHeader);
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
        archiveBorder: archiveStyle.borderLeftColor,
        archiveBackground: archiveStyle.backgroundColor,
        archiveHeaderHeight: Math.round(archiveHeader.getBoundingClientRect().height),
        archiveHeaderBorder: archiveHeaderStyle.borderBottomColor,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.composerBorder).toBe(metrics!.materialBorderFocus);
    expect(metrics!.composerBackground).toBe(metrics!.composerSurfaceFocus);
    expect(metrics!.materialShadowStrong).toContain("0 4px 14px");
    expect(metrics!.composerShadowFocus).toContain("0 4px 14px");
    expect(metrics!.composerShadow).toContain("4px 14px");
    expect(metrics!.detailBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.detailBackground).toBe(metrics!.materialRaised);
    expect(metrics!.menuBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.menuBackground).toBe(metrics!.materialPopover);
    expect(metrics!.archiveBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.archiveBackground).toBe(metrics!.materialOverlay);
    expect(metrics!.archiveHeaderHeight).toBe(42);
    expect(metrics!.archiveHeaderBorder).toBe(metrics!.materialBorderColor);
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

    await page.getByTitle("打开项目档案").click();
    const archive = projectArchive(page);
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();

    const radii = await page.evaluate(() => {
      const composer = document.querySelector("[data-testid='composer-lane'] > div:last-child");
      const archivePanel = [...document.querySelectorAll("aside:last-of-type section div")]
        .find((node) => node.textContent?.includes("项目概览"));
      return [composer, archivePanel]
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

    await expect(card).toHaveAttribute("data-confirm-state", "resolved");
    await expect(card.getByTestId("confirm-resolved-summary")).toBeVisible();
    await expect(card.getByText("已继续", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "继续" })).toHaveCount(0);
    await expect(card.getByRole("button", { name: "取消" })).toHaveCount(0);
    await expect(card.getByTestId("confirm-boundary-grid")).toHaveCount(0);
    await expect(card.getByTestId("confirm-warning")).toHaveCount(0);
    await expect(card.getByTestId("confirm-action-bar")).toHaveCount(0);

    const metrics = await card.evaluate((node) => {
      const style = getComputedStyle(node);
      const header = node.querySelector<HTMLElement>(".forge-message-panel-header");
      const summary = node.querySelector<HTMLElement>("[data-testid='confirm-resolved-summary']");
      const status = node.querySelector<HTMLElement>(".forge-confirm-resolved");
      return {
        height: Math.round(node.getBoundingClientRect().height),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        headerHeight: header ? Math.round(header.getBoundingClientRect().height) : 0,
        summaryHeight: summary ? Math.round(summary.getBoundingClientRect().height) : 0,
        summaryDisplay: summary ? getComputedStyle(summary).display : "",
        statusHeight: status ? Math.round(status.getBoundingClientRect().height) : 0,
      };
    });

    expect(metrics.height).toBeLessThan(pendingHeight - 80);
    expect(metrics.height).toBeLessThanOrEqual(68);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.borderColor).not.toBe("rgba(184, 138, 86, 0.22)");
    expect(metrics.headerHeight).toBeLessThanOrEqual(32);
    expect(metrics.summaryHeight).toBeLessThanOrEqual(30);
    expect(metrics.summaryDisplay).toBe("flex");
    expect(metrics.statusHeight).toBeLessThanOrEqual(20);
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

  test("pending project record delivery opens project archive", async ({ page }) => {
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

    const card = page.getByTestId("message-panel").filter({ hasText: "本轮交付" });
    await expect(card.getByText("自动记录")).toBeVisible();
    await card.getByRole("button", { name: "查看记录" }).click();

    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    await expect(projectArchive(page).getByTestId("archive-disclosure-records").getByRole("button", { name: /项目记录/ }).first()).toHaveAttribute("aria-expanded", "true");
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
      return {
        openState: (panel as HTMLElement).dataset.diffOpen,
        cardWidth: Math.round(panel.getBoundingClientRect().width),
        cardBackground: cardStyle.backgroundColor,
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
    expect(metrics!.cardBackground).toBe("rgba(42, 39, 33, 0.94)");
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
    const projectPath = "/Users/cabbos/project/forge-test-app";
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

    const group = page.getByTestId("tool-activity-group");
    await expect(group).toHaveCount(1);
    await expect(group.getByTestId("tool-activity-summary")).toHaveAttribute("aria-expanded", "true");
    await expect(group.getByText("处理遇到问题")).toBeVisible();
    await expect(group.getByText("3 步")).toBeVisible();
    await expect(group.getByText("已读取文件")).toBeVisible();
    await expect(group.getByTestId("shell-exit-code")).toHaveText("exit 1");
    await expect(group.getByTestId("tool-result-summary")).toContainText("权限不足");
    await expect(group.getByTestId("shell-output-section").filter({ hasText: "stderr" })).toContainText("Cannot find module");
    await expect(group.getByText("完成", { exact: true })).toHaveCount(0);

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

    const group = page.getByTestId("tool-activity-group");
    await expect(group).toHaveCount(1);
    const summary = group.getByTestId("tool-activity-summary");
    await expect(summary).toBeVisible();
    await expect(summary).toHaveAttribute("aria-expanded", "false");
    await expect(summary).toContainText("过程已收起 · 3 步");
    await expect(summary).toContainText("查看 1 个文件");
    await expect(summary).toContainText("运行 1 次检查");
    await expect(group.getByText("过程证据")).toHaveCount(0);
    await expect(group.getByText("已读取文件")).toHaveCount(0);
    await expect(group.getByText("npm run build")).toHaveCount(0);

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
    await expect(group.getByText("已读取文件")).toBeVisible();
    await expect(group.getByText("npm run build")).toBeVisible();
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

    const pending = page.getByTestId("pending-block");
    await expect(pending).toHaveText(/正在组织回答/);
    await expect(pending).toHaveAttribute("role", "status");
    await expect(pending).toHaveAttribute("aria-live", "polite");
    await expect(pending).toHaveAttribute("data-state", "running");
    await expect(pending.getByTestId("pending-dots")).toBeVisible();
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

    const thinking = page.getByTestId("thinking-trigger");
    await expect(thinking).toHaveText(/正在梳理思路/);
    await expect(thinking).toHaveAttribute("data-state", "running");
    await expect(thinking.getByTestId("thinking-dots")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']");
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
    expect(pendingMetrics.minHeight).toBe(22);
    expect(metrics!.thinkingMinHeight).toBe(22);
    expect(pendingMetrics.height).toBeGreaterThanOrEqual(22);
    expect(pendingMetrics.height).toBeLessThanOrEqual(24);
    expect(metrics!.thinkingHeight).toBeGreaterThanOrEqual(22);
    expect(metrics!.thinkingHeight).toBeLessThanOrEqual(24);
    expect(pendingMetrics.fontSize).toBeCloseTo(10.5);
    expect(metrics!.thinkingFontSize).toBeCloseTo(10.5);
    expect(pendingMetrics.gap).toBe(6);
    expect(metrics!.thinkingGap).toBe(6);
    expect(pendingMetrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.thinkingBackground).toBe("rgba(0, 0, 0, 0)");
    expect(pendingMetrics.borderTop).toBe(0);
    expect(metrics!.thinkingBorderTop).toBe(0);
    expect(pendingMetrics.color).toBe(metrics!.thinkingColor);
    await releaseHeldSendInput(page);
  });

  test("thinking block expands and shows content", async ({ page }) => {
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
    const thinkingTrigger = page.getByRole("button", { name: /思考已收起/ });
    await expect(thinkingTrigger).toBeVisible({ timeout: 5000 });

    // Click to expand
    await thinkingTrigger.click();

    // Thinking content should be visible
    await expect(page.getByText("I need to analyze the auth system first.")).toBeVisible();
  });

  test("tool card shows running then done status", async ({ page }) => {
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

    // Should show running status
    const runningTool = page.getByRole("button", { name: /正在读取文件/ });
    await expect(runningTool).toBeVisible({ timeout: 3000 });
    await expect(runningTool).toHaveAttribute("data-state", "running");
    await expect(page.getByText("进行中", { exact: true })).toHaveCount(0);

    // Send tool_result (done state)
    await simulateStream(page, sessionId, [
      { event_type: "tool_call_result", session_id: sessionId, block_id: toolId, result: "fn main() {}", is_error: false, duration_ms: 100 },
    ], 30);

    // Should show done
    const doneTool = page.getByRole("button", { name: /已读取文件/ });
    await expect(doneTool).toBeVisible({ timeout: 3000 });
    await expect(doneTool).toHaveAttribute("data-state", "done");
    await expect(doneTool).toContainText("100ms");
    await expect(page.getByText("完成", { exact: true })).toHaveCount(0);
  });

  test("shell card exposes a restrained running state before exit", async ({ page }) => {
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

    const runningShell = page.getByTestId("shell-card-trigger");
    await expect(runningShell).toBeVisible({ timeout: 3000 });
    await expect(runningShell).toHaveAttribute("data-state", "running");
    const runningMetrics = await runningShell.evaluate((node) => {
      const status = node.querySelector<HTMLElement>(".forge-log-status");
      const style = getComputedStyle(node);
      const statusStyle = status ? getComputedStyle(status) : null;
      return {
        minHeight: Math.round(Number.parseFloat(getComputedStyle(node).minHeight)),
        background: style.backgroundColor,
        borderTop: Math.round(Number.parseFloat(style.borderTopWidth)),
        statusTone: status?.getAttribute("data-tone") ?? "",
        statusTitle: status?.getAttribute("title") ?? "",
        statusColor: statusStyle?.color ?? "",
      };
    });
    expect(runningMetrics.minHeight).toBe(44);
    expect(runningMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(runningMetrics.borderTop).toBe(0);
    expect(runningMetrics.statusTone).toBe("running");
    expect(runningMetrics.statusTitle).toBe("运行中");
    expect(runningMetrics.statusColor).not.toBe("rgb(184, 138, 86)");

    await simulateStream(page, sessionId, [
      { event_type: "shell_end", session_id: sessionId, block_id: shellId, exit_code: 0 },
    ], 30);

    await expect(runningShell).toHaveAttribute("data-state", "done");
    const doneTone = await runningShell.evaluate((node) =>
      node.querySelector<HTMLElement>(".forge-log-status")?.getAttribute("data-tone"),
    );
    expect(doneTone).toBe("success");
  });
