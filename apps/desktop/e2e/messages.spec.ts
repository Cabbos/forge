import { test, expect } from "@playwright/test";
import { resolve } from "node:path";
import { setup } from "./fixtures/app";
import { simulateStream, fullConversation } from "./mock-ipc";

async function forceDarkWorkbench(page: import("@playwright/test").Page) {
  await page.addInitScript(() => {
    const apply = () => {
      document.querySelectorAll<HTMLElement>("[data-conversation-theme='light']").forEach((el) => {
        el.setAttribute("data-conversation-theme", "dark");
      });
      document.querySelectorAll<HTMLElement>(".forge-app-shell[data-design-version='v3-light-workbench']").forEach((el) => {
        el.setAttribute("data-design-version", "v3-dark-workbench");
      });
    };

    new MutationObserver(apply).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-conversation-theme", "data-design-version"],
      childList: true,
      subtree: true,
    });
    window.addEventListener("DOMContentLoaded", apply);
    apply();
  });
}

test.beforeEach(async ({ page }) => {
  await setup(page);
  await forceDarkWorkbench(page);
  await page.goto("http://localhost:1420");
  await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
});

  test("internal skill context is not rendered in the conversation", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "internal-skills" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "internal-skills",
        content: "## Active Skills\n\n- code-review\n- browser",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "internal-skills" },
    ], 5);

    await expect(page.getByRole("main").getByText("Active Skills")).toHaveCount(0);
    await expect(page.getByRole("main").getByText("code-review")).toHaveCount(0);
  });

  test("same delivery summary after a new user message appends a new card", async ({ page }) => {
    const sessionId = "same-summary-new-turn";
    const projectPath = "/Users/cabbos/project/forge";
    const summary = {
      project_path: projectPath,
      preview_label: "预览未运行",
      checkpoint_label: "检查点已就绪",
      next_action: "下一步：交付状态可以继续验收。",
    };
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, projectPath, summary }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: projectPath, name: "forge", path: projectPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(projectPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: projectPath,
          workspaceId: projectPath,
          contextWindowTokens: 1_000_000,
          status: "running",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "first-turn-user",
          event_type: "user_message",
          content: "第一轮",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "first-turn-delivery",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: { summary },
        },
        {
          block_id: "second-turn-user",
          event_type: "user_message",
          content: "第二轮",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, projectPath, summary });

    await page.reload();
    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(1);
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "second-turn-delivery",
        summary,
      },
    ], 1);

    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(2);
  });

  test("provider usage renders as trace metadata instead of assistant prose", async ({ page }) => {
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
        event_type: "provider_usage",
        session_id: sessionId,
        block_id: "usage-visible-placement",
        provider_id: "deepseek",
        model: "deepseek-v4-flash[1m]",
        source: "anthropic",
        reason: "provider_reported",
        input_tokens: 411,
        output_tokens: 137,
        cache_read_tokens: null,
        cache_creation_tokens: null,
        reasoning_tokens: null,
        estimated_cost_micros: 96,
        pricing_source: "forge_static_pricing_2026_06_20",
      },
    ], 1);

    const usageCard = page.getByTestId("provider-usage-card");
    await expect(usageCard).toBeVisible();
    await expect(usageCard).toContainText("deepseek-v4-flash[1m]");
    await expect(usageCard).toContainText("输入 411");
    await expect(usageCard).toContainText("输出 137");
    await expect(usageCard).toContainText("96 micros");
    await expect(page.getByTestId("assistant-message").filter({ hasText: "模型用量 · provider" })).toHaveCount(0);

    const blockRole = await usageCard.evaluate((node) => {
      return node.closest("[data-testid='message-block']")?.getAttribute("data-block-role");
    });
    expect(blockRole).toBe("trace");

    await simulateStream(page, sessionId, [
      {
        event_type: "provider_usage",
        session_id: sessionId,
        block_id: "usage-unknown-placement",
        provider_id: "deepseek",
        model: "deepseek-v4-flash[1m]",
        source: "anthropic",
        reason: "provider_omitted",
        input_tokens: null,
        output_tokens: null,
        cache_read_tokens: null,
        cache_creation_tokens: null,
        reasoning_tokens: null,
        estimated_cost_micros: null,
        pricing_source: null,
      },
    ], 1);

    const unknownUsageCard = page.getByTestId("provider-usage-card").last();
    await expect(unknownUsageCard).toContainText("输入 unknown");
    await expect(unknownUsageCard).toContainText("输出 unknown");
    await expect(unknownUsageCard).toContainText("费用 unknown");
    await expect(unknownUsageCard).toContainText("用量未知");
  });

  test("timeline messages render correctly", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    // Create session
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    // Simulate a full conversation
    const events = fullConversation(sessionId);
    await simulateStream(page, sessionId, events, 30);

    await expect(page.getByRole("button", { name: /思考已收起/ })).toBeVisible({ timeout: 5000 });
    await expect(page.getByText("I'll create a fibonacci function.")).toBeVisible();

    const processSummary = page.getByTestId("tool-activity-summary");
    await expect(processSummary).toBeVisible({ timeout: 5000 });
    await expect(processSummary).toContainText("过程已收起 · 2 步");
    await processSummary.click();

    // Tool card should show write_to_file after expanding handled work.
    await expect(page.getByTestId("tool-card-trigger").filter({ hasText: "write_to_file" })).toBeVisible({ timeout: 5000 });

    // Shell card should show terminal output
    await expect(page.locator("text=python test.py")).toBeVisible();

    // Final text should be visible
    await expect(page.locator("text=The fibonacci function works correctly")).toBeVisible();
  });

  test("conversation area uses a compact centered prose lane", async ({ page }) => {
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

    await page.locator("textarea").fill("把这个页面整理得更像正式产品。");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "style-assistant-message" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "style-assistant-message",
        content: "可以。先把默认对话区收成一条安静的阅读栏，再处理行动卡片。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "style-assistant-message" },
    ], 10);

    const lane = page.getByTestId("message-lane");
    await expect(lane).toBeVisible();
    await expect(page.getByText("你的请求")).toHaveCount(0);

    const laneWidth = await lane.evaluate((node) => Math.round(node.getBoundingClientRect().width));
    expect(laneWidth).toBeLessThanOrEqual(860);

    const userMessage = page.getByTestId("user-message").last();
    const userMaterial = await userMessage.evaluate((node) => {
      const style = getComputedStyle(node);
      const alphaMatch = style.backgroundColor.match(/rgba?\(([^)]+)\)/);
      const channels = alphaMatch ? alphaMatch[1].split(",").map((part) => Number.parseFloat(part.trim())) : [];
      return {
        borderTopWidth: style.borderTopWidth,
        radius: Number.parseFloat(style.borderTopLeftRadius),
        backgroundAlpha: channels.length === 4 ? channels[3] : 1,
        boxShadow: style.boxShadow,
        transform: style.transform,
        before: getComputedStyle(node, "::before").content,
        after: getComputedStyle(node, "::after").content,
      };
    });
    expect(userMaterial.borderTopWidth).toBe("1px");
    expect(userMaterial.radius).toBeLessThanOrEqual(8);
    expect(userMaterial.backgroundAlpha).toBeGreaterThanOrEqual(0.9);
    expect(userMaterial.backgroundAlpha).toBeLessThanOrEqual(1);
    expect(userMaterial.boxShadow).toBe("none");
    expect(userMaterial.transform).toBe("none");
    expect(userMaterial.before).toBe("none");
    expect(userMaterial.after).toBe("none");
    await expect(page.getByTestId("assistant-message").last()).toHaveCSS("border-top-width", "0px");

    const laneAlignment = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const user = document.querySelector<HTMLElement>("[data-testid='user-message']");
      const assistant = document.querySelector<HTMLElement>("[data-testid='assistant-message']");
      if (!lane || !user || !assistant) return null;
      const laneRect = lane.getBoundingClientRect();
      const userRect = user.getBoundingClientRect();
      const assistantRect = assistant.getBoundingClientRect();
      return {
        assistantLeft: Math.round(assistantRect.left),
        userLeft: Math.round(userRect.left),
        userRight: Math.round(userRect.right),
        laneRight: Math.round(laneRect.right),
      };
    });
    expect(laneAlignment).not.toBeNull();
    expect(laneAlignment!.userLeft).toBeGreaterThan(laneAlignment!.assistantLeft);
    expect(laneAlignment!.userRight).toBeLessThanOrEqual(laneAlignment!.laneRight);
    expect(laneAlignment!.laneRight - laneAlignment!.userRight).toBeLessThanOrEqual(4);
  });

  test("scroll-to-bottom control stays outside the centered reading lane", async ({ page }) => {
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
      { event_type: "text_start" as const, session_id: sessionId, block_id: `scroll-lane-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `scroll-lane-${index}`,
        content: `第 ${index + 1} 条输出，用来撑开滚动区域。这里保持足够长度，让对话区出现滚动。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `scroll-lane-${index}` },
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

    const placement = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const button = document.querySelector<HTMLElement>("[data-testid='scroll-to-bottom']");
      if (!lane || !button) return null;
      const laneRect = lane.getBoundingClientRect();
      const buttonRect = button.getBoundingClientRect();
      return {
        laneLeft: Math.round(laneRect.left),
        laneRight: Math.round(laneRect.right),
        buttonLeft: Math.round(buttonRect.left),
        buttonRight: Math.round(buttonRect.right),
      };
    });

    expect(placement).not.toBeNull();
    expect(
      placement!.buttonLeft >= placement!.laneRight + 8 ||
      placement!.buttonRight <= placement!.laneLeft - 8,
    ).toBe(true);
  });

  test("streaming output keeps bottom lock without stealing manual scroll", async ({ page }) => {
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

    const filler = Array.from({ length: 32 }, (_, index) => ([
      { event_type: "text_start" as const, session_id: sessionId, block_id: `stream-fill-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `stream-fill-${index}`,
        content: `第 ${index + 1} 条历史输出，用来撑开真实滚动区域。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `stream-fill-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, filler, 1);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.scrollTop = scroller.scrollHeight;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "live-stream" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "正在整理第一段。" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "\n继续补充第二段，让输出变高。" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "\n最后收尾。" },
    ], 20);

    const bottomDistance = await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return null;
      return Math.round(scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight);
    });
    expect(bottomDistance).not.toBeNull();
    expect(bottomDistance!).toBeLessThanOrEqual(2);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "\n用户在上方阅读时继续输出。" },
      { event_type: "text_end", session_id: sessionId, block_id: "live-stream" },
    ], 20);

    const manualScrollTop = await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      return scroller ? Math.round(scroller.scrollTop) : null;
    });
    expect(manualScrollTop).toBe(0);
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();
  });

  test("streaming chunks update quickly enough to feel live", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "live-cadence" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-cadence", content: "第一段" },
    ], 1);
    await expect(page.getByTestId("assistant-message").last()).toContainText("第一段");

    await simulateStream(page, sessionId, [
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-cadence", content: "\n第二段" },
    ], 1);

    await expect(page.getByTestId("assistant-message").last()).toContainText("第二段", { timeout: 150 });
  });

  test("composer internals use editor rhythm tokens", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const rhythm = await page.evaluate(() => {
      const root = document.documentElement;
      const textareaWrap = document.querySelector("[data-testid='composer-textarea-wrap']");
      const toolbar = document.querySelector("[data-testid='composer-toolbar']");
      const textarea = document.querySelector("textarea");
      if (!textareaWrap || !toolbar || !textarea) return null;
      const textareaWrapStyle = getComputedStyle(textareaWrap);
      const toolbarStyle = getComputedStyle(toolbar);
      const textareaStyle = getComputedStyle(textarea);

      return {
        innerX: getComputedStyle(root).getPropertyValue("--forge-composer-inner-x").trim(),
        innerY: getComputedStyle(root).getPropertyValue("--forge-composer-inner-y").trim(),
        textPadLeft: Math.round(Number.parseFloat(textareaWrapStyle.paddingLeft)),
        textPadTop: Math.round(Number.parseFloat(textareaWrapStyle.paddingTop)),
        toolbarPadLeft: Math.round(Number.parseFloat(toolbarStyle.paddingLeft)),
        toolbarPadBottom: Math.round(Number.parseFloat(toolbarStyle.paddingBottom)),
        textareaLineHeight: Math.round(Number.parseFloat(textareaStyle.lineHeight)),
        textareaScrollbarWidth: textareaStyle.scrollbarWidth,
      };
    });

    expect(rhythm).not.toBeNull();
    expect(rhythm!.innerX).toBe("18px");
    expect(rhythm!.innerY).toBe("16px");
    expect(rhythm!.textPadLeft).toBe(18);
    expect(rhythm!.textPadTop).toBe(16);
    expect(rhythm!.toolbarPadLeft).toBe(18);
    expect(rhythm!.toolbarPadBottom).toBe(8);
    expect(rhythm!.textareaLineHeight).toBe(24);
    expect(rhythm!.textareaScrollbarWidth).toBe("thin");
  });

  test("assistant prose and user bubbles share readable message primitives", async ({ page }) => {
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

    await page.locator("textarea").fill("整理这个页面");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "primitive-text" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "primitive-text",
        content: "可以。先把可读性收稳，再继续压低 UI 噪音。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "primitive-text" },
    ], 1);

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const assistant = document.querySelector("[data-testid='assistant-message']");
      const user = document.querySelector("[data-testid='user-message']");
      if (!assistant || !user) return null;
      const assistantStyle = getComputedStyle(assistant);
      const userStyle = getComputedStyle(user);
      return {
        assistantLineToken: getComputedStyle(root).getPropertyValue("--forge-assistant-line-height").trim(),
        userLineToken: getComputedStyle(root).getPropertyValue("--forge-user-line-height").trim(),
        assistantLineHeight: Math.round(Number.parseFloat(assistantStyle.lineHeight)),
        userLineHeight: Math.round(Number.parseFloat(userStyle.lineHeight)),
        userShadow: userStyle.boxShadow,
        userRadius: Number.parseFloat(userStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.assistantLineToken).toBe("25px");
    expect(metrics!.userLineToken).toBe("22px");
    expect(metrics!.assistantLineHeight).toBe(25);
    expect(metrics!.userLineHeight).toBe(22);
    expect(metrics!.userShadow).toBe("none");
    expect(metrics!.userRadius).toBeLessThanOrEqual(8);
  });

  test("assistant and user messages expose quiet copy actions", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error test clipboard capture
      window.__clipboardText = "";
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async (text: string) => {
            // @ts-expect-error test clipboard capture
            window.__clipboardText = text;
          },
        },
      });
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const prompt = "请看 `src/App.tsx:1`，然后总结一下。";
    await page.locator("textarea").fill(prompt);
    await page.locator("textarea").press("Enter");

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "copyable-reply" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "copyable-reply",
        content: "## 结论\n\n这个改动可以继续推进。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "copyable-reply" },
    ], 1);

    const userMessage = page.getByTestId("user-message").last();
    const assistantMessage = page.getByTestId("assistant-message").last();
    const userCopy = userMessage.getByRole("button", { name: "复制提问" });
    const assistantCopy = assistantMessage.getByRole("button", { name: "复制回复" });
    const assistantOpacityBeforeHover = await assistantCopy.evaluate((action) => Number.parseFloat(getComputedStyle(action).opacity));
    expect(assistantOpacityBeforeHover).toBe(0);

    await userMessage.hover();
    await expect(userCopy).toBeVisible();
    await userCopy.click();
    await expect(userCopy).toHaveAttribute("aria-label", "已复制提问");
    await expect(page.evaluate(() => {
      // @ts-expect-error test clipboard capture
      return window.__clipboardText;
    })).resolves.toBe(prompt);

    await assistantMessage.hover();
    await expect(assistantCopy).toBeVisible();
    await assistantCopy.click();
    await expect(assistantCopy).toHaveAttribute("aria-label", "已复制回复");
    await expect(page.evaluate(() => {
      // @ts-expect-error test clipboard capture
      return window.__clipboardText;
    })).resolves.toContain("## 结论");

    const metrics = await page.evaluate(() => {
      const action = document.querySelector<HTMLElement>("[data-testid='assistant-message'] [data-testid='message-copy-action']");
      if (!action) return null;
      const style = getComputedStyle(action);
      const styleWithWebkit = style as CSSStyleDeclaration & { webkitBackdropFilter?: string };
      return {
        width: Math.round(Number.parseFloat(style.width)),
        height: Math.round(Number.parseFloat(style.height)),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        position: style.position,
        top: Math.round(Number.parseFloat(style.top)),
        right: Math.round(Number.parseFloat(style.right)),
        background: style.backgroundColor,
        boxShadow: style.boxShadow,
        backdropFilter: style.backdropFilter || styleWithWebkit.webkitBackdropFilter || "",
        transform: style.transform,
        transitionProperty: style.transitionProperty,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.width).toBe(26);
    expect(metrics!.height).toBe(26);
    expect(metrics!.radius).toBeLessThanOrEqual(8);
    expect(metrics!.position).toBe("absolute");
    expect(metrics!.top).toBeGreaterThanOrEqual(0);
    expect(metrics!.top).toBeLessThanOrEqual(4);
    expect(metrics!.right).toBeGreaterThanOrEqual(0);
    expect(metrics!.right).toBeLessThanOrEqual(4);
    expect(metrics!.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.boxShadow).not.toBe("none");
    expect(metrics!.backdropFilter).toBe("none");
    expect(metrics!.transform).not.toBe("none");
    expect(metrics!.transitionProperty).toContain("transform");
  });

  test("assistant markdown uses a compact editorial rhythm", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "markdown-rhythm" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "markdown-rhythm",
        content: [
          "先把阅读节奏收稳。",
          "",
          "## 排版目标",
          "",
          "- 文字要安静",
          "- 层级要清楚",
          "",
          "> 过程信息可以轻，结论必须清楚。",
          "",
          "---",
          "",
          "使用 `npm run build` 作为最小验证。",
          "",
          "| 项目 | 状态 |",
          "| --- | --- |",
          "| 预览 | 可用 |",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "markdown-rhythm" },
    ], 1);

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const assistant = document.querySelector("[data-testid='assistant-message']");
      if (!assistant) return null;
      const assistantStyle = getComputedStyle(assistant);
      const paragraph = assistant.querySelector("p");
      const heading = assistant.querySelector("h2");
      const list = assistant.querySelector("ul");
      const listItem = assistant.querySelector("li");
      const quote = assistant.querySelector("blockquote");
      const rule = assistant.querySelector("hr");
      const inlineCode = assistant.querySelector("p code");
      const table = assistant.querySelector("table");
      const tableCell = assistant.querySelector("td");
      if (!paragraph || !heading || !list || !listItem || !quote || !rule || !inlineCode || !table || !tableCell) return null;
      const paragraphStyle = getComputedStyle(paragraph);
      const headingStyle = getComputedStyle(heading);
      const listStyle = getComputedStyle(list);
      const listItemStyle = getComputedStyle(listItem);
      const quoteStyle = getComputedStyle(quote);
      const ruleStyle = getComputedStyle(rule);
      const codeStyle = getComputedStyle(inlineCode);
      const tableStyle = getComputedStyle(table);
      const cellStyle = getComputedStyle(tableCell);

      return {
        paragraphGapToken: getComputedStyle(root).getPropertyValue("--forge-markdown-paragraph-gap").trim(),
        blockGapToken: getComputedStyle(root).getPropertyValue("--forge-markdown-block-gap").trim(),
        assistantMaxWidth: assistantStyle.maxWidth,
        assistantWidth: Math.round(assistant.getBoundingClientRect().width),
        assistantPaddingRight: Math.round(Number.parseFloat(assistantStyle.paddingRight)),
        assistantOverflowWrap: assistantStyle.overflowWrap,
        paragraphMarginBottom: Math.round(Number.parseFloat(paragraphStyle.marginBottom)),
        headingFontSize: Math.round(Number.parseFloat(headingStyle.fontSize)),
        headingLineHeight: Math.round(Number.parseFloat(headingStyle.lineHeight)),
        headingMarginTop: Math.round(Number.parseFloat(headingStyle.marginTop)),
        listPaddingLeft: Math.round(Number.parseFloat(listStyle.paddingLeft)),
        listItemMarginBottom: Math.round(Number.parseFloat(listItemStyle.marginBottom)),
        quoteBorderWidth: Math.round(Number.parseFloat(quoteStyle.borderLeftWidth)),
        quoteBorderTopWidth: Math.round(Number.parseFloat(quoteStyle.borderTopWidth)),
        quoteBorderColor: quoteStyle.borderLeftColor,
        quoteBorderTopColor: quoteStyle.borderTopColor,
        quoteBackground: quoteStyle.backgroundColor,
        quoteRadius: Number.parseFloat(quoteStyle.borderTopLeftRadius),
        quotePaddingLeft: Math.round(Number.parseFloat(quoteStyle.paddingLeft)),
        ruleHeight: Math.round(rule.getBoundingClientRect().height),
        ruleMarginTop: Math.round(Number.parseFloat(ruleStyle.marginTop)),
        ruleBackground: ruleStyle.backgroundColor,
        codeBackground: codeStyle.backgroundColor,
        codePaddingLeft: Math.round(Number.parseFloat(codeStyle.paddingLeft)),
        tableDisplay: tableStyle.display,
        tableBackground: tableStyle.backgroundColor,
        tableMarginTop: Math.round(Number.parseFloat(tableStyle.marginTop)),
        tableMaxWidth: tableStyle.maxWidth,
        tableOverflowX: tableStyle.overflowX,
        tableScrollbarWidth: tableStyle.scrollbarWidth,
        tableRadius: Math.round(Number.parseFloat(tableStyle.borderTopLeftRadius)),
        cellPaddingTop: Math.round(Number.parseFloat(cellStyle.paddingTop)),
        cellBorderRightWidth: Math.round(Number.parseFloat(cellStyle.borderRightWidth)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.paragraphGapToken).toBe("9px");
    expect(metrics!.blockGapToken).toBe("14px");
    expect(metrics!.assistantMaxWidth).not.toBe("none");
    expect(metrics!.assistantWidth).toBeLessThanOrEqual(760);
    expect(metrics!.assistantPaddingRight).toBeGreaterThanOrEqual(16);
    expect(metrics!.assistantOverflowWrap).toBe("anywhere");
    expect(metrics!.paragraphMarginBottom).toBe(14);
    expect(metrics!.headingFontSize).toBe(15);
    expect(metrics!.headingLineHeight).toBe(23);
    expect(metrics!.headingMarginTop).toBe(18);
    expect(metrics!.listPaddingLeft).toBe(20);
    expect(metrics!.listItemMarginBottom).toBe(2);
    expect(metrics!.quoteBorderWidth).toBe(1);
    expect(metrics!.quoteBorderTopWidth).toBe(1);
    expect(metrics!.quoteBorderColor).toBe(metrics!.quoteBorderTopColor);
    expect(metrics!.quoteBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.quoteRadius).toBeLessThanOrEqual(8);
    expect(metrics!.quotePaddingLeft).toBe(12);
    expect(metrics!.ruleHeight).toBe(1);
    expect(metrics!.ruleMarginTop).toBe(14);
    expect(metrics!.ruleBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.codeBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.codePaddingLeft).toBeGreaterThanOrEqual(4);
    expect(metrics!.tableDisplay).toBe("block");
    expect(metrics!.tableBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.tableMarginTop).toBe(14);
    expect(metrics!.tableMaxWidth).toBe("100%");
    expect(metrics!.tableOverflowX).toBe("auto");
    expect(metrics!.tableScrollbarWidth).toBe("thin");
    expect(metrics!.tableRadius).toBe(8);
    expect(metrics!.cellPaddingTop).toBe(8);
    expect(metrics!.cellBorderRightWidth).toBe(0);
  });

  test("markdown tables fit their content before falling back to horizontal scroll", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "table-compat" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "table-compat",
        content: [
          "这里有一张短表和一张很宽的表。",
          "",
          "| 文件 | 状态 |",
          "| --- | --- |",
          "| `src/filter.test.ts` | 可复用 |",
          "",
          "| 扩展点 | 机制 | 示例 | 备注 |",
          "| --- | --- | --- | --- |",
          "| 自定义 Agent | 项目目录放 .md 文件 | my-project/.claude/agents/reviewer-with-a-very-long-name.md | 这列故意很长用来验证横向滚动不会撑破消息栏 |",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "table-compat" },
    ], 1);

    const metrics = await page.evaluate(() => {
      const assistant = document.querySelector<HTMLElement>("[data-testid='assistant-message']");
      const tables = Array.from(document.querySelectorAll<HTMLElement>("[data-testid='assistant-message'] table"));
      if (!assistant || tables.length < 2) return null;
      const [compact, wide] = tables;
      const compactRect = compact.getBoundingClientRect();
      const wideRect = wide.getBoundingClientRect();
      const assistantRect = assistant.getBoundingClientRect();
      const compactCode = compact.querySelector<HTMLElement>(".forge-inline-code");
      const compactCodeRect = compactCode?.getBoundingClientRect();
      const compactCodeStyle = compactCode ? getComputedStyle(compactCode) : null;
      return {
        assistantWidth: Math.round(assistantRect.width),
        compactWidth: Math.round(compactRect.width),
        wideWidth: Math.round(wideRect.width),
        wideClientWidth: Math.round(wide.clientWidth),
        wideScrollWidth: Math.round(wide.scrollWidth),
        compactOverflowX: getComputedStyle(compact).overflowX,
        wideOverflowX: getComputedStyle(wide).overflowX,
        compactCodeHeight: compactCodeRect ? Math.round(compactCodeRect.height) : 0,
        compactCodeWidth: compactCodeRect ? Math.round(compactCodeRect.width) : 0,
        compactCodeOverflowWrap: compactCodeStyle?.overflowWrap ?? "",
        compactCodeWhiteSpace: compactCodeStyle?.whiteSpace ?? "",
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.compactWidth).toBeLessThan(metrics!.assistantWidth - 120);
    expect(metrics!.compactWidth).toBeLessThanOrEqual(360);
    expect(metrics!.wideWidth).toBeLessThanOrEqual(metrics!.assistantWidth);
    expect(metrics!.wideScrollWidth).toBeGreaterThan(metrics!.wideClientWidth);
    expect(metrics!.compactOverflowX).toBe("auto");
    expect(metrics!.wideOverflowX).toBe("auto");
    expect(metrics!.compactCodeWhiteSpace).toBe("nowrap");
    expect(metrics!.compactCodeOverflowWrap).toBe("normal");
    expect(metrics!.compactCodeWidth).toBeGreaterThan(100);
    expect(metrics!.compactCodeHeight).toBeLessThanOrEqual(26);
  });

  test("inline file references stay quiet and wrap within the message lane", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "inline-file-ref" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "inline-file-ref",
        content: [
          "可以先检查 `src/features/deep-context/components/ProjectArchiveInspectorReallyLongNameForWrap.tsx:128`，再看相邻渲染逻辑。",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "inline-file-ref" },
    ], 1);

    const assistant = page.getByTestId("assistant-message");
    const fileRef = assistant.locator(".forge-inline-code-file .forge-file-ref");
    await expect(fileRef.locator(".forge-file-ref-icon")).toBeVisible();
    await expect(fileRef.locator(".forge-file-ref-name")).toHaveText("ProjectArchiveInspectorReallyLongNameForWrap.tsx");
    await expect(fileRef.locator(".forge-file-ref-line")).toHaveText("line 128");
    await expect(fileRef).toHaveAttribute("title", "src/features/deep-context/components/ProjectArchiveInspectorReallyLongNameForWrap.tsx:128");
    await expect(fileRef).toHaveAttribute("aria-label", "打开 src/features/deep-context/components/ProjectArchiveInspectorReallyLongNameForWrap.tsx:128");

    const metrics = await assistant.evaluate((node) => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const token = node.querySelector<HTMLElement>(".forge-inline-code-file");
      const link = node.querySelector<HTMLElement>(".forge-inline-code-file .forge-file-ref");
      if (!lane || !token || !link) return null;
      const laneRect = lane.getBoundingClientRect();
      const tokenRect = token.getBoundingClientRect();
      const tokenStyle = getComputedStyle(token);
      const linkStyle = getComputedStyle(link);
      return {
        laneWidth: Math.round(laneRect.width),
        tokenWidth: Math.round(tokenRect.width),
        tokenRight: Math.round(tokenRect.right),
        laneRight: Math.round(laneRect.right),
        tokenOverflowWrap: tokenStyle.overflowWrap,
        tokenWordBreak: tokenStyle.wordBreak,
        linkTextDecoration: linkStyle.textDecorationLine,
        linkDisplay: linkStyle.display,
        linkGap: Math.round(Number.parseFloat(linkStyle.gap)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.tokenWidth).toBeLessThan(metrics!.laneWidth);
    expect(metrics!.tokenRight).toBeLessThanOrEqual(metrics!.laneRight);
    expect(metrics!.tokenOverflowWrap).toBe("anywhere");
    expect(metrics!.tokenWordBreak).toBe("normal");
    expect(metrics!.linkTextDecoration).toBe("none");
    expect(metrics!.linkDisplay).toBe("inline-flex");
    expect(metrics!.linkGap).toBeGreaterThanOrEqual(4);
  });

  test("conversation turns keep quiet separation without card framing", async ({ page }) => {
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

    await page.locator("textarea").fill("第一轮问题");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "turn-one" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "turn-one", content: "第一轮回复。" },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-one" },
    ], 1);

    await page.locator("textarea").fill("第二轮问题");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "turn-two" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "turn-two", content: "第二轮回复。" },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-two" },
    ], 1);

    await expect(page.getByTestId("conversation-turn")).toHaveCount(2);

    const metrics = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const turns = Array.from(document.querySelectorAll<HTMLElement>("[data-testid='conversation-turn']"));
      const first = turns[0];
      const second = turns[1];
      if (!lane || !first || !second) return null;
      const laneStyle = getComputedStyle(lane);
      const firstStyle = getComputedStyle(first);
      const turnStyle = getComputedStyle(second);
      const firstBlocks = Array.from(first.querySelectorAll<HTMLElement>("[data-testid='message-block']"));
      return {
        laneGap: Math.round(Number.parseFloat(laneStyle.rowGap)),
        firstTurnGap: Math.round(Number.parseFloat(firstStyle.rowGap)),
        firstRoles: firstBlocks.map((block) => block.dataset.blockRole),
        firstMargins: firstBlocks.map((block) => Math.round(Number.parseFloat(getComputedStyle(block).marginTop))),
        secondPaddingTop: Math.round(Number.parseFloat(turnStyle.paddingTop)),
        secondBorderRadius: Math.round(Number.parseFloat(turnStyle.borderTopLeftRadius)),
        secondBackground: turnStyle.backgroundColor,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.laneGap).toBe(0);
    expect(metrics!.firstTurnGap).toBe(0);
    expect(metrics!.firstRoles).toEqual(["user", "assistant"]);
    expect(metrics!.firstMargins).toEqual([0, 14]);
    expect(metrics!.secondPaddingTop).toBe(16);
    expect(metrics!.secondBorderRadius).toBe(0);
    expect(metrics!.secondBackground).toBe("rgba(0, 0, 0, 0)");
  });

  test("code blocks use a compact reader surface", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "code-rhythm" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "code-rhythm",
        content: [
          "可以先这样写：",
          "",
          "```ts",
          "export function sum(a: number, b: number) {",
          "  return a + b;",
          "}",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "code-rhythm" },
    ], 1);

    await expect(page.locator(".code-surface")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const surface = document.querySelector(".code-surface");
      const header = surface?.querySelector("figcaption");
      const label = surface?.querySelector("figcaption span:nth-child(2)");
      const code = surface?.querySelector(".shiki-wrapper .shiki code, .code-fallback code");
      if (!surface || !header || !label || !code) return null;
      const surfaceStyle = getComputedStyle(surface);
      const headerStyle = getComputedStyle(header);
      const labelStyle = getComputedStyle(label);
      const codeStyle = getComputedStyle(code);
      return {
        marginTop: Math.round(Number.parseFloat(surfaceStyle.marginTop)),
        marginBottom: Math.round(Number.parseFloat(surfaceStyle.marginBottom)),
        headerHeight: Math.round(header.getBoundingClientRect().height),
        headerBackground: headerStyle.backgroundColor,
        labelFontSize: Math.round(Number.parseFloat(labelStyle.fontSize)),
        codeLineHeight: Math.round(Number.parseFloat(codeStyle.lineHeight)),
        codeFontSize: Number.parseFloat(codeStyle.fontSize),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.marginTop).toBe(14);
    expect(metrics!.marginBottom).toBe(14);
    expect(metrics!.headerHeight).toBe(34);
    expect(metrics!.headerBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.labelFontSize).toBe(10);
    expect(metrics!.codeLineHeight).toBe(20);
    expect(metrics!.codeFontSize).toBeCloseTo(12.5);
  });

  test("reader surface caption actions share quiet desktop material", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "caption-actions" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "caption-actions",
        content: [
          "先看代码，再看架构图。",
          "",
          "```ts",
          "const stable = true;",
          "```",
          "",
          "```text",
          "┌─────────────┐",
          "│ Composer    │",
          "└──────┬──────┘",
          "       ▼",
          "┌─────────────┐",
          "│ Tool Row    │",
          "└─────────────┘",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "caption-actions" },
    ], 1);

    await expect(page.locator(".code-surface")).toBeVisible();
    await expect(page.getByTestId("diagram-surface")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const codeAction = document.querySelector<HTMLElement>(".code-caption button[aria-label='复制代码']");
      const diagramAction = document.querySelector<HTMLElement>(".diagram-caption button[aria-label='复制图示源码']");
      const actions = [codeAction, diagramAction].filter(Boolean) as HTMLElement[];
      if (actions.length !== 2) return null;
      return actions.map((action) => {
        const style = getComputedStyle(action);
        return {
          hasClass: action.classList.contains("forge-caption-action"),
          width: Math.round(action.getBoundingClientRect().width),
          height: Math.round(action.getBoundingClientRect().height),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          background: style.backgroundColor,
          border: style.borderTopColor,
          color: style.color,
          transitionProperty: style.transitionProperty,
        };
      });
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.every((item) => item.hasClass)).toBe(true);
    expect(metrics!.every((item) => item.width === 24 && item.height === 24)).toBe(true);
    expect(metrics!.every((item) => item.radius <= 8)).toBe(true);
    expect(metrics!.every((item) => item.background !== "rgba(0, 0, 0, 0)")).toBe(true);
    expect(metrics!.every((item) => item.border !== "rgba(0, 0, 0, 0)")).toBe(true);
    expect(metrics!.every((item) => item.color !== "rgb(184, 138, 86)")).toBe(true);
    expect(metrics!.every((item) => item.transitionProperty.includes("background-color"))).toBe(true);

    await page.getByRole("button", { name: "复制代码" }).hover();
    await expect(page.getByRole("button", { name: "复制代码" })).not.toHaveCSS("border-color", "rgba(0, 0, 0, 0)");
  });

  test("ascii architecture diagrams render as diagram surfaces instead of code blocks", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "ascii-diagram" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "ascii-diagram",
        content: [
          "架构图（简化）",
          "",
          "```text",
          "┌──────────────────────────────────────────┐",
          "│              主 LLM 循环                 │",
          "│  main.tsx -> query.ts -> Anthropic API    │",
          "└──────────────────────────────────────────┘",
          "                    │",
          "                    ▼",
          "┌───────────────────┬──────────────────────┐",
          "│ Agent 工具        │ Coordinator 模式      │",
          "│ 解析定义          │ 并行派发任务          │",
          "│ 返回结果          │ 合成结果              │",
          "└───────────────────┴──────────────────────┘",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "ascii-diagram" },
    ], 1);

    const diagram = page.getByTestId("diagram-surface");
    await expect(diagram).toBeVisible();
    await expect(diagram).toHaveAttribute("data-diagram-kind", "ascii");
    await expect(diagram.getByText("架构图", { exact: true })).toBeVisible();
    await expect(diagram.locator(".diagram-code")).toContainText("主 LLM 循环");
    await expect(page.locator(".code-surface")).toHaveCount(0);
    await expect(diagram.locator(".shiki-wrapper")).toHaveCount(0);

    const metrics = await diagram.evaluate((node) => {
      const viewport = node.querySelector<HTMLElement>("[data-testid='diagram-viewport']");
      const caption = node.querySelector<HTMLElement>(".diagram-caption");
      const code = node.querySelector<HTMLElement>(".diagram-code");
      const style = getComputedStyle(node);
      const captionStyle = caption ? getComputedStyle(caption) : null;
      const viewportStyle = viewport ? getComputedStyle(viewport) : null;
      const codeStyle = code ? getComputedStyle(code) : null;
      const rect = node.getBoundingClientRect();
      return {
        width: Math.round(rect.width),
        marginTop: Math.round(Number.parseFloat(style.marginTop)),
        marginBottom: Math.round(Number.parseFloat(style.marginBottom)),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        border: style.borderTopColor,
        captionHeight: caption ? Math.round(caption.getBoundingClientRect().height) : 0,
        captionBackground: captionStyle?.backgroundColor ?? "",
        viewportDisplay: viewportStyle?.display ?? "",
        viewportJustify: viewportStyle?.justifyContent ?? "",
        viewportPaddingTop: viewportStyle ? Math.round(Number.parseFloat(viewportStyle.paddingTop)) : 0,
        viewportMaxHeight: viewportStyle?.maxHeight ?? "",
        viewportBackground: viewportStyle?.backgroundColor ?? "",
        viewportBackgroundImage: viewportStyle?.backgroundImage ?? "",
        codeColor: codeStyle?.color ?? "",
        codeLineHeight: codeStyle ? Math.round(Number.parseFloat(codeStyle.lineHeight)) : 0,
        codeFontSize: codeStyle ? Number.parseFloat(codeStyle.fontSize) : 0,
      };
    });

    expect(metrics.width).toBeLessThanOrEqual(780);
    expect(metrics.marginTop).toBe(14);
    expect(metrics.marginBottom).toBe(14);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.border).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.captionHeight).toBe(34);
    expect(metrics.captionBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.viewportDisplay).toBe("block");
    expect(metrics.viewportJustify).not.toBe("center");
    expect(metrics.viewportPaddingTop).toBe(16);
    expect(metrics.viewportMaxHeight).not.toBe("none");
    expect(metrics.viewportBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.viewportBackgroundImage).toBe("none");
    expect(metrics.codeColor).not.toBe("rgb(184, 138, 86)");
    expect(metrics.codeLineHeight).toBe(20);
    expect(metrics.codeFontSize).toBeLessThanOrEqual(12.5);
  });

  test("wide ascii diagrams keep their left edge reachable inside the diagram viewport", async ({ page }) => {
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

    const longRule = "─".repeat(220);
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "wide-ascii-diagram" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "wide-ascii-diagram",
        content: [
          "```text",
          `┌${longRule}┐`,
          "│ Planner ───────────────────────────────→ Executor ───────────────────────────────→ Verifier ───────────────────────────────→ Report │",
          `└${longRule}┘`,
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "wide-ascii-diagram" },
    ], 1);

    const metrics = await page.getByTestId("diagram-surface").evaluate((node) => {
      const viewport = node.querySelector<HTMLElement>("[data-testid='diagram-viewport']");
      const code = node.querySelector<HTMLElement>(".diagram-code");
      if (!viewport || !code) return null;
      const viewportRect = viewport.getBoundingClientRect();
      const codeRect = code.getBoundingClientRect();
      const viewportStyle = getComputedStyle(viewport);
      return {
        viewportDisplay: viewportStyle.display,
        viewportJustify: viewportStyle.justifyContent,
        viewportPaddingLeft: Math.round(Number.parseFloat(viewportStyle.paddingLeft)),
        viewportClientWidth: Math.round(viewport.clientWidth),
        viewportScrollWidth: Math.round(viewport.scrollWidth),
        codeLeft: Math.round(codeRect.left),
        viewportLeft: Math.round(viewportRect.left),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.viewportDisplay).toBe("block");
    expect(metrics!.viewportJustify).not.toBe("center");
    expect(metrics!.viewportScrollWidth).toBeGreaterThan(metrics!.viewportClientWidth);
    expect(metrics!.codeLeft).toBeGreaterThanOrEqual(metrics!.viewportLeft + metrics!.viewportPaddingLeft - 1);
  });

  test("unlabelled multiline box diagrams use the diagram renderer", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "unlabelled-diagram" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "unlabelled-diagram",
        content: [
          "```",
          "+-----------+     +-----------+",
          "| Planner   | --> | Executor  |",
          "+-----------+     +-----------+",
          "      |                 |",
          "      v                 v",
          "+-----------+     +-----------+",
          "| Context   | <-- | Result    |",
          "+-----------+     +-----------+",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "unlabelled-diagram" },
    ], 1);

    await expect(page.getByTestId("diagram-surface")).toBeVisible();
    await expect(page.getByTestId("diagram-surface").locator(".diagram-code")).toContainText("Planner");
    await expect(page.locator(".code-surface")).toHaveCount(0);
  });

  test("streaming markdown renders structure before the final chunk", async ({ page }) => {
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
      { event_type: "session_status", session_id: sessionId, status: "working" },
      { event_type: "text_start", session_id: sessionId, block_id: "streaming-markdown" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "streaming-markdown",
        content: [
          "## 正在整理",
          "",
          "- 先保持结构",
          "- 再补充代码",
          "",
          "```ts",
          "const preview = true;",
        ].join("\n"),
      },
    ], 1);

    const assistant = page.getByTestId("assistant-message");
    await expect(assistant.locator("h2")).toContainText("正在整理");
    await expect(assistant.locator("li").first()).toContainText("先保持结构");
    const streamingCode = assistant.locator(".code-surface");
    await expect(streamingCode).toBeVisible();
    await expect(streamingCode).toHaveAttribute("data-renderer", "plain");
    await expect(streamingCode.locator(".code-fallback code")).toContainText("const preview = true;");
    await expect(streamingCode.locator(".shiki-wrapper")).toHaveCount(0);
    await expect(page.getByTestId("composer-surface")).toHaveAttribute("data-state", "running");

    const streamingMetrics = await page.evaluate(() => {
      const assistant = document.querySelector("[data-testid='assistant-message']");
      if (!assistant) return null;
      const heading = assistant.querySelector("h2");
      const listItem = assistant.querySelector("li");
      const codeSurface = assistant.querySelector(".code-surface");
      const highlightedCode = assistant.querySelector(".shiki-wrapper");
      const plaintextWrapper = assistant.querySelector(".whitespace-pre-wrap");
      return {
        hasHeading: Boolean(heading),
        hasListItem: Boolean(listItem),
        hasCodeSurface: Boolean(codeSurface),
        hasHighlightedCode: Boolean(highlightedCode),
        hasPlaintextWrapper: Boolean(plaintextWrapper),
      };
    });

    expect(streamingMetrics).toEqual({
      hasHeading: true,
      hasListItem: true,
      hasCodeSurface: true,
      hasHighlightedCode: false,
      hasPlaintextWrapper: false,
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "streaming-markdown",
        content: "\n```",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "streaming-markdown" },
    ], 1);

    await expect(page.getByTestId("assistant-message").locator("h2")).toContainText("正在整理");
    await expect(page.locator(".code-surface")).toHaveCount(1);
    await expect(page.locator(".code-surface")).toHaveAttribute("data-renderer", "highlighted");
    await expect(page.locator(".code-surface .shiki-wrapper")).toHaveCount(1);
  });

  test("long assistant replies expose a quiet section index for scanning", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "long-scanning" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "long-scanning",
        content: [
          "下面是一段较长的整理，用来验证长回复的扫读体验。".repeat(8),
          "",
          "## 结论",
          "",
          "先把对话阅读面收稳，再继续做更深的执行能力。",
          "",
          "## 改动范围",
          "",
          "- 对话排版",
          "- diff 阅读",
          "- 工具证据",
          "",
          "## 验收方式",
          "",
          "用 demo 文件夹跑一轮小改动，然后观察 diff、工具和总结。",
          "",
          "## 后续",
          "",
          "继续处理复制、打开和定位的一致性。",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "long-scanning" },
    ], 1);

    const assistant = page.getByTestId("assistant-message");
    const index = assistant.getByTestId("answer-section-index");
    await expect(index).toBeVisible();
    await expect(index.getByText("回复结构")).toBeVisible();
    await expect(index.getByRole("link", { name: "结论" })).toBeVisible();
    await expect(index.getByRole("link", { name: "改动范围" })).toBeVisible();
    await expect(index.getByRole("link", { name: "验收方式" })).toBeVisible();

    const metrics = await index.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        height: Math.round(node.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
      };
    });

    expect(metrics.height).toBeLessThanOrEqual(34);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
  });

  test("message stream uses role-aware rhythm without component margins", async ({ page }) => {
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
      { event_type: "text_start", session_id: sessionId, block_id: "gap-a" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "gap-a", content: "第一条回复。" },
      { event_type: "text_end", session_id: sessionId, block_id: "gap-a" },
      {
        event_type: "tool_call",
        session_id: sessionId,
        block_id: "gap-tool",
        tool_name: "read_file",
        tool_input: { path: "src/App.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "gap-tool",
        result: "ok",
        is_error: false,
        duration_ms: 50,
      },
    ], 1);

    const layout = await page.evaluate(() => {
      const root = document.documentElement;
      const lane = document.querySelector("[data-testid='message-lane']");
      const turn = document.querySelector("[data-testid='conversation-turn']");
      const blocks = [...document.querySelectorAll<HTMLElement>("[data-testid='message-block']")];
      if (!lane || !turn || blocks.length < 2) return null;
      const laneStyle = getComputedStyle(lane);
      const turnStyle = getComputedStyle(turn);
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-message-gap").trim(),
        laneGap: Math.round(Number.parseFloat(laneStyle.rowGap)),
        turnGap: Math.round(Number.parseFloat(turnStyle.rowGap)),
        roles: blocks.map((block) => block.dataset.blockRole),
        margins: blocks.map((block) => {
          const style = getComputedStyle(block);
          return {
            top: Math.round(Number.parseFloat(style.marginTop)),
            bottom: Math.round(Number.parseFloat(style.marginBottom)),
          };
        }),
      };
    });

    expect(layout).not.toBeNull();
    expect(layout!.token).toBe("14px");
    expect(layout!.laneGap).toBe(0);
    expect(layout!.turnGap).toBe(0);
    expect(layout!.roles).toEqual(["assistant", "trace"]);
    expect(layout!.margins[0]).toEqual({ top: 0, bottom: 0 });
    expect(layout!.margins[1]).toEqual({ top: 8, bottom: 0 });
  });

  test("conversation turns create hidden work structure without workflow chrome", async ({ page }) => {
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

    await page.locator("textarea").fill("把 demo 输入框收得更像正式产品。");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call",
        session_id: sessionId,
        block_id: "turn-tool",
        tool_name: "read_file",
        tool_input: { path: "src/InputBar.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "turn-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
      { event_type: "text_start", session_id: sessionId, block_id: "turn-result-a" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "turn-result-a",
        content: "我先只动 demo。输入框已经收了一版，重点看边框、背景和长文本时的稳定性。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-result-a" },
    ], 1);

    await page.locator("textarea").fill("再看一下失败状态。");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "turn-result-b" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "turn-result-b",
        content: "失败状态这轮先保持轻提示，不额外加确认流程。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-result-b" },
    ], 1);

    const turns = page.getByTestId("conversation-turn");
    await expect(turns).toHaveCount(2);
    await expect(turns.nth(0)).toHaveAttribute("data-turn-shape", "with-evidence");
    await expect(turns.nth(1)).toHaveAttribute("data-turn-shape", "direct");
    await expect(turns.nth(0).getByTestId("user-message")).toContainText("把 demo 输入框");
    await expect(turns.nth(0).getByTestId("tool-card-trigger")).toBeVisible();
    await expect(turns.nth(0).getByTestId("assistant-message")).toContainText("我先只动 demo");
    await expect(turns.nth(1).getByTestId("user-message")).toContainText("失败状态");
    await expect(turns.nth(1).getByTestId("assistant-message")).toContainText("轻提示");

    await expect(page.getByText("用户意图", { exact: true })).toHaveCount(0);
    await expect(page.getByText("Forge 理解", { exact: true })).toHaveCount(0);
    await expect(page.getByText("结果与下一步", { exact: true })).toHaveCount(0);

    const metrics = await page.evaluate(() => {
      const turnNodes = [...document.querySelectorAll("[data-testid='conversation-turn']")];
      if (turnNodes.length < 2) return null;
      const firstStyle = getComputedStyle(turnNodes[0]);
      const secondStyle = getComputedStyle(turnNodes[1]);
      const firstBlocks = [...turnNodes[0].querySelectorAll<HTMLElement>("[data-testid='message-block']")];
      return {
        rowGap: Math.round(Number.parseFloat(firstStyle.rowGap)),
        secondPaddingTop: Math.round(Number.parseFloat(secondStyle.paddingTop)),
        firstBackground: firstStyle.backgroundColor,
        firstBorderTop: Math.round(Number.parseFloat(firstStyle.borderTopWidth)),
        firstRadius: Number.parseFloat(firstStyle.borderTopLeftRadius),
        firstRoles: firstBlocks.map((block) => block.dataset.blockRole),
        firstMargins: firstBlocks.map((block) => Math.round(Number.parseFloat(getComputedStyle(block).marginTop))),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.rowGap).toBe(0);
    expect(metrics!.secondPaddingTop).toBeGreaterThanOrEqual(16);
    expect(metrics!.firstBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.firstBorderTop).toBe(0);
    expect(metrics!.firstRadius).toBe(0);
    expect(metrics!.firstRoles).toEqual(["user", "trace", "assistant"]);
    expect(metrics!.firstMargins).toEqual([0, 8, 8]);
  });

  test("context compaction notice follows message rhythm", async ({ page }) => {
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
        event_type: "context_compacted",
        session_id: sessionId,
        block_id: "compact-notice",
        summary: "整理后的上下文摘要",
        compacted_messages: 12,
        retained_messages: 4,
        estimated_tokens_before: 124000,
        estimated_tokens_after: 42000,
      },
    ], 1);

    const metrics = await page.evaluate(() => {
      const trigger = document.querySelector("[data-testid='context-compact-trigger']");
      if (!trigger) return null;
      const wrapper = trigger.closest(".compact-spool");
      const wrapperStyle = wrapper ? getComputedStyle(wrapper) : null;
      const wrapperAfter = wrapper ? getComputedStyle(wrapper, "::after") : null;
      const triggerStyle = getComputedStyle(trigger);
      const meta = trigger.querySelector(".compact-spool-meta");
      const metaStyle = meta ? getComputedStyle(meta) : null;
      return {
        height: Math.round(trigger.getBoundingClientRect().height),
        marginTop: wrapperStyle ? Math.round(Number.parseFloat(wrapperStyle.marginTop)) : -1,
        marginBottom: wrapperStyle ? Math.round(Number.parseFloat(wrapperStyle.marginBottom)) : -1,
        wrapperBackground: wrapperStyle?.backgroundColor ?? "",
        wrapperBorderTop: wrapperStyle?.borderTopWidth ?? "",
        wrapperAfterContent: wrapperAfter?.content ?? "",
        wrapperAfterHeight: wrapperAfter?.height ?? "",
        triggerBackground: triggerStyle.backgroundColor,
        triggerBorderTop: triggerStyle.borderTopWidth,
        triggerRadius: Number.parseFloat(triggerStyle.borderTopLeftRadius),
        metaColor: metaStyle?.color ?? "",
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.height).toBe(28);
    expect(metrics!.marginTop).toBe(0);
    expect(metrics!.marginBottom).toBe(0);
    expect(metrics!.wrapperBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.wrapperBorderTop).toBe("0px");
    expect(metrics!.wrapperAfterContent).toBe("none");
    expect(metrics!.wrapperAfterHeight).toBe("auto");
    expect(metrics!.triggerBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.triggerBorderTop).toBe("1px");
    expect(metrics!.triggerRadius).toBeLessThanOrEqual(8);
    expect(metrics!.metaColor).toBe("rgb(113, 106, 97)");
  });

  test("structured message panels use one compact conversation style", async ({ page }) => {
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
        block_id: "style-confirm",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["src/App.tsx"],
          risk_level: "low",
          checkpoint_status: "ready",
          command: null,
        },
      },
      {
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "style-diff",
        file_path: "src/App.tsx",
        old_content: "-old",
        new_content: "diff --git a/src/App.tsx b/src/App.tsx\n@@\n-old\n+new",
      },
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "style-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 5);

    const panels = page.getByTestId("message-panel");
    await expect(panels).toHaveCount(3);
    const confirmPanel = panels.filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toBeVisible();
    await expect(confirmPanel).toContainText("/Users/cabbos/project/forge");
    await expect(confirmPanel).toContainText("src/App.tsx");
    await expect(confirmPanel).toContainText("这次确认只对当前这一步生效");
    await expect(confirmPanel).toContainText("信任当前项目");
    await expect(panels.filter({ hasText: "文件改动" })).toContainText("src/App.tsx");
    await expect(panels.filter({ hasText: "文件改动" }).getByRole("button", { name: "复制 diff" })).toBeVisible();
    await expect(panels.filter({ hasText: "本轮交付" })).toBeVisible();

    const widths = await panels.evaluateAll((nodes) =>
      nodes.map((node) => Math.round(node.getBoundingClientRect().width)),
    );
    expect(widths.every((width) => width <= 780)).toBeTruthy();

    const margins = await panels.evaluateAll((nodes) =>
      nodes.map((node) => {
        const style = getComputedStyle(node);
        return {
          top: Math.round(Number.parseFloat(style.marginTop)),
          bottom: Math.round(Number.parseFloat(style.marginBottom)),
        };
      }),
    );
    expect(margins.every((margin) => margin.top === 0 && margin.bottom === 0)).toBeTruthy();

    const deliveryMetrics = await panels.filter({ hasText: "本轮交付" }).evaluate((node) => {
      const grid = node.querySelector<HTMLElement>("[data-testid='delivery-summary-grid']");
      const items = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='delivery-summary-item']"));
      const values = Array.from(node.querySelectorAll<HTMLElement>(".forge-delivery-value"));
      const gridStyle = grid ? getComputedStyle(grid) : null;
      return {
        width: Math.round((node as HTMLElement).getBoundingClientRect().width),
        itemCount: items.length,
        gridColumnCount: gridStyle?.gridTemplateColumns.split(" ").filter(Boolean).length ?? 0,
        kinds: items.map((item) => item.dataset.deliveryKind ?? ""),
        valueText: values.map((value) => value.textContent?.trim() ?? ""),
        valueColors: values.map((value) => getComputedStyle(value).color),
        itemBackgrounds: items.map((item) => getComputedStyle(item).backgroundColor),
        itemBorders: items.map((item) => getComputedStyle(item).borderTopColor),
        minItemHeight: items.length ? Math.min(...items.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
      };
    });
    expect(deliveryMetrics.width).toBeLessThanOrEqual(720);
    expect(deliveryMetrics.itemCount).toBe(3);
    expect(deliveryMetrics.gridColumnCount).toBe(3);
    expect(deliveryMetrics.kinds).toEqual(["preview", "checkpoint", "next"]);
    expect(deliveryMetrics.valueText).toEqual(["预览未运行", "检查点已就绪", "下一步：检查当前版本。"]);
    expect(deliveryMetrics.valueColors.every((color) => color === "rgb(36, 42, 36)")).toBeTruthy();
    expect(deliveryMetrics.itemBackgrounds.every((color) => color !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(deliveryMetrics.itemBorders.every((color) => color !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(deliveryMetrics.minItemHeight).toBeGreaterThanOrEqual(52);
  });

  test("ask_user confirmation explains boolean-only response limits", async ({ page }) => {
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
        block_id: "ask-user-confirm",
        question: "你想把状态区放在哪里？",
        kind: "ask_user",
      },
    ], 5);

    const prompt = page.getByTestId("message-panel").filter({ hasText: "你想把状态区放在哪里？" });
    await expect(prompt).toContainText("这一步只能确认是否继续");
    await expect(prompt).toContainText("请直接发一条新消息");
  });

  test("write_file tool details show a markdown write preview", async ({ page }) => {
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
        block_id: "write-preview-tool",
        tool_name: "write_file",
        tool_input: {
          path: "docs/runtime.md",
          content: "# Runtime\n\n- Gateway\n- Sessions\n",
        },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "write-preview-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
    ], 1);

    await page.getByTestId("tool-card-trigger").click();

    const preview = page.getByTestId("write-file-preview");
    await expect(preview).toBeVisible();
    await expect(preview).toContainText("docs/runtime.md");
    await expect(preview).toContainText("Markdown");
    await expect(preview.locator(".markdown-content")).toContainText("Gateway");
  });

  test("write_file tool details show an image write preview", async ({ page }) => {
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
        block_id: "image-write-preview-tool",
        tool_name: "write_file",
        tool_input: {
          path: "assets/logo.svg",
          content: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="#0f766e" /></svg>',
        },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "image-write-preview-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
    ], 1);

    await page.getByTestId("tool-card-trigger").click();

    const preview = page.getByTestId("write-file-preview");
    await expect(preview).toBeVisible();
    await expect(preview).toContainText("assets/logo.svg");
    await expect(preview).toContainText("SVG");

    const image = preview.getByTestId("write-file-image-preview");
    await expect(image).toBeVisible();
    await expect(image).toHaveAttribute("src", /^data:image\/svg\+xml;utf8,/);

    const imageBox = await image.boundingBox();
    expect(imageBox?.width ?? 0).toBeGreaterThan(0);
    expect(imageBox?.height ?? 0).toBeGreaterThan(0);
  });

  test("diff cards show a compact file tree for multi-file changes", async ({ page }) => {
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
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "multi-file-diff",
        file_path: "workspace",
        old_content: "",
        new_content: [
          "diff --git a/src/App.tsx b/src/App.tsx",
          "--- a/src/App.tsx",
          "+++ b/src/App.tsx",
          "@@ -1 +1,2 @@",
          "-old",
          "+new",
          "+enabled",
          "diff --git a/docs/runtime.md b/docs/runtime.md",
          "--- /dev/null",
          "+++ b/docs/runtime.md",
          "@@ -0,0 +1 @@",
          "+# Runtime",
        ].join("\n"),
      },
    ], 1);

    const tree = page.getByTestId("diff-file-tree");
    await expect(tree).toBeVisible();
    await expect(tree).toContainText("src/App.tsx");
    await expect(tree).toContainText("docs/runtime.md");
    await expect(tree).toContainText("+2");
    await expect(tree).toContainText("+1");
  });

  test("image diff cards show before and after previews", async ({ page }) => {
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
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "image-diff",
        file_path: "assets/logo.svg",
        old_content: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="#b91c1c" /></svg>',
        new_content: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" fill="#0f766e" /></svg>',
      },
    ], 1);

    await page.getByTestId("diff-body-toggle").click();

    const imageDiff = page.getByTestId("image-diff-preview");
    await expect(imageDiff).toBeVisible();
    await expect(imageDiff).toContainText("之前");
    await expect(imageDiff).toContainText("之后");

    const before = imageDiff.getByTestId("image-diff-before");
    const after = imageDiff.getByTestId("image-diff-after");
    await expect(before).toHaveAttribute("src", /^data:image\/svg\+xml;utf8,/);
    await expect(after).toHaveAttribute("src", /^data:image\/svg\+xml;utf8,/);

    const boxes = await Promise.all([before.boundingBox(), after.boundingBox()]);
    expect(boxes.every((box) => (box?.width ?? 0) > 0 && (box?.height ?? 0) > 0)).toBeTruthy();
  });

  test("user messages can carry pasted code paths and logs without breaking the lane", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill([
      "看一下 `src/App.tsx:12`，这里报错：",
      "",
      "```bash",
      "Error: Cannot find module '@/components/ReallyLongBrokenComponentNameThatShouldNotStretchTheBubble'",
      "    at src/App.tsx:12:3",
      "```",
    ].join("\n"));
    await page.locator("textarea").press("Enter");

    const userMessage = page.getByTestId("user-message").last();
    await expect(userMessage.locator(".code-surface")).toBeVisible();
    await expect(userMessage.locator(".forge-file-ref-name")).toHaveText("App.tsx");
    await expect(userMessage.locator(".forge-file-ref-line")).toHaveText("line 12");
    await expect(userMessage.locator(".forge-file-ref")).toHaveAttribute("title", "src/App.tsx:12");

    const metrics = await userMessage.evaluate((node) => {
      const bubble = node.getBoundingClientRect();
      const lane = document.querySelector("[data-testid='message-lane']")?.getBoundingClientRect();
      const code = node.querySelector(".code-surface");
      const codeScroll = node.querySelector(".code-scroll");
      if (!lane || !code || !codeScroll) return null;
      return {
        bubbleWidth: Math.round(bubble.width),
        laneWidth: Math.round(lane.width),
        codeWidth: Math.round(code.getBoundingClientRect().width),
        overflowX: getComputedStyle(codeScroll).overflowX,
        whiteSpace: getComputedStyle(node).whiteSpace,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.bubbleWidth).toBeLessThan(metrics!.laneWidth);
    expect(metrics!.codeWidth).toBeLessThanOrEqual(metrics!.bubbleWidth);
    expect(metrics!.overflowX).toBe("auto");
    expect(metrics!.whiteSpace).toBe("normal");
  });

  test("project archive disclosure rows use inspector rhythm tokens", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.setViewportSize({ width: 900, height: 720 });
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.keyboard.down("Control");
    await page.keyboard.press("i");
    await page.keyboard.up("Control");

    const archive = page.getByRole("complementary", { name: "项目档案" });
    await expect(archive).toBeVisible();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const archive = document.querySelector<HTMLElement>("[data-testid='project-archive-panel']");
      const body = document.querySelector<HTMLElement>("[data-testid='project-archive-body']");
      const disclosure = document.querySelector<HTMLElement>("[data-testid='archive-disclosure-records'] button");
      const main = document.querySelector<HTMLElement>("[data-testid='main-workbench']");
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const modelChip = document.querySelector<HTMLElement>("[data-testid='composer-model-chip']");
      const title = document.querySelector<HTMLElement>(".forge-inspector-title");
      const subtitle = document.querySelector<HTMLElement>(".forge-inspector-subtitle");
      const summaryLabel = document.querySelector<HTMLElement>(".forge-archive-summary-label");
      const summaryValue = document.querySelector<HTMLElement>(".forge-archive-summary-value");
      if (!archive || !body || !disclosure || !main || !composer || !modelChip || !title || !subtitle || !summaryLabel || !summaryValue) return null;
      const archiveRect = archive.getBoundingClientRect();
      const composerRect = composer.getBoundingClientRect();
      const modelChipRect = modelChip.getBoundingClientRect();
      const archiveStyle = getComputedStyle(archive);
      return {
        widthToken: getComputedStyle(root).getPropertyValue("--forge-inspector-width").trim(),
        gapToken: getComputedStyle(root).getPropertyValue("--forge-inspector-gap").trim(),
        rowToken: getComputedStyle(root).getPropertyValue("--forge-disclosure-row-height").trim(),
        width: Math.round(archiveRect.width),
        background: archiveStyle.backgroundColor,
        backdropFilter: archiveStyle.backdropFilter,
        bodyGap: Math.round(Number.parseFloat(getComputedStyle(body).rowGap)),
        rowHeight: Math.round(disclosure.getBoundingClientRect().height),
        archiveLeft: Math.round(archiveRect.left),
        composerRight: Math.round(composerRect.right),
        modelChipRight: Math.round(modelChipRect.right),
        mainPaddingRight: getComputedStyle(main).paddingRight,
        titleFontSize: getComputedStyle(title).fontSize,
        subtitleFontSize: getComputedStyle(subtitle).fontSize,
        subtitleColor: getComputedStyle(subtitle).color,
        summaryLabelFontSize: getComputedStyle(summaryLabel).fontSize,
        summaryValueColor: getComputedStyle(summaryValue).color,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.widthToken).toBe("300px");
    expect(metrics!.gapToken).toBe("10px");
    expect(metrics!.rowToken).toBe("28px");
    expect(metrics!.width).toBe(300);
    expect(metrics!.background).toBe("rgb(236, 226, 212)");
    expect(metrics!.backdropFilter).toBe("none");
    expect(metrics!.bodyGap).toBe(10);
    expect(metrics!.rowHeight).toBe(28);
    expect(metrics!.mainPaddingRight).toBe(metrics!.widthToken);
    expect(metrics!.composerRight).toBeLessThanOrEqual(metrics!.archiveLeft - 12);
    expect(metrics!.modelChipRight).toBeLessThanOrEqual(metrics!.archiveLeft - 12);
    expect(metrics!.titleFontSize).toBe("14px");
    expect(metrics!.subtitleFontSize).toBe("11px");
    expect(metrics!.subtitleColor).toBe("rgb(95, 93, 85)");
    expect(metrics!.summaryLabelFontSize).toBe("11px");
    expect(metrics!.summaryValueColor).toBe("rgb(36, 42, 36)");
  });

  test("global new conversation shortcut starts from the active workspace", async ({ page }) => {
    await page.keyboard.down("Control");
    await page.keyboard.press("n");
    await page.keyboard.up("Control");

    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByRole("main").getByText("选择一个项目开始")).toHaveCount(0);
  });



test.describe("Timeline Messages", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await forceDarkWorkbench(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("empty workbench primary action starts a conversation", async ({ page }) => {
      await page.getByRole("main").getByRole("button", { name: "开始新对话" }).click();
  
      await expect(page.locator("textarea")).toBeVisible();
      const createArgs = await page.evaluate(() => {
        // @ts-expect-error mock
        return window.__lastCreateSessionArgs;
      });
      expect(createArgs.workingDir).toBe("/Users/cabbos/project/forge");
    });
  
    test("empty workbench shows start readiness before the first conversation", async ({ page }) => {
      await page.addInitScript(() => {
        // @ts-expect-error mock
        window.__mockApiKeyStatus = [
          { provider: "deepseek", configured: false, source: "none", status: "not_configured", error: null },
        ];
      });
      await page.goto("http://localhost:1420");
  
      const main = page.getByRole("main");
      const readiness = main.getByTestId("start-readiness");
      await expect(readiness).toBeVisible();
      await expect(readiness.getByText("需要配置模型密钥")).toBeVisible();
      await expect(readiness.getByText("还没有配置 DeepSeek", { exact: true })).toBeVisible();
      await readiness.getByRole("button", { name: "打开设置" }).first().click();
      await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    });
  
    test("desktop chrome keeps the conversation surface restrained", async ({ page }) => {
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
  
      const titlebar = page.getByTestId("app-titlebar");
      await expect(titlebar).toHaveCSS("height", "64px");
  
      const composerFrame = page.getByTestId("composer-frame");
      await expect(composerFrame).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
      await expect(composerFrame).toHaveCSS("backdrop-filter", "none");
      await expect(page.getByTestId("composer-surface")).not.toHaveCSS("box-shadow", "none");
  
      await simulateStream(page, sessionId, fullConversation(sessionId), 10);
      const processSummary = page.getByTestId("tool-activity-summary").first();
      await expect(processSummary).toContainText("过程已收起 · 2 步");
      await expect(processSummary).toHaveCSS("min-height", "24px");
    });
  
    test("V3 operating surface keeps conversation focused without the Inspector rail", async ({ page }) => {
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
  
      const shell = page.getByTestId("operating-surface");
      await expect(shell).toHaveAttribute("data-design-version", "v3-light-workbench");
  
      const tokens = await page.evaluate(() => {
        const root = getComputedStyle(document.documentElement);
        const parseHex = (hex: string) => {
          const value = hex.trim().replace("#", "");
          return {
            r: Number.parseInt(value.slice(0, 2), 16),
            g: Number.parseInt(value.slice(2, 4), 16),
            b: Number.parseInt(value.slice(4, 6), 16),
          };
        };
        const luminance = (color: { r: number; g: number; b: number }) => {
          const [r, g, b] = [color.r, color.g, color.b].map((channel) => {
            const value = channel / 255;
            return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
          });
          return 0.2126 * r + 0.7152 * g + 0.0722 * b;
        };
        const contrast = (foreground: string, background: string) => {
          const fg = luminance(parseHex(foreground));
          const bg = luminance(parseHex(background));
          return (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);
        };
        const base = root.getPropertyValue("--forge-bg-base").trim();
        const depth = root.getPropertyValue("--forge-bg-depth").trim();
        const raised = root.getPropertyValue("--forge-bg-raised").trim();
        const muted = root.getPropertyValue("--forge-text-muted").trim();
        const faint = root.getPropertyValue("--forge-text-faint").trim();
        return {
          base,
          ink: root.getPropertyValue("--forge-ink").trim(),
          brass: root.getPropertyValue("--forge-accent").trim(),
          muted,
          faint,
          mutedOnBase: contrast(muted, base),
          mutedOnDepth: contrast(muted, depth),
          faintOnBase: contrast(faint, base),
          faintOnDepth: contrast(faint, depth),
          faintOnRaised: contrast(faint, raised),
        };
      });
      expect(tokens.base).toBe("#1B1A17");
      expect(tokens.ink).toBe("#12110F");
      expect(tokens.brass).toBe("#B88A56");
      expect(tokens.mutedOnBase).toBeGreaterThanOrEqual(4.5);
      expect(tokens.mutedOnDepth).toBeGreaterThanOrEqual(4.5);
      expect(tokens.faintOnBase).toBeGreaterThanOrEqual(4.5);
      expect(tokens.faintOnDepth).toBeGreaterThanOrEqual(4.5);
      expect(tokens.faintOnRaised).toBeGreaterThanOrEqual(4.5);
  
      await expect(page.getByTestId("project-cockpit")).toHaveCount(0);
      await expect(page.getByRole("complementary", { name: "Inspector" })).toHaveCount(0);
      await expect(page.getByTestId("message-lane")).toHaveAttribute("data-surface", "conversation");
      await expect(page.getByTestId("composer-frame")).toHaveAttribute("data-surface", "composer");
  
      const layout = await page.evaluate(() => {
        const shell = document.querySelector<HTMLElement>("[data-testid='operating-surface']");
        const main = document.querySelector<HTMLElement>("[data-testid='main-workbench']");
        const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
        if (!shell || !main || !lane) return null;
        return {
          columns: getComputedStyle(shell).gridTemplateColumns,
          mainRight: Math.round(main.getBoundingClientRect().right),
          viewportRight: Math.round(window.innerWidth),
          laneWidth: Math.round(lane.getBoundingClientRect().width),
        };
      });
      expect(layout).not.toBeNull();
      expect(layout!.columns).not.toContain("320px");
      expect(layout!.mainRight).toBe(layout!.viewportRight);
      expect(layout!.laneWidth).toBeLessThanOrEqual(820);
  
      const decorativeSurfaces = await page.evaluate(() => {
        const scroll = document.querySelector("[data-testid='conversation-scroll']");
        const operatingLane = document.querySelector(".forge-operating-lane");
        return {
          scrollTexture: scroll ? getComputedStyle(scroll, "::after").backgroundImage : "",
          operatingRail: operatingLane ? getComputedStyle(operatingLane, "::before").content : "",
        };
      });
      expect(decorativeSurfaces.scrollTexture).toBe("none");
      expect(decorativeSurfaces.operatingRail).toBe("none");
  
      await simulateStream(page, sessionId, fullConversation(sessionId), 10);
      await expect(page.getByTestId("tool-activity-summary").first()).toContainText("过程已收起 · 2 步");
      await expect(page.getByRole("complementary", { name: "Inspector" })).toHaveCount(0);
      const turnDecoration = await page.getByTestId("conversation-scroll").evaluate(() => {
        const turn = document.querySelector(".forge-conversation-turn");
        if (!turn) return null;
        const turnStyle = getComputedStyle(turn);
        return {
          bead: getComputedStyle(turn, "::after").content,
          borderLeft: turnStyle.borderLeftWidth,
        };
      });
      expect(turnDecoration).not.toBeNull();
      expect(turnDecoration!.bead).toBe("none");
      expect(turnDecoration!.borderLeft).toBe("0px");
  
      const assistantDecoration = await page.getByTestId("assistant-message").first().evaluate((node) => {
        const before = getComputedStyle(node, "::before");
        return {
          content: before.content,
          width: before.width,
          background: before.backgroundColor,
        };
      });
      expect(assistantDecoration.content).toBe("none");
      expect(assistantDecoration.width).toBe("auto");
    });
  
    test("long assistant replies do not add an automatic acceptance checklist", async ({ page }) => {
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
        { event_type: "text_start", session_id: sessionId, block_id: "long-answer" },
        {
          event_type: "text_chunk",
          session_id: sessionId,
          block_id: "long-answer",
          content: "我已经把第一版方向整理好了。这个版本会先保留一个核心界面，一个主要交互，以及一个清楚的下一步。用户可以直接继续描述想改哪里，Forge 会在当前项目里继续推进，而不是让用户管理一堆流程提示。这里还会补充当前版本包含什么、暂时不包含什么、为什么先从最小可用版本开始，以及如果预览失败应该优先检查哪个地方。",
        },
        { event_type: "text_end", session_id: sessionId, block_id: "long-answer" },
      ], 5);
  
      const main = page.getByRole("main");
      await expect(main.getByText("验收清单", { exact: true })).toHaveCount(0);
      await expect(main.getByText("下一步提示词", { exact: true })).toHaveCount(0);
    });
  
    test("restores the active conversation after reload", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.evaluate(async (sessionId) => {
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
          const request = indexedDB.open("keyval-store");
          request.onerror = () => reject(request.error);
          request.onsuccess = () => resolve(request.result);
        });
        const tx = db.transaction("keyval", "readwrite");
        tx.objectStore("keyval").put([
          {
            id: sessionId,
            agentType: "deepseek",
            model: "deepseek-v4-flash",
            contextWindowTokens: 1_000_000,
            status: "stopped",
            workflowState: null,
          },
        ], "forge-sessions");
        tx.objectStore("keyval").put([
          {
            block_id: "seed-user-message",
            event_type: "user_message",
            content: "已有对话内容",
            isComplete: true,
            metadata: {},
          },
        ], `forge-blocks:${sessionId}`);
        await new Promise<void>((resolve, reject) => {
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      }, sessionId);
  
      await page.reload();
  
      await expect(page.locator("textarea")).toBeVisible();
      await expect(page.getByRole("main").getByText("已有对话内容").last()).toBeVisible();
      await expect(page.getByRole("main").getByText("从当前任务开始")).toHaveCount(0);
    });
  
    test("stopping generation keeps the conversation instead of deleting it", async ({ page }) => {
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
        { event_type: "session_status", session_id: sessionId, status: "working" },
      ], 1);
  
      await page.getByTestId("composer-stop").click();
  
      const stoppedSessionId = await page.evaluate(() => {
        // @ts-expect-error mock
        return window.__lastKilledSessionId;
      });
      expect(stoppedSessionId).toBe(sessionId);
      await expect(page.getByRole("button", { name: "继续会话" })).toBeVisible();
      await expect(page.locator("aside").first().getByRole("button", { name: /删除对话/ })).toHaveCount(1);
    });
  
    test("deleting a conversation still removes it from history", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const sidebar = page.locator("aside").first();
      await sidebar.getByRole("button", { name: /删除对话/ }).click();
  
      const deletedSessionId = await page.evaluate(() => {
        // @ts-expect-error mock
        return window.__lastDeletedSessionId;
      });
      expect(deletedSessionId).toBe(sessionId);
      await expect(sidebar.getByRole("button", { name: /删除对话/ })).toHaveCount(0);
      await expect(sidebar.getByText("还没有对话")).toBeVisible();
    });
  
    test("design system materials stay subtle and token-driven", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
      await page.locator("textarea").focus();
      await page.waitForFunction(() => {
        const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
        if (!composer) return false;
        const rootStyle = getComputedStyle(composer);
        const composerStyle = getComputedStyle(composer);
        return composerStyle.borderTopColor === rootStyle.getPropertyValue("--forge-material-border-focus").trim();
      });
  
      const metrics = await page.evaluate(() => {
        const root = document.querySelector<HTMLElement>(".forge-app-shell") ?? document.documentElement;
        const titlebar = document.querySelector("[data-testid='app-titlebar']");
        const sidebar = document.querySelector("aside");
        const composer = document.querySelector("[data-testid='composer-surface']");
        if (!titlebar || !sidebar || !composer) return null;
        const rootStyle = getComputedStyle(root);
        const titlebarStyle = getComputedStyle(titlebar);
        const sidebarStyle = getComputedStyle(sidebar);
        const composerStyle = getComputedStyle(composer);
        const resolveColor = (color: string) => {
          const probe = document.createElement("span");
          probe.style.color = color;
          document.body.append(probe);
          const resolved = getComputedStyle(probe).color;
          probe.remove();
          return resolved;
        };
        const materialBorderColor = resolveColor(rootStyle.getPropertyValue("--forge-material-border").trim());
        return {
          borderSubtle: rootStyle.getPropertyValue("--forge-border-subtle").trim(),
          materialBorder: rootStyle.getPropertyValue("--forge-material-border").trim(),
          materialBorderColor,
          materialSurface: rootStyle.getPropertyValue("--forge-material-surface").trim(),
          materialRaised: rootStyle.getPropertyValue("--forge-material-raised").trim(),
          materialPopover: rootStyle.getPropertyValue("--forge-material-popover").trim(),
          materialOverlay: rootStyle.getPropertyValue("--forge-material-overlay").trim(),
          materialShadow: rootStyle.getPropertyValue("--forge-material-shadow").trim(),
          composerBorderToken: rootStyle.getPropertyValue("--forge-composer-border").trim(),
          composerBorderFocusToken: rootStyle.getPropertyValue("--forge-material-border-focus").trim(),
          composerSurface: rootStyle.getPropertyValue("--forge-composer-surface").trim(),
          composerSurfaceFocus: rootStyle.getPropertyValue("--forge-composer-surface-focus").trim(),
          composerSurfaceFocusColor: resolveColor(rootStyle.getPropertyValue("--forge-composer-surface-focus").trim()),
          composerShadowToken: rootStyle.getPropertyValue("--forge-composer-shadow").trim(),
          bgRaised: rootStyle.getPropertyValue("--forge-bg-raised").trim(),
          hover: rootStyle.getPropertyValue("--forge-hover").trim(),
          focusRing: rootStyle.getPropertyValue("--forge-focus-ring").trim(),
          titlebarBorder: titlebarStyle.borderBottomColor,
          sidebarBorder: sidebarStyle.borderRightColor,
          composerBorder: composerStyle.borderTopColor,
          composerBg: composerStyle.backgroundColor,
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.borderSubtle).toBe("#DDD2C3");
      expect(metrics!.materialBorder).toBe("#DDD2C3");
      expect(metrics!.materialSurface).toBe("#FEFCF8");
      expect(metrics!.materialRaised).toBe("#FEFCF8");
      expect(metrics!.materialPopover).toBe("#FEFCF8");
      expect(metrics!.materialOverlay).toBe("rgba(254, 252, 248, 0.97)");
      expect(metrics!.materialShadow).toContain("0 10px 24px");
      expect(metrics!.composerBorderToken).toBe("#D8C9B8");
      expect(metrics!.composerSurface).toBe("#FEFCF8");
      expect(metrics!.composerShadowToken).toContain("0 10px 26px");
      expect(metrics!.bgRaised).toBe("#F4EEE4");
      expect(metrics!.hover).toBe("rgba(92, 81, 68, 0.055)");
      expect(metrics!.focusRing).toBe("rgba(184, 138, 86, 0.38)");
      expect(metrics!.titlebarBorder).toBe(metrics!.materialBorderColor);
      expect(metrics!.sidebarBorder).toBe("rgba(221, 210, 195, 0.72)");
      expect(metrics!.composerBorder).toBe(metrics!.composerBorderFocusToken);
      expect(metrics!.composerBg).toBe(metrics!.composerSurfaceFocusColor);
    });
  
    test("V3 color ladder keeps the dark workbench readable", async ({ page }) => {
      await page.goto("http://localhost:1420");
  
      const tokens = await page.evaluate(() => {
        const root = getComputedStyle(document.documentElement);
        return {
          base: root.getPropertyValue("--forge-bg-base").trim(),
          depth: root.getPropertyValue("--forge-bg-depth").trim(),
          surface: root.getPropertyValue("--forge-bg-surface").trim(),
          raised: root.getPropertyValue("--forge-bg-raised").trim(),
          composer: root.getPropertyValue("--forge-bg-composer").trim(),
          muted: root.getPropertyValue("--forge-text-muted").trim(),
        };
      });
  
      expect(tokens).toEqual({
        base: "#1B1A17",
        depth: "#12110F",
        surface: "#22201C",
        raised: "#26231E",
        composer: "#22201C",
        muted: "#928B7E",
      });
    });
  
    test("capability drawer reads as a compact safety console", async ({ page }) => {
      const sidebar = page.locator("aside").first();
      await sidebar.getByRole("button", { name: "插件" }).click();
  
      const drawer = page.getByRole("complementary", { name: "插件" });
      await expect(drawer).toBeVisible();
      const manager = drawer.locator(".forge-capability-manager");
      await expect(manager).toBeVisible();
      const firstRow = manager.locator(".forge-capability-row").filter({ hasText: "File Reader" }).first();
      await expect(firstRow).toBeVisible();
      await expect.poll(async () => manager.evaluate((node) => node.querySelectorAll<HTMLElement>("[style]").length)).toBe(0);
  
      const metrics = await manager.evaluate((node) => {
        const root = document.documentElement;
        const style = getComputedStyle(node);
        const tab = node.querySelector<HTMLElement>(".forge-capability-tab[aria-selected='true']");
        const summary = node.querySelector<HTMLElement>("[data-testid='capability-summary-strip']");
        const summaryItems = Array.from(node.querySelectorAll<HTMLElement>(".forge-capability-summary-item"));
        const search = node.querySelector<HTMLElement>(".forge-capability-search");
        const row = node.querySelector<HTMLElement>(".forge-capability-row");
        const toggle = node.querySelector<HTMLElement>(".forge-capability-toggle[data-state='enabled']");
        const inlineStyled = node.querySelectorAll<HTMLElement>("[style]");
        const toggleStyle = toggle ? getComputedStyle(toggle) : null;
        const tabStyle = tab ? getComputedStyle(tab) : null;
        const summaryStyle = summary ? getComputedStyle(summary) : null;
        const searchStyle = search ? getComputedStyle(search) : null;
        const rowStyle = row ? getComputedStyle(row) : null;
        return {
          accent: getComputedStyle(root).getPropertyValue("--forge-accent").trim(),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          tabHeight: tab ? Math.round(tab.getBoundingClientRect().height) : 0,
          tabBorder: tabStyle?.borderBottomColor ?? "",
          summaryDisplay: summaryStyle?.display ?? "",
          summaryItemCount: summaryItems.length,
          summaryMaxHeight: summaryItems.length ? Math.max(...summaryItems.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
          motionEntryCount: node.querySelectorAll("[data-forge-motion='capability-entry']").length,
          searchHeight: search ? Math.round(search.getBoundingClientRect().height) : 0,
          rowHeight: row ? Math.round(row.getBoundingClientRect().height) : 0,
          rowBackground: rowStyle?.backgroundColor ?? "",
          toggleColor: toggleStyle?.color ?? "",
          toggleBackground: toggleStyle?.backgroundColor ?? "",
          inlineStyledCount: inlineStyled.length,
        };
      });
  
      expect(metrics.accent).toBe("#B88A56");
      expect(metrics.radius).toBeLessThanOrEqual(8);
      expect(metrics.tabHeight).toBe(32);
      expect(metrics.tabBorder).toBe("rgb(196, 138, 58)");
      expect(metrics.summaryDisplay).toBe("grid");
      expect(metrics.summaryItemCount).toBe(3);
      expect(metrics.summaryMaxHeight).toBeLessThanOrEqual(44);
      expect(metrics.motionEntryCount).toBeGreaterThanOrEqual(3);
      expect(metrics.searchHeight).toBe(32);
      expect(metrics.rowHeight).toBeGreaterThanOrEqual(44);
      expect(metrics.rowBackground).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics.toggleColor).toBe("rgb(196, 138, 58)");
      expect(metrics.toggleBackground).not.toContain("16, 185, 129");
      expect(metrics.toggleBackground).not.toContain("52, 211, 153");
      expect(metrics.inlineStyledCount).toBe(0);
    });
  });
