import { test, expect } from "@playwright/test";
import { setup, expectLastSendInputArgs } from "./fixtures/app";

test.describe("Timeline Message Flow", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });
  test("app loads and shows empty state", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(page.getByTestId("app-titlebar")).toHaveAttribute("data-tauri-drag-region", "true");
    await expect(page.getByTestId("app-titlebar")).toHaveCSS("height", "64px");
    await expect(main.getByTestId("empty-workbench")).toBeVisible();
    await expect(main.getByTestId("empty-workbench-project")).toContainText("forge");
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByTestId("empty-workbench-action")).toBeVisible();
    await expect(main.getByTestId("empty-entry-new-tool")).toBeVisible();
    await expect(main.getByTestId("empty-entry-existing-project")).toBeVisible();
    await expect(main.getByRole("button", { name: /做个新工具/ })).toBeVisible();
    await expect(main.getByRole("button", { name: /打开已有项目/ })).toBeVisible();
    const emptyMetrics = await main.evaluate((node) => {
      const workbench = node.querySelector<HTMLElement>("[data-testid='empty-workbench']");
      const frame = node.querySelector<HTMLElement>(".forge-empty-composer-frame");
      const composer = node.querySelector<HTMLElement>("[data-testid='empty-start-composer']");
      const project = node.querySelector<HTMLElement>("[data-testid='empty-workbench-project']");
      const action = node.querySelector<HTMLElement>("[data-testid='empty-workbench-action']");
      const style = workbench ? getComputedStyle(workbench) : null;
      const frameStyle = frame ? getComputedStyle(frame) : null;
      const actionStyle = action ? getComputedStyle(action) : null;
      const nodeRect = (node as HTMLElement).getBoundingClientRect();
      const frameRect = frame?.getBoundingClientRect();
      return {
        borderWidth: style?.borderTopWidth ?? "",
        background: style?.backgroundColor ?? "",
        textAlign: style?.textAlign ?? "",
        composerWidth: composer ? Math.round(composer.getBoundingClientRect().width) : 0,
        frameBackground: frameStyle?.backgroundColor ?? "",
        frameBorderTop: frameStyle ? Math.round(Number.parseFloat(frameStyle.borderTopWidth)) : -1,
        frameShadow: frameStyle?.boxShadow ?? "",
        frameBottomGap: frameRect ? Math.round(nodeRect.bottom - frameRect.bottom) : -1,
        frameTop: frameRect ? Math.round(frameRect.top - nodeRect.top) : 0,
        mainHeight: Math.round(nodeRect.height),
        projectHeight: project ? Math.round(project.getBoundingClientRect().height) : 0,
        projectRadius: project ? Number.parseFloat(getComputedStyle(project).borderTopLeftRadius) : 0,
        actionHeight: action ? Math.round(action.getBoundingClientRect().height) : 0,
        actionRadius: actionStyle ? Number.parseFloat(actionStyle.borderTopLeftRadius) : 0,
        actionDisplay: actionStyle?.display ?? "",
      };
    });
    expect(emptyMetrics.borderWidth).toBe("0px");
    expect(emptyMetrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(emptyMetrics.textAlign).toBe("left");
    expect(emptyMetrics.composerWidth).toBeGreaterThanOrEqual(520);
    expect(emptyMetrics.frameBackground).toBe("rgba(0, 0, 0, 0)");
    expect(emptyMetrics.frameBorderTop).toBe(0);
    expect(emptyMetrics.frameShadow).toBe("none");
    expect(emptyMetrics.frameBottomGap).toBeLessThanOrEqual(1);
    expect(emptyMetrics.frameTop).toBeGreaterThan(emptyMetrics.mainHeight * 0.65);
    expect(emptyMetrics.projectHeight).toBe(26);
    expect(emptyMetrics.projectRadius).toBeLessThanOrEqual(8);
    expect(emptyMetrics.actionHeight).toBe(26);
    expect(emptyMetrics.actionRadius).toBeLessThanOrEqual(8);
    expect(["inline-flex", "flex"]).toContain(emptyMetrics.actionDisplay);
    const entryMetrics = await main.evaluate(() => {
      const cards = Array.from(document.querySelectorAll<HTMLElement>("[data-testid^='empty-entry-']"));
      return cards.map((card) => {
        const style = getComputedStyle(card);
        return {
          width: Math.round(card.getBoundingClientRect().width),
          height: Math.round(card.getBoundingClientRect().height),
          borderColor: style.borderTopColor,
          radius: Number.parseFloat(style.borderTopLeftRadius),
        };
      });
    });
    expect(entryMetrics).toHaveLength(2);
    expect(Math.abs(entryMetrics[0].width - entryMetrics[1].width)).toBeLessThanOrEqual(1);
    expect(Math.abs(entryMetrics[0].height - entryMetrics[1].height)).toBeLessThanOrEqual(12);
    expect(entryMetrics[0].borderColor).toBe(entryMetrics[1].borderColor);
    expect(entryMetrics[0].radius).toBeLessThanOrEqual(8);
    await expect(main.locator("img")).toHaveCount(1);
    await expect(main.locator("img.forge-empty-identity-mark")).toHaveCount(1);
    await expect(main.locator("p", { hasText: "从当前对话开始" })).toHaveCount(0);
    await expect(main.getByText("Forge 会带着项目档案，把结果推进到可预览、可检查、可继续。")).toHaveCount(0);
    await expect(main.getByText("当前任务", { exact: true })).toHaveCount(0);
    await expect(main.getByText("交付", { exact: true })).toHaveCount(0);
    await expect(main.getByText("创建一个任务开始")).toHaveCount(0);
  });

  test("empty workbench does not duplicate readiness when start is ready", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(main.getByText("准备开始", { exact: true })).toHaveCount(0);
    await expect(main.getByTestId("start-readiness")).toHaveCount(0);
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByRole("button", { name: "开始新对话" })).toBeVisible();
  });

  test("history dialog searches snapshots and can restore or delete a session", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error mock
      window.__mockSessionStoreStats = {
        total_snapshots: 2,
        corrupted_snapshots: 0,
        total_bytes: 4096,
        oldest_updated_at_ms: Date.now() - 120_000,
        newest_updated_at_ms: Date.now() - 30_000,
        by_provider: { deepseek: 1, anthropic: 1 },
        by_workspace: { "/Users/cabbos/project/forge": 2 },
      };
      // @ts-expect-error mock
      window.__mockSessionStoreSearchResults = [
        {
          session_id: "history-launch-plan",
          provider: "deepseek",
          model: "deepseek-v4-flash[1m]",
          working_dir: "/Users/cabbos/project/forge",
          summary: "Launch service hardening plan",
          created_at_ms: Date.now() - 120_000,
          updated_at_ms: Date.now() - 30_000,
          message_count: 8,
        },
        {
          session_id: "history-memory-notes",
          provider: "anthropic",
          model: "claude-sonnet-4.6",
          working_dir: "/Users/cabbos/project/forge",
          summary: "Memory profile notes",
          created_at_ms: Date.now() - 240_000,
          updated_at_ms: Date.now() - 180_000,
          message_count: 3,
        },
      ];
    });

    await page.getByRole("button", { name: "历史", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "历史" })).toBeVisible();
    await expect(dialog.getByText("2 个快照")).toBeVisible();

    await dialog.getByRole("button", { name: "导出历史" }).click();
    const exported = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastExportSessionStoreCalled;
    });
    expect(exported).toBe(true);
    await expect(dialog.getByText("已导出 2 个快照")).toBeVisible();

    await dialog.getByLabel("服务筛选").selectOption("anthropic");
    await expect(dialog.getByText("Memory profile notes")).toBeVisible();
    await expect(dialog.getByText("Launch service hardening plan")).toHaveCount(0);
    await dialog.getByLabel("服务筛选").selectOption("all");

    await dialog.getByPlaceholder("搜索摘要、模型、项目路径").fill("launch");
    await expect(dialog.getByText("Launch service hardening plan")).toBeVisible();
    await expect(dialog.getByText("Memory profile notes")).toHaveCount(0);
    const searchArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSearchSessionStoreArgs;
    });
    expect(searchArgs.query).toBe("launch");

    await dialog.getByRole("button", { name: "重命名 history-launch-plan" }).click();
    await dialog.getByLabel("会话名称").fill("Launch plan renamed");
    await dialog.getByRole("button", { name: "保存重命名 history-launch-plan" }).click();
    const renamedArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastRenamedSessionSnapshotArgs;
    });
    expect(renamedArgs).toEqual({
      sessionId: "history-launch-plan",
      summary: "Launch plan renamed",
    });
    await expect(dialog.getByText("Launch plan renamed")).toBeVisible();

    await dialog.getByRole("button", { name: "恢复 history-launch-plan" }).click();
    const resumedId = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastResumedSessionId;
    });
    expect(resumedId).toBe("history-launch-plan");

    await page.getByRole("button", { name: "历史", exact: true }).click();
    await dialog.getByRole("button", { name: "清理旧记录" }).click();
    const pruneArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastPruneSessionStoreArgs;
    });
    expect(pruneArgs.keepRecent).toBe(50);
    await expect(dialog.getByText("已清理 0 个快照")).toBeVisible();

    await dialog.getByPlaceholder("搜索摘要、模型、项目路径").fill("launch");
    await dialog.getByRole("button", { name: "删除 history-launch-plan" }).click();
    const deletedId = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastDeletedSessionId;
    });
    expect(deletedId).toBe("history-launch-plan");
  });

  test("settings dialog close button closes the modal", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "设置" })).toBeVisible();

    await dialog.getByRole("button", { name: "关闭" }).click();
    await expect(dialog).toHaveCount(0);
  });

  test("settings tools panel manages permission rules", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error mock
      window.__mockPermissionRules = [
        {
          tool_name: "run_shell",
          decision: "deny",
          created_at: "2026-06-16T00:00:00.000Z",
        },
        {
          tool_name: "write_to_file",
          decision: "allow",
          created_at: "2026-06-16T00:00:00.000Z",
        },
      ];
    });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "工具" }).click();

    await expect(dialog.getByRole("heading", { name: "工具" })).toBeVisible();
    const panel = dialog.getByTestId("settings-permissions-panel");
    await expect(panel).toBeVisible();
    await expect(panel.getByTestId("settings-permission-rule-write_to_file")).toContainText("允许");
    await expect(panel.getByTestId("settings-permission-rule-run_shell")).toContainText("拒绝");

    await panel.getByRole("button", { name: "拒绝 write_to_file" }).click();
    const setArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSetPermissionRuleArgs;
    });
    expect(setArgs).toMatchObject({ toolName: "write_to_file", decision: "deny" });
    await expect(panel.getByTestId("settings-permission-rule-write_to_file")).toContainText("拒绝");

    await panel.getByRole("button", { name: "重置 write_to_file" }).click();
    const resetArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastResetPermissionRuleArgs;
    });
    expect(resetArgs).toMatchObject({ toolName: "write_to_file" });
    await expect(panel.getByTestId("settings-permission-rule-write_to_file")).toContainText("默认");
  });

  test("empty workbench can start directly from a prompt", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");

    const composer = page.getByTestId("empty-start-composer");
    await composer.getByRole("textbox").fill("做一个可以记录收支的小工具");
    await composer.getByRole("textbox").press("Enter");

    await expect(page.getByTestId("user-message").last()).toContainText("做一个可以记录收支的小工具");
    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    const checkpointArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateProjectCheckpointArgs;
    });
    const sendArgs = await expectLastSendInputArgs(page, { sessionId });
    const sentText = String(sendArgs.text);
    expect(createArgs.workingDir).toBe("/Users/cabbos/project/forge");
    expect(checkpointArgs.sessionId).toBe(sessionId);
    expect(checkpointArgs.workingDir).toBe("/Users/cabbos/project/forge");
    expect(sentText).toContain("Forge 第一闭环提示");
    expect(sentText).toContain("当前工作空间：/Users/cabbos/project/forge");
    expect(sentText).toContain("所有文件搜索、修改、预览、检查点和验证都必须限定在当前工作空间。");
    expect(sentText).toContain("如果预览端口来自其他项目，必须提示冲突，不要打开别的项目。");
    expect(sentText).toContain("本地网页小工具");
    expect(sentText).toContain("React/Vite");
    expect(sentText).toContain("少问问题");
    expect(sentText).toContain("做一个可以记录收支的小工具");
  });

  test("vague beginner idea is shaped before making", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");

    const composer = page.getByTestId("empty-start-composer");
    await composer.getByRole("textbox").fill("我想做个能记录客户的东西，最好能提醒我，还能导出表格，但我也不知道怎么说。");
    await composer.getByRole("textbox").press("Enter");

    const sendArgs = await expectLastSendInputArgs(page, { sessionId });
    const sentText = String(sendArgs.text);
    expect(sentText).toContain("Forge 需求梳理提示");
    expect(sentText).toContain("只问一个轻确认问题");
    expect(sentText).toContain("先不做");
    expect(sentText).not.toContain("请优先推进到一个可预览的第一版");
  });

  test("start readiness stays compact in an empty session", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const readiness = page.getByTestId("start-readiness-panel");
    await expect(readiness).toBeVisible();
    await expect(readiness).toHaveCSS("border-radius", "8px");
    await expect(readiness.getByTestId("start-readiness-row")).toHaveCount(0);
    await expect(readiness.getByText("当前项目", { exact: true })).toHaveCount(0);
    await expect(readiness.getByText("模型密钥", { exact: true })).toHaveCount(0);
    await expect(readiness.getByText("预览", { exact: true })).toHaveCount(0);
    await expect(readiness.getByText("检查点", { exact: true })).toHaveCount(0);
    await expect(readiness.getByRole("button", { name: "刷新准备状态" })).toBeVisible();
  });

  test("missing API key is shown as an actionable setup card", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      window.__mockMissingApiKey = true;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await expect(page.getByText("需要配置模型密钥")).toBeVisible();
    await expect(page.getByText("需要配置模型密钥")).toHaveCount(1);
    const setupPanel = page.getByTestId("message-panel").filter({ hasText: "需要配置模型密钥" });
    await expect(setupPanel).toHaveAttribute("role", "status");
    await expect(setupPanel.getByTestId("missing-api-key-card")).toBeVisible();
    const setupMetrics = await setupPanel.evaluate((node) => {
      const body = node.querySelector<HTMLElement>("[data-testid='missing-api-key-card']");
      const action = node.querySelector<HTMLElement>("[data-testid='missing-api-key-action']");
      const style = getComputedStyle(node);
      const actionStyle = action ? getComputedStyle(action) : null;
      return {
        width: Math.round(node.getBoundingClientRect().width),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        border: style.borderTopColor,
        bodyHeight: body ? Math.round(body.getBoundingClientRect().height) : 0,
        actionHeight: action ? Math.round(action.getBoundingClientRect().height) : 0,
        actionRadius: action ? Number.parseFloat(getComputedStyle(action).borderTopLeftRadius) : 0,
        actionBackground: actionStyle?.backgroundColor ?? "",
        actionBorder: actionStyle?.borderTopColor ?? "",
        actionBorderColor: actionStyle?.borderColor ?? "",
      };
    });
    expect(setupMetrics.width).toBeLessThanOrEqual(620);
    expect(setupMetrics.radius).toBeLessThanOrEqual(8);
    expect(setupMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(setupMetrics.border).not.toBe("rgba(0, 0, 0, 0)");
    expect(setupMetrics.bodyHeight).toBeLessThanOrEqual(38);
    expect(setupMetrics.actionHeight).toBe(28);
    expect(setupMetrics.actionRadius).toBeLessThanOrEqual(8);
    expect(setupMetrics.actionBackground).not.toBe("rgb(184, 138, 86)");
    expect(setupMetrics.actionBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(setupMetrics.actionBorderColor).not.toBe("rgba(0, 0, 0, 0)");
    await page.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "模型服务" })).toBeVisible();
    await page.getByRole("button", { name: /本机数据/ }).click();
    await expect(page.getByRole("heading", { name: "本机数据" })).toBeVisible();
    const localDataRegion = page.getByRole("region", { name: "本机数据" });
    await expect(localDataRegion.getByText("API Key")).toHaveCount(0);
    await expect(localDataRegion.getByText("~/.forge/config.json")).toHaveCount(0);
    await expect(localDataRegion.getByText("IndexedDB")).toHaveCount(0);
  });

  test("session creation errors stay inline and can open settings", async ({ page }) => {
    const dialogs: string[] = [];
    page.on("dialog", async (dialog) => {
      dialogs.push(dialog.message());
      await dialog.dismiss();
    });

    await page.evaluate(() => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "create_session") {
          throw new Error("No DeepSeek API key configured. Open Settings (Cmd+,) to set one.");
        }
        return original?.(cmd, args);
      };
    });

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("status")).toContainText("模型服务还没有可用密钥");
    expect(dialogs).toEqual([]);

    await sidebar.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  });



});
