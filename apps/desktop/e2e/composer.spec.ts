import { test, expect } from "@playwright/test";
import {
  setup,
  expectLastSendInputArgs,
  expectNoSendInput,
} from "./fixtures/app";

test.describe("InputBar", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
  });

  test("enter key sends message and clears input", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("Hello DeepSeek");
    await textarea.press("Enter");

    // User bubble should appear
    await expect(page.getByRole("main").getByText("Hello DeepSeek", { exact: true }).last()).toBeVisible({ timeout: 3000 });
  });

  test("empty start readiness checks the active workspace explicitly", async ({ page }) => {
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript((projectPath) => {
      window.localStorage.setItem("forge-working-dir", projectPath);
    }, projectPath);

    await page.goto("http://localhost:1420");
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();

    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastProjectRuntimeStatusArgs;
    })).toMatchObject({ sessionId: null, workingDir: projectPath });
    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastProjectCheckpointStatusArgs;
    })).toMatchObject({ sessionId: null, workingDir: projectPath });
  });

  test("composer checkpoint is created inside the active session workspace", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("帮我检查这个 demo 页面");
    await textarea.press("Enter");

    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateProjectCheckpointArgs;
    })).toMatchObject({ sessionId, workingDir: projectPath });
  });

  test("send failures show an inline recovery message and clear pending state", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.evaluate(() => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "send_input") {
          throw new Error("Session not found: send-failure-test");
        }
        return original?.(cmd, args);
      };
    });

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("帮我继续做这个页面");
    await textarea.press("Enter");

    await expect(page.getByTestId("user-message").last()).toContainText("帮我继续做这个页面");
    const errorCard = page.getByTestId("message-panel").filter({ hasText: "发送失败" });
    await expect(errorCard).toHaveAttribute("role", "status");
    await expect(errorCard.getByTestId("error-card-body")).toContainText("当前会话暂时不可用");
    const errorMetrics = await errorCard.evaluate((node) => {
      const body = node.querySelector<HTMLElement>("[data-testid='error-card-body']");
      const style = getComputedStyle(node);
      const after = getComputedStyle(node, "::after");
      return {
        width: Math.round(node.getBoundingClientRect().width),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        border: style.borderTopColor,
        bodyHeight: body ? Math.round(body.getBoundingClientRect().height) : 0,
        afterContent: after.content,
        afterWidth: after.width,
      };
    });
    expect(errorMetrics.width).toBeLessThanOrEqual(620);
    expect(errorMetrics.radius).toBeLessThanOrEqual(8);
    expect(errorMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(errorMetrics.border).not.toBe("rgba(0, 0, 0, 0)");
    expect(errorMetrics.bodyHeight).toBeLessThanOrEqual(38);
    expect(errorMetrics.afterContent).toBe("none");
    expect(errorMetrics.afterWidth).toBe("auto");
    await expect(page.getByTestId("pending-block")).toHaveCount(0);
    await expect(textarea).toBeEnabled();
  });

  test("shift+enter creates newline without sending", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("line1");
    await textarea.press("Shift+Enter");
    await textarea.pressSequentially("line2");

    // Should still be in the textarea, not sent
    await expect(textarea).toContainText("line1\nline2");
  });

  test("long prompts stay inside a bounded editor scroll area", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    const longPrompt = Array.from({ length: 24 }, (_, index) => `第 ${index + 1} 行：继续描述这个小工具的细节。`).join("\n");
    await textarea.fill(longPrompt);

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const textarea = document.querySelector("textarea");
      if (!textarea) return null;
      const rect = textarea.getBoundingClientRect();
      const style = getComputedStyle(textarea);

      return {
        token: getComputedStyle(root).getPropertyValue("--forge-composer-max-input-height").trim(),
        height: Math.round(rect.height),
        maxHeight: Math.round(Number.parseFloat(style.maxHeight)),
        overflowY: style.overflowY,
        canScrollInside: textarea.scrollHeight > textarea.clientHeight,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.token).toBe("128px");
    expect(metrics!.height).toBeLessThanOrEqual(128);
    expect(metrics!.maxHeight).toBe(128);
    expect(metrics!.overflowY).toBe("auto");
    expect(metrics!.canScrollInside).toBe(true);
  });

  test("enter during IME composition does not send the draft", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("正在组词");
    await textarea.focus();
    await textarea.evaluate((node) => {
      node.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true, data: "zheng" }));
      node.dispatchEvent(new KeyboardEvent("keydown", {
        key: "Enter",
        code: "Enter",
        bubbles: true,
        cancelable: true,
      }));
    });

    await expect(textarea).toHaveValue("正在组词");
    await expect(page.getByTestId("user-message")).toHaveCount(0);
  });

  test("composer command surface stays compact but exposes structured controls", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const slash = composer.getByRole("button", { name: "常用请求" });
    await expect(slash).toHaveAttribute("aria-expanded", "false");
    await slash.click();
    await expect(slash).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByTestId("composer-command-menu")).toHaveAttribute("role", "listbox");
    await expect(page.getByRole("option", { name: /\/code-review/ })).toBeVisible();

    await page.keyboard.press("Escape");
    const model = composer.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ });
    await expect(model).toHaveAttribute("aria-expanded", "false");
    await model.click();
    await expect(model).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("menuitemradio", { name: /DeepSeek V4 Flash 1M/ })).toHaveAttribute("aria-checked", "true");
  });

  test("composer capability rows use semantic icon tones", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    await composer.getByRole("button", { name: "常用请求" }).click();
    await expect(page.getByTestId("composer-command-menu").getByTestId("forge-icon-action").first()).toBeVisible();

    await page.keyboard.press("Escape");
    const textarea = page.locator("textarea");
    await textarea.fill("@src");
    await expect(page.getByTestId("composer-command-menu").getByTestId("forge-icon-context").first()).toBeVisible();
  });

  test("composer command menu supports keyboard selection", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("/");
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();
    await expect(page.getByRole("option", { name: /\/code-review/ })).toHaveAttribute("aria-selected", "true");

    await textarea.press("ArrowDown");
    await expect(page.getByRole("option", { name: /\/fix/ })).toHaveAttribute("aria-selected", "true");
    await textarea.press("Enter");

    const composer = page.getByTestId("composer-lane");
    await expect(composer.getByText("/fix", { exact: true })).toBeVisible();
    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expectNoSendInput(page);
  });

  test("composer file suggestions can be accepted without leaving the keyboard", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toBeVisible();
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toHaveAttribute("aria-selected", "true");

    await textarea.press("Tab");

    const composer = page.getByTestId("composer-lane");
    await expect(composer.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expect(textarea).toHaveValue("");
  });

  test("composer keyboard selection ignores a stationary pointer when file suggestions open", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const composer = page.getByTestId("composer-lane");
    const textarea = composer.locator("textarea");
    await expect(textarea).toBeEnabled();

    await textarea.fill("@src");
    const secondOption = page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ });
    await expect(secondOption).toBeVisible();
    const secondOptionBox = await secondOption.boundingBox();
    expect(secondOptionBox).not.toBeNull();

    await textarea.fill("");
    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await page.mouse.move(secondOptionBox!.x + 8, secondOptionBox!.y + secondOptionBox!.height / 2);

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toHaveAttribute("aria-selected", "true");
    await textarea.press("Tab");
    await expect(composer.getByText("src/App.tsx", { exact: true })).toBeVisible();
  });

  test("composer file search is scoped to the active session project", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();

    const args = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSearchWorkspaceFilesArgs;
    });

    expect(args).toMatchObject({ query: "src", sessionId, workingDir: "/Users/cabbos/project/forge" });
  });

  test("composer file search sends the explicit workspace path for restored sessions", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript(({ sessionId, projectPath }) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      window.localStorage.setItem("forge-working-dir", projectPath);
    }, { sessionId, projectPath });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/DemoApp\.tsx/ })).toBeVisible();
    await expect(page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ })).toHaveCount(0);

    const args = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSearchWorkspaceFilesArgs;
    });

    expect(args).toMatchObject({ query: "src", sessionId, workingDir: projectPath });
  });

  test("composer sends selected capabilities as structured intent", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("/");
    await textarea.press("ArrowDown");
    await textarea.press("Enter");
    await expect(page.getByTestId("composer-lane").getByText("/fix", { exact: true })).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toBeVisible();
    await textarea.press("Tab");
    await expect(page.getByTestId("composer-lane").getByText("src/App.tsx", { exact: true })).toBeVisible();

    await textarea.fill("按钮没有反应");
    await textarea.press("Enter");

    const sendArgs = await expectLastSendInputArgs(page, {
      sessionId,
      capabilities: [
        { kind: "slash_command", command: "/fix" },
        { kind: "file_reference", path: "src/App.tsx" },
      ],
    });
    const sentText = String(sendArgs.text);
    expect(sentText).toContain("按钮没有反应");
    expect(sentText).not.toContain("/fix");
  });

  test("composer keeps active tool state quiet and explicit", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const surface = page.getByTestId("composer-surface");
    const fileButton = composer.getByRole("button", { name: "引用文件" });
    const modelButton = composer.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ });

    await expect(surface).toHaveAttribute("data-menu-open", "false");
    await expect(fileButton).toHaveAttribute("data-active", "false");

    await fileButton.click();
    await expect(surface).toHaveAttribute("data-menu-open", "true");
    await expect(fileButton).toHaveAttribute("data-active", "true");
    await expect(fileButton).toHaveText("");

    await page.keyboard.press("Escape");
    await modelButton.click();
    await expect(surface).toHaveAttribute("data-menu-open", "true");
    await expect(modelButton).toHaveAttribute("data-active", "true");
  });

  test("composer surface uses claude-style conversation proportions", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const textarea = surface.querySelector<HTMLTextAreaElement>("textarea");
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const send = surface.querySelector<HTMLElement>("[data-testid='composer-send']");
      const style = getComputedStyle(surface);
      const textareaStyle = textarea ? getComputedStyle(textarea) : null;
      const toolbarStyle = toolbar ? getComputedStyle(toolbar) : null;
      const sendRect = send?.getBoundingClientRect();
      return {
        surfaceHeight: Math.round(surface.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        shadow: style.boxShadow,
        minInputHeight: textareaStyle ? Number.parseFloat(textareaStyle.minHeight) : 0,
        lineHeight: textareaStyle ? Number.parseFloat(textareaStyle.lineHeight) : 0,
        toolbarHeight: toolbar ? Math.round(toolbar.getBoundingClientRect().height) : 0,
        toolbarBorderTopWidth: toolbarStyle ? Math.round(Number.parseFloat(toolbarStyle.borderTopWidth)) : -1,
        toolbarBackground: toolbarStyle?.backgroundColor ?? "",
        toolbarPaddingBottom: toolbarStyle ? Number.parseFloat(toolbarStyle.paddingBottom) : 0,
        sendWidth: sendRect ? Math.round(sendRect.width) : 0,
        sendHeight: sendRect ? Math.round(sendRect.height) : 0,
      };
    });

    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.surfaceHeight).toBeGreaterThanOrEqual(102);
    expect(metrics.surfaceHeight).toBeLessThanOrEqual(110);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.shadow).not.toBe("none");
    expect(metrics.minInputHeight).toBeGreaterThanOrEqual(44);
    expect(metrics.minInputHeight).toBeLessThanOrEqual(46);
    expect(metrics.lineHeight).toBe(24);
    expect(metrics.toolbarHeight).toBeGreaterThanOrEqual(34);
    expect(metrics.toolbarHeight).toBeLessThanOrEqual(38);
    expect(metrics.toolbarBorderTopWidth).toBe(0);
    expect(metrics.toolbarBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics.toolbarPaddingBottom).toBe(8);
    expect(metrics.sendWidth).toBe(30);
    expect(metrics.sendHeight).toBe(30);
  });

  test("composer controls feel like a quiet conversation composer rail", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src/components/session");
    const fileOption = page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ });
    await expect(fileOption).toBeVisible();
    await fileOption.click();

    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    await expect(page.getByRole("menu")).toBeVisible();

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const chip = surface.querySelector<HTMLElement>(".forge-composer-chip");
      const remove = surface.querySelector<HTMLElement>(".forge-composer-chip-remove");
      const menu = document.querySelector<HTMLElement>(".forge-composer-model-menu");
      const activeOption = menu?.querySelector<HTMLElement>("[role='menuitemradio'][aria-checked='true']");
      const currentBadge = menu?.querySelector<HTMLElement>("[data-testid='composer-model-current-badge']");
      if (!toolbar || !chip || !remove || !menu || !activeOption || !currentBadge) return null;
      const toolbarStyle = getComputedStyle(toolbar);
      const chipStyle = getComputedStyle(chip);
      const removeStyle = getComputedStyle(remove);
      const activeStyle = getComputedStyle(activeOption);
      const badgeStyle = getComputedStyle(currentBadge);
      const removeRect = remove.getBoundingClientRect();
      const badgeRect = currentBadge.getBoundingClientRect();
      return {
        toolbarBorderTopWidth: Math.round(Number.parseFloat(toolbarStyle.borderTopWidth)),
        toolbarBackground: toolbarStyle.backgroundColor,
        chipBackground: chipStyle.backgroundColor,
        chipBorder: chipStyle.borderTopColor,
        removeWidth: Math.round(removeRect.width),
        removeHeight: Math.round(removeRect.height),
        removeBorder: removeStyle.borderTopColor,
        removeRadius: Number.parseFloat(removeStyle.borderTopLeftRadius),
        activeOptionBackground: activeStyle.backgroundColor,
        activeOptionBorder: activeStyle.borderTopColor,
        badgeHeight: Math.round(badgeRect.height),
        badgeBorder: badgeStyle.borderTopColor,
        badgeRadius: Number.parseFloat(badgeStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.toolbarBorderTopWidth).toBe(0);
    expect(metrics!.toolbarBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.chipBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.chipBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removeWidth).toBe(18);
    expect(metrics!.removeHeight).toBe(18);
    expect(metrics!.removeBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removeRadius).toBeLessThanOrEqual(6);
    expect(metrics!.activeOptionBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.activeOptionBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.badgeHeight).toBe(18);
    expect(metrics!.badgeBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.badgeRadius).toBeLessThanOrEqual(6);
  });

  test("composer suggestion menu and selected references stay visually bounded", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("/");
    const commandMenu = page.getByTestId("composer-command-menu");
    await expect(commandMenu).toBeVisible();

    const menuMetrics = await commandMenu.evaluate((node) => {
      const option = node.querySelector<HTMLElement>('[role="option"]');
      const style = getComputedStyle(node);
      return {
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        optionHeight: option ? Math.round(option.getBoundingClientRect().height) : 0,
      };
    });

    expect(menuMetrics.radius).toBeLessThanOrEqual(8);
    expect(menuMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(menuMetrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(menuMetrics.optionHeight).toBeGreaterThanOrEqual(32);

    await page.keyboard.press("Escape");
    await textarea.fill("@src/components/session");
    const fileOption = page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ });
    await expect(fileOption).toBeVisible();
    await fileOption.click();

    const chipMetrics = await page.locator(".forge-composer-chip").first().evaluate((node) => {
      const style = getComputedStyle(node);
      const label = node.querySelector<HTMLElement>(".forge-composer-chip-label");
      const labelStyle = label ? getComputedStyle(label) : null;
      return {
        chipWidth: Math.round(node.getBoundingClientRect().width),
        maxWidth: Number.parseFloat(style.maxWidth),
        overflow: labelStyle?.overflow ?? "",
        textOverflow: labelStyle?.textOverflow ?? "",
        whiteSpace: labelStyle?.whiteSpace ?? "",
      };
    });

    expect(chipMetrics.chipWidth).toBeLessThanOrEqual(300);
    expect(chipMetrics.maxWidth).toBeLessThanOrEqual(300);
    expect(chipMetrics.overflow).toBe("hidden");
    expect(chipMetrics.textOverflow).toBe("ellipsis");
    expect(chipMetrics.whiteSpace).toBe("nowrap");
  });

  test("composer chip tray caps dense long references inside the input surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    const denseReferencePaths = [
      "src/features/deep-context/adapters/anthropic-session-stream-router.ts",
      "src/features/deep-context/adapters/openai-compatible-stream-router.ts",
      "src/features/deep-context/components/RunEvidenceTimeline.tsx",
      "src/features/deep-context/components/ProjectArchiveInspector.tsx",
      "src/features/deep-context/lib/workspace-boundary-policy.ts",
      "src/features/deep-context/lib/markdown-diagram-normalizer.ts",
    ];

    for (const path of denseReferencePaths) {
      await textarea.fill("@deep-context");
      await expect(page.getByTestId("composer-command-menu")).toBeVisible();
      const option = page.getByRole("option", { name: new RegExp(path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")) });
      await expect(option).toBeVisible();
      await option.scrollIntoViewIfNeeded();
      await option.click();
      await expect(textarea).toHaveValue("");
    }

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const chips = surface.querySelector<HTMLElement>(".forge-composer-chips");
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      if (!chips || !toolbar) return null;
      const chipsStyle = getComputedStyle(chips);
      const surfaceRect = surface.getBoundingClientRect();
      const chipsRect = chips.getBoundingClientRect();
      const toolbarRect = toolbar.getBoundingClientRect();
      return {
        chipCount: chips.querySelectorAll(".forge-composer-chip").length,
        overflowY: chipsStyle.overflowY,
        maxHeight: Math.round(Number.parseFloat(chipsStyle.maxHeight)),
        chipsClientHeight: Math.round(chips.clientHeight),
        chipsScrollHeight: Math.round(chips.scrollHeight),
        chipsWidth: Math.round(chipsRect.width),
        surfaceWidth: Math.round(surfaceRect.width),
        surfaceHeight: Math.round(surfaceRect.height),
        toolbarBottom: Math.round(toolbarRect.bottom),
        surfaceBottom: Math.round(surfaceRect.bottom),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.chipCount).toBe(6);
    expect(metrics!.overflowY).toBe("auto");
    expect(metrics!.maxHeight).toBeLessThanOrEqual(68);
    expect(metrics!.chipsScrollHeight).toBeGreaterThan(metrics!.chipsClientHeight);
    expect(metrics!.chipsWidth).toBeLessThanOrEqual(metrics!.surfaceWidth);
    expect(metrics!.surfaceHeight).toBeLessThanOrEqual(196);
    expect(metrics!.toolbarBottom).toBeLessThanOrEqual(metrics!.surfaceBottom);
  });

  test("composer remains bounded in a narrow desktop window with dense context", async ({ page }) => {
    await page.setViewportSize({ width: 760, height: 620 });
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "开始新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    const denseReferencePaths = [
      "src/features/deep-context/adapters/anthropic-session-stream-router.ts",
      "src/features/deep-context/adapters/openai-compatible-stream-router.ts",
      "src/features/deep-context/components/RunEvidenceTimeline.tsx",
      "src/features/deep-context/components/ProjectArchiveInspector.tsx",
      "src/features/deep-context/lib/workspace-boundary-policy.ts",
      "src/features/deep-context/lib/markdown-diagram-normalizer.ts",
    ];

    for (const path of denseReferencePaths) {
      await textarea.fill("@deep-context");
      await expect(page.getByTestId("composer-command-menu")).toBeVisible();
      const option = page.getByRole("option", { name: new RegExp(path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")) });
      await expect(option).toBeVisible();
      await option.scrollIntoViewIfNeeded();
      await option.click();
    }
    await textarea.fill(Array.from({ length: 18 }, (_, index) => `第 ${index + 1} 行：继续描述细节。`).join("\n"));

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const controlCluster = surface.querySelector<HTMLElement>("[data-testid='composer-control-cluster']");
      const toolCluster = surface.querySelector<HTMLElement>("[data-testid='composer-tool-cluster']");
      const model = surface.querySelector<HTMLElement>("[data-testid='composer-model-chip']");
      const send = surface.querySelector<HTMLElement>("[data-testid='composer-send']");
      const textarea = surface.querySelector<HTMLTextAreaElement>("textarea");
      if (!toolbar || !controlCluster || !toolCluster || !model || !send || !textarea) return null;
      const surfaceRect = surface.getBoundingClientRect();
      const toolbarRect = toolbar.getBoundingClientRect();
      const controlRect = controlCluster.getBoundingClientRect();
      const toolRect = toolCluster.getBoundingClientRect();
      const modelRect = model.getBoundingClientRect();
      const sendRect = send.getBoundingClientRect();
      const toolbarStyle = getComputedStyle(toolbar);
      return {
        surfaceWidth: Math.round(surfaceRect.width),
        surfaceHeight: Math.round(surfaceRect.height),
        toolbarWrap: toolbarStyle.flexWrap,
        toolbarHeight: Math.round(toolbarRect.height),
        controlRight: Math.round(controlRect.right - surfaceRect.right),
        toolLeft: Math.round(toolRect.left - surfaceRect.left),
        modelWidth: Math.round(modelRect.width),
        sendRight: Math.round(sendRect.right - surfaceRect.right),
        inputHeight: Math.round(textarea.getBoundingClientRect().height),
        canScrollInside: textarea.scrollHeight > textarea.clientHeight,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.surfaceWidth).toBeLessThanOrEqual(500);
    expect(metrics!.surfaceHeight).toBeLessThanOrEqual(250);
    expect(metrics!.toolbarWrap).toBe("wrap");
    expect(metrics!.toolbarHeight).toBeLessThanOrEqual(76);
    expect(metrics!.toolLeft).toBeGreaterThanOrEqual(16);
    expect(metrics!.controlRight).toBeLessThanOrEqual(-16);
    expect(metrics!.sendRight).toBeLessThanOrEqual(-16);
    expect(metrics!.modelWidth).toBeLessThanOrEqual(188);
    expect(metrics!.inputHeight).toBeLessThanOrEqual(128);
    expect(metrics!.canScrollInside).toBe(true);
  });

  test("composer model menu floats above the composer instead of being clipped by it", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    const menu = page.getByRole("menu");
    await expect(menu).toBeVisible();

    const metrics = await menu.evaluate((node) => {
      const menuRect = node.getBoundingClientRect();
      const surface = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const surfaceRect = surface?.getBoundingClientRect();
      const hit = document.elementFromPoint(menuRect.left + 12, menuRect.top + 12);
      return {
        menuBottom: Math.round(menuRect.bottom),
        surfaceTop: surfaceRect ? Math.round(surfaceRect.top) : 0,
        topHitIsMenu: hit === node || Boolean(hit?.closest("[role='menu']")),
      };
    });

    expect(metrics.menuBottom).toBeLessThanOrEqual(metrics.surfaceTop - 4);
    expect(metrics.topHitIsMenu).toBe(true);
  });

  test("composer menus close when focus moves back to the transcript", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const surface = page.getByTestId("composer-surface");
    const slash = composer.getByRole("button", { name: "常用请求" });

    await slash.click();
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();
    await page.getByTestId("message-lane").click();

    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expect(surface).toHaveAttribute("data-menu-open", "false");
    await expect(slash).toHaveAttribute("data-active", "false");
  });

  test("composer only keeps one floating menu open at a time", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const slash = composer.getByRole("button", { name: "常用请求" });
    const model = composer.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ });

    await slash.click();
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();
    await model.click();

    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expect(model).toHaveAttribute("aria-expanded", "true");
    await expect(slash).toHaveAttribute("data-active", "false");
  });
});

test.describe("Timeline Composer", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  
  
    test("empty workbench entry cards tune the composer for beginners and existing projects", async ({ page }) => {
      const main = page.getByRole("main");
      const textbox = main.getByTestId("empty-start-composer").getByRole("textbox");
  
      await main.getByTestId("empty-entry-new-tool").click();
      await expect(textbox).toBeFocused();
      await expect(textbox).toHaveAttribute("placeholder", /记录喝水次数/);
  
      await main.getByTestId("empty-entry-existing-project").click();
      await expect(textbox).toBeFocused();
      await expect(textbox).toHaveAttribute("placeholder", /当前项目/);
    });
  
    test("empty workbench composer send uses the same compact ready material", async ({ page }) => {
      await page.goto("http://localhost:1420");
  
      const composer = page.getByTestId("empty-start-composer");
      const send = composer.getByRole("button", { name: "发送并开始" });
      await expect(send).toBeDisabled();
  
      const disabledMetrics = await send.evaluate((node) => {
        const style = getComputedStyle(node);
        return {
          cursor: style.cursor,
          opacity: style.opacity,
        };
      });
      expect(disabledMetrics.cursor).toBe("default");
      expect(Number.parseFloat(disabledMetrics.opacity)).toBeLessThan(1);
  
      await composer.getByRole("textbox").fill("继续优化当前页面体验");
      await expect(send).toBeEnabled();
      await expect(send).toHaveAttribute("data-ready", "true");
      await expect.poll(async () => send.evaluate((node) => {
        const style = getComputedStyle(node);
        return {
          borderReady: style.borderTopColor !== "rgba(0, 0, 0, 0)",
          shadowReady: style.boxShadow !== "none",
        };
      })).toEqual({ borderReady: true, shadowReady: true });
  
      const readyMetrics = await send.evaluate((node) => {
        const style = getComputedStyle(node);
        const composer = node.closest("[data-testid='empty-start-composer']");
        const input = composer?.querySelector<HTMLElement>(".forge-empty-composer-input");
        return {
          width: Math.round(node.getBoundingClientRect().width),
          height: Math.round(node.getBoundingClientRect().height),
          background: style.backgroundColor,
          borderColor: style.borderTopColor,
          boxShadow: style.boxShadow,
          radius: Number.parseFloat(style.borderTopLeftRadius),
          inputMinHeight: input ? Number.parseFloat(getComputedStyle(input).minHeight) : 0,
        };
      });
  
      expect(readyMetrics.width).toBe(32);
      expect(readyMetrics.height).toBe(32);
      expect(readyMetrics.background).not.toBe("rgb(184, 138, 86)");
      expect(readyMetrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
      expect(readyMetrics.boxShadow).not.toBe("none");
      expect(readyMetrics.radius).toBeLessThanOrEqual(8);
      expect(readyMetrics.inputMinHeight).toBeGreaterThanOrEqual(88);
    });
  
    test("empty workbench hints fill the bottom composer without sending", async ({ page }) => {
      const main = page.getByRole("main");
      const hints = main.getByTestId("empty-middle-hints");
      await expect(hints).toBeVisible();
      await hints.getByRole("button", { name: "检查这个项目能不能运行" }).click();
  
      await expect(main.getByTestId("empty-start-composer").getByRole("textbox")).toHaveValue("检查这个项目能不能运行");
      await expectNoSendInput(page);
    });
  
    test("creating a session shows chat input", async ({ page }) => {
      await page.goto("http://localhost:1420");
      // Click new session button
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      // Input should appear
      await expect(page.locator("textarea")).toBeVisible();
      await expect(page.getByRole("main").getByText("运行中", { exact: true })).toHaveCount(0);
      const composer = page.getByTestId("composer-lane");
      await expect(composer).toBeVisible();
      await expect(composer.getByRole("button", { name: "引用文件" })).toBeVisible();
      await expect(composer.getByRole("button", { name: "常用请求" })).toBeVisible();
      await expect(page.getByRole("button", { name: "我想做一个番茄钟小工具，可以开始、暂停、重置。" })).toHaveCount(0);
      await expect(page.getByRole("button", { name: "我想做一个记账小工具，先能记录一笔收入或支出。" })).toHaveCount(0);
      await expect(page.getByRole("button", { name: "我想做一个文案小工具，输入主题后生成一版短文案。" })).toHaveCount(0);
      await expect(page.getByRole("main").getByText("可以继续描述任务")).toHaveCount(0);
    });
  
    test("stopped composer presents a quiet resume state", async ({ page }) => {
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
  
      const composer = page.getByTestId("composer-surface");
      await expect(composer).toHaveAttribute("data-state", "paused");
      await expect(page.locator("textarea")).toBeDisabled();
      await expect(page.locator("textarea")).toHaveAttribute("placeholder", "这个会话已停止，可以继续后再发送");
      await expect(page.getByRole("button", { name: "继续会话" })).toBeVisible();
  
      const metrics = await composer.evaluate((node) => {
        const surface = node as HTMLElement;
        const textarea = surface.querySelector<HTMLTextAreaElement>(".forge-composer-textarea");
        const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
        const resume = surface.querySelector<HTMLElement>(".forge-composer-resume");
        const surfaceStyle = getComputedStyle(surface);
        const textareaStyle = textarea ? getComputedStyle(textarea) : null;
        const resumeStyle = resume ? getComputedStyle(resume) : null;
        return {
          background: surfaceStyle.backgroundColor,
          borderColor: surfaceStyle.borderTopColor,
          textareaMinHeight: textareaStyle ? Math.round(Number.parseFloat(textareaStyle.minHeight)) : 0,
          textareaCursor: textareaStyle?.cursor ?? "",
          toolbarHeight: toolbar ? Math.round(toolbar.getBoundingClientRect().height) : 0,
          resumeHeight: resume ? Math.round(resume.getBoundingClientRect().height) : 0,
          resumeRadius: resumeStyle ? Number.parseFloat(resumeStyle.borderTopLeftRadius) : 0,
          resumeBorder: resumeStyle ? resumeStyle.borderColor : "",
        };
      });
  
      expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics.textareaMinHeight).toBeLessThanOrEqual(36);
      expect(metrics.textareaCursor).toBe("default");
      expect(metrics.toolbarHeight).toBeLessThanOrEqual(36);
      expect(metrics.resumeHeight).toBe(32);
      expect(metrics.resumeRadius).toBeLessThanOrEqual(8);
      expect(metrics.resumeBorder).not.toBe("rgba(0, 0, 0, 0)");
    });
  
    test("composer floats in a transparent frame with bottom breathing room", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const frameMetrics = await page.evaluate(() => {
        const composerFrameNode = document.querySelector("[data-testid='composer-frame']");
        const composerFrame = composerFrameNode?.getBoundingClientRect();
        const composerLane = document.querySelector("[data-testid='composer-lane']")?.getBoundingClientRect();
        if (!composerFrameNode || !composerFrame || !composerLane) return null;
        const frameStyle = getComputedStyle(composerFrameNode);
  
        return {
          frameBackground: frameStyle.backgroundColor,
          frameBorderTop: Math.round(Number.parseFloat(frameStyle.borderTopWidth)),
          frameShadow: frameStyle.boxShadow,
          frameBackdrop: frameStyle.backdropFilter || frameStyle.getPropertyValue("-webkit-backdrop-filter"),
          composerTop: Math.round(composerLane.top - composerFrame.top),
          composerBottom: Math.round(composerFrame.bottom - composerLane.bottom),
        };
      });
  
      expect(frameMetrics).not.toBeNull();
      expect(frameMetrics!.frameBackground).toBe("rgba(0, 0, 0, 0)");
      expect(frameMetrics!.frameBorderTop).toBe(0);
      expect(frameMetrics!.frameShadow).toBe("none");
      expect(frameMetrics!.frameBackdrop).toBe("none");
      expect(frameMetrics!.composerTop).toBe(14);
      expect(frameMetrics!.composerBottom).toBe(24);
    });
  
    test("composer floating menus sit above the editor without overlap", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const composer = page.getByTestId("composer-lane");
      await composer.getByRole("button", { name: "常用请求" }).click();
      const menu = page.getByTestId("composer-command-menu");
      await expect(menu).toBeVisible();
  
      const metrics = await page.evaluate(() => {
        const root = document.documentElement;
        const menu = document.querySelector("[data-testid='composer-command-menu']");
        const surface = document.querySelector("[data-testid='composer-surface']");
        const option = document.querySelector("[data-testid='composer-command-menu'] [role='option']");
        if (!menu || !surface || !option) return null;
        const menuRect = menu.getBoundingClientRect();
        const surfaceRect = surface.getBoundingClientRect();
        const menuStyle = getComputedStyle(menu);
        return {
          gapToken: getComputedStyle(root).getPropertyValue("--forge-floating-gap").trim(),
          menuBottomGap: Math.round(surfaceRect.top - menuRect.bottom),
          menuWidth: Math.round(menuRect.width),
          surfaceWidth: Math.round(surfaceRect.width),
          menuShadow: menuStyle.boxShadow,
          optionHeight: Math.round(option.getBoundingClientRect().height),
          radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.gapToken).toBe("8px");
      expect(metrics!.menuBottomGap).toBeGreaterThanOrEqual(4);
      expect(metrics!.menuBottomGap).toBeLessThanOrEqual(8);
      expect(metrics!.menuWidth).toBeLessThanOrEqual(metrics!.surfaceWidth);
      expect(metrics!.menuWidth).toBeLessThanOrEqual(560);
      expect(metrics!.menuShadow).not.toContain("0px 25px");
      expect(metrics!.optionHeight).toBe(34);
      expect(metrics!.radius).toBeLessThanOrEqual(8);
    });
  
    test("composer model menu uses a grounded desktop picker surface", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const modelButton = page.getByTestId("composer-model-chip");
      await modelButton.click();
      const menu = page.getByRole("menu");
      await expect(menu).toBeVisible();
      await expect(modelButton).toHaveAttribute("aria-expanded", "true");
  
      const metrics = await page.evaluate(() => {
        const root = document.documentElement;
        const menu = document.querySelector("[role='menu']");
        const button = document.querySelector("[data-testid='composer-model-chip']");
        const surface = document.querySelector("[data-testid='composer-surface']");
        const active = menu?.querySelector("[role='menuitemradio'][aria-checked='true']");
        const firstOption = menu?.querySelector("[role='menuitemradio']");
        const heading = menu?.querySelector(".forge-menu-heading");
        if (!menu || !button || !surface || !active || !firstOption || !heading) return null;
        const menuRect = menu.getBoundingClientRect();
        const buttonRect = button.getBoundingClientRect();
        const surfaceRect = surface.getBoundingClientRect();
        const menuStyle = getComputedStyle(menu);
        const activeStyle = getComputedStyle(active);
        return {
          gapToken: getComputedStyle(root).getPropertyValue("--forge-floating-gap").trim(),
          menuBottomGap: Math.round(buttonRect.top - menuRect.bottom),
          surfaceBottomGap: Math.round(surfaceRect.top - menuRect.bottom),
          minWidth: Math.round(Number.parseFloat(menuStyle.minWidth)),
          backdrop: menuStyle.backdropFilter || menuStyle.webkitBackdropFilter,
          shadow: menuStyle.boxShadow,
          background: menuStyle.backgroundColor,
          radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
          optionHeight: Math.round(firstOption.getBoundingClientRect().height),
          activeBorder: Math.round(Number.parseFloat(activeStyle.borderTopWidth)),
          activeBackground: activeStyle.backgroundColor,
          headingHeight: Math.round(heading.getBoundingClientRect().height),
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.gapToken).toBe("8px");
      expect(metrics!.menuBottomGap).toBeGreaterThan(8);
      expect(metrics!.surfaceBottomGap).toBeGreaterThanOrEqual(4);
      expect(metrics!.surfaceBottomGap).toBeLessThanOrEqual(8);
      expect(metrics!.minWidth).toBeGreaterThanOrEqual(300);
      expect(metrics!.backdrop).toContain("blur");
      expect(metrics!.shadow).not.toContain("0px 10px 24px");
      expect(metrics!.background).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics!.radius).toBeLessThanOrEqual(8);
      expect(metrics!.optionHeight).toBeGreaterThanOrEqual(44);
      expect(metrics!.activeBorder).toBe(1);
      expect(metrics!.activeBackground).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics!.headingHeight).toBeLessThanOrEqual(30);
    });
  
    test("composer send states stay compact without primary fill", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const send = page.getByTestId("composer-send");
      await expect(send).toBeDisabled();
      await send.hover({ force: true });
  
      const metrics = await send.evaluate((node) => {
        const style = getComputedStyle(node);
        return {
          background: style.backgroundColor,
          borderColor: style.borderTopColor,
          color: style.color,
          cursor: style.cursor,
        };
      });
  
      expect(metrics.background).toBe("rgba(0, 0, 0, 0)");
      expect(metrics.borderColor).toBe("rgba(0, 0, 0, 0)");
      expect(metrics.color).toBe("rgba(184, 180, 170, 0.48)");
      expect(metrics.cursor).toBe("default");
  
      await page.locator("textarea").fill("继续优化当前界面");
      await expect(send).toBeEnabled();
      await expect(send).toHaveAttribute("data-ready", "true");
      await expect.poll(async () => send.evaluate((node) => {
        const style = getComputedStyle(node);
        return {
          backgroundReady: style.backgroundColor !== "rgba(0, 0, 0, 0)",
          borderReady: style.borderTopColor !== "rgba(0, 0, 0, 0)",
          shadowReady: style.boxShadow !== "none",
        };
      })).toEqual({ backgroundReady: true, borderReady: true, shadowReady: true });
  
      const readyMetrics = await send.evaluate((node) => {
        const style = getComputedStyle(node);
        return {
          background: style.backgroundColor,
          borderColor: style.borderTopColor,
          boxShadow: style.boxShadow,
        };
      });
  
      expect(readyMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
      expect(readyMetrics.background).not.toBe("rgb(184, 138, 86)");
      expect(readyMetrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
      expect(readyMetrics.boxShadow).not.toBe("none");
    });
  
    test("composer command menu keeps keyboard selection visible and compact", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const composer = page.getByTestId("composer-lane");
      await composer.getByRole("button", { name: "常用请求" }).click();
      const menu = page.getByTestId("composer-command-menu");
      await expect(menu).toBeVisible();
      await page.keyboard.press("ArrowDown");
      await page.waitForFunction(() => {
        const selected = document.querySelector("[data-testid='composer-command-menu'] [role='option'][aria-selected='true']");
        if (!selected) return false;
        const rootStyle = getComputedStyle(document.documentElement);
        const style = getComputedStyle(selected);
        return style.backgroundColor === rootStyle.getPropertyValue("--forge-hover").trim() &&
          Math.round(Number.parseFloat(style.borderTopWidth)) === 1;
      });
  
      const metrics = await page.evaluate(() => {
        const menu = document.querySelector("[data-testid='composer-command-menu']");
        const selected = menu?.querySelector("[role='option'][aria-selected='true']");
        const options = Array.from(menu?.querySelectorAll("[role='option']") ?? []);
        if (!menu || !selected || options.length === 0) return null;
        const rootStyle = getComputedStyle(document.documentElement);
        const selectedStyle = getComputedStyle(selected);
        return {
          hoverToken: rootStyle.getPropertyValue("--forge-hover").trim(),
          optionCount: options.length,
          selectedText: selected.textContent ?? "",
          selectedHeight: Math.round(selected.getBoundingClientRect().height),
          selectedBackground: selectedStyle.backgroundColor,
          selectedRadius: Number.parseFloat(selectedStyle.borderTopLeftRadius),
          selectedBorder: Math.round(Number.parseFloat(selectedStyle.borderTopWidth)),
          selectedBorderColor: selectedStyle.borderColor,
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.optionCount).toBeGreaterThanOrEqual(6);
      expect(metrics!.selectedText).toContain("/fix");
      expect(metrics!.selectedHeight).toBe(34);
      expect(metrics!.selectedBackground).toBe(metrics!.hoverToken);
      expect(metrics!.selectedRadius).toBeLessThanOrEqual(8);
      expect(metrics!.selectedBorder).toBe(1);
      expect(metrics!.selectedBorderColor).not.toBe("rgba(0, 0, 0, 0)");
    });
  
    test("composer uses a grounded editor surface instead of a plastic card", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
  
      const surface = page.getByTestId("composer-surface");
      const send = surface.getByRole("button", { name: "发送" });
      await expect(surface).toBeVisible();
      await expect(send).toBeVisible();
      await expect(surface.getByTestId("composer-tool-cluster")).toBeVisible();
      await expect(surface.getByTestId("composer-control-cluster")).toBeVisible();
      await expect(surface.getByTestId("composer-model-indicator")).toBeVisible();
  
      const metrics = await page.evaluate(() => {
        const surface = document.querySelector("[data-testid='composer-surface']");
        const toolbar = document.querySelector("[data-testid='composer-toolbar']");
        const toolCluster = document.querySelector("[data-testid='composer-tool-cluster']");
        const controlCluster = document.querySelector("[data-testid='composer-control-cluster']");
        const model = document.querySelector("[data-testid='composer-model-chip']");
        const send = document.querySelector("[data-testid='composer-send']");
        const tools = Array.from(document.querySelectorAll("[data-testid='composer-tool-button']"));
        if (!surface || !toolbar || !toolCluster || !controlCluster || !model || !send) return null;
        const surfaceStyle = getComputedStyle(surface);
        const modelStyle = getComputedStyle(model);
        const sendStyle = getComputedStyle(send);
        return {
          surfaceBackdrop: surfaceStyle.backdropFilter || surfaceStyle.getPropertyValue("-webkit-backdrop-filter"),
          surfaceOverflow: surfaceStyle.overflow,
          surfaceShadow: surfaceStyle.boxShadow,
          surfaceRadius: Number.parseFloat(surfaceStyle.borderTopLeftRadius),
          toolbarHeight: Math.round(toolbar.getBoundingClientRect().height),
          toolClusterHeight: Math.round(toolCluster.getBoundingClientRect().height),
          controlGap: Math.round(Number.parseFloat(getComputedStyle(controlCluster).columnGap)),
          toolSizes: tools.map((item) => ({
            width: Math.round(item.getBoundingClientRect().width),
            height: Math.round(item.getBoundingClientRect().height),
          })),
          modelRadius: Number.parseFloat(modelStyle.borderTopLeftRadius),
          modelHeight: Math.round(model.getBoundingClientRect().height),
          modelBackground: modelStyle.backgroundColor,
          sendRadius: Number.parseFloat(sendStyle.borderTopLeftRadius),
          sendBackground: sendStyle.backgroundColor,
          sendWidth: Math.round(send.getBoundingClientRect().width),
          sendHeight: Math.round(send.getBoundingClientRect().height),
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.surfaceBackdrop).not.toBe("none");
      expect(metrics!.surfaceOverflow).toBe("hidden");
      expect(metrics!.surfaceShadow).not.toBe("none");
      expect(metrics!.surfaceRadius).toBeLessThanOrEqual(8);
      expect(metrics!.toolbarHeight).toBeLessThanOrEqual(40);
      expect(metrics!.toolClusterHeight).toBeLessThanOrEqual(32);
      expect(metrics!.controlGap).toBeLessThanOrEqual(8);
      expect(metrics!.toolSizes).toEqual([{ width: 30, height: 30 }, { width: 30, height: 30 }]);
      expect(metrics!.modelRadius).toBeLessThanOrEqual(8);
      expect(metrics!.modelHeight).toBe(30);
      expect(metrics!.modelBackground).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics!.sendRadius).toBeLessThanOrEqual(8);
      expect(metrics!.sendBackground).not.toBe("rgb(184, 138, 86)");
      expect(metrics!.sendWidth).toBe(30);
      expect(metrics!.sendHeight).toBe(30);
    });
  
    test("workspace menu uses the shared compact floating surface", async ({ page }) => {
      const sidebar = page.locator("aside").first();
      const trigger = sidebar.getByRole("button", { name: /forge/ });
      await trigger.click();
      const menu = page.getByRole("menu", { name: "项目文件夹" });
      await expect(menu).toBeVisible();
  
      const metrics = await page.evaluate(() => {
        const root = document.documentElement;
        const trigger = document.querySelector("[data-testid='workspace-trigger']");
        const menu = document.querySelector("#workspace-menu");
        const option = menu?.querySelector("[role='menuitemradio'], [role='menuitem']");
        if (!trigger || !menu || !option) return null;
        const triggerRect = trigger.getBoundingClientRect();
        const menuRect = menu.getBoundingClientRect();
        const menuStyle = getComputedStyle(menu);
        return {
          gapToken: getComputedStyle(root).getPropertyValue("--forge-floating-gap").trim(),
          menuTopGap: Math.round(menuRect.top - triggerRect.bottom),
          optionHeight: Math.round(option.getBoundingClientRect().height),
          shadow: menuStyle.boxShadow,
          radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.gapToken).toBe("8px");
      expect(metrics!.menuTopGap).toBe(8);
      expect(metrics!.optionHeight).toBe(34);
      expect(metrics!.shadow).not.toContain("0px 25px");
      expect(metrics!.radius).toBeLessThanOrEqual(8);
    });
});
