import { test, expect } from "@playwright/test";
import { setup } from "./fixtures/app";
import { simulateStream } from "./mock-ipc";
import type { AgentA2ATaskProjection } from "../src/lib/protocol";

function workPanelSubtask(
  task: Pick<AgentA2ATaskProjection, "task_id" | "title" | "latest_message"> &
    Partial<AgentA2ATaskProjection>,
): AgentA2ATaskProjection {
  return {
    agent_id: `agent-${task.task_id}`,
    role: "implementer",
    execution_mode: "worktree_worker",
    status: "running",
    messages: [],
    failure_message: null,
    updated_at_ms: Date.now(),
    artifact_count: 0,
    latest_artifact_kind: null,
    latest_artifact_title: null,
    needs_human_review: null,
    reason_codes: [],
    tests_passed: null,
    diff_truncated: null,
    worktree_path: null,
    cleaned_up: null,
    suggested_action: null,
    parent_task_id: null,
    child_task_ids: [],
    created_at_ms: Date.now() - 1_000,
    started_at_ms: Date.now() - 800,
    ended_at_ms: null,
    duration_ms: null,
    retryable: null,
    failure_kind: null,
    resume_note: null,
    latest_progress: null,
    lease_owner: null,
    lease_acquired_at_ms: null,
    lease_expires_at_ms: null,
    last_heartbeat_at_ms: null,
    attempt_count: 1,
    max_attempts: 3,
    diff_available: null,
    changed_file_count: null,
    changed_files: [],
    test_report_excerpt: null,
    ...task,
  };
}

async function selectComposerPermissionMode(
  page: import("@playwright/test").Page,
  mode: "manual" | "trust" | "full-access",
) {
  await page.getByTestId("composer-permission-mode").click();
  await page.getByTestId(`composer-permission-${mode === "trust" ? "trust-current-project" : mode}`).click();
}

test.describe("Phase 7 acceptance surfaces", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("desktop capability baseline preserves project, output, and dialog flows", async ({ page }) => {
    const sessionId = "desktop-capability-smoke";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockDirectoryPicker = async () => "/Users/cabbos/project/security-smoke";
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    const sidebar = page.locator("aside").first();
    await sidebar.getByTestId("workspace-trigger").click();
    await page.getByRole("menuitem", { name: "选择文件夹" }).click();
    await expect(sidebar.getByRole("button", { name: /security-smoke/ })).toBeVisible();

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "security-output" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "security-output",
        content: "capability smoke output",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "security-output" },
    ]);
    await expect(page.getByText("capability smoke output")).toBeVisible();

    await page.getByRole("button", { name: "设置" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
  });

  test("主题切换会同步工作台与新对话", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "theme-surface-session";
    });

    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const shell = page.getByTestId("operating-surface");
    const conversation = page.locator(".forge-session-operating-surface");
    const initialTheme = await shell.getAttribute("data-theme");
    expect(initialTheme).toMatch(/^(light|dark)$/);
    await expect(conversation).toHaveAttribute("data-conversation-theme", initialTheme!);

    await page.keyboard.press("Meta+K");
    await page.getByRole("option", { name: /切换主题/ }).click();

    const selectedTheme = initialTheme === "dark" ? "light" : "dark";
    await expect(shell).toHaveAttribute("data-theme", selectedTheme);
    await expect(conversation).toHaveAttribute("data-conversation-theme", selectedTheme);
  });

  test("work panel opens on a launcher without project archive content", async ({ page }) => {
    await page.getByRole("button", { name: "打开工作面板" }).click();

    const panel = page.getByRole("complementary", { name: "工作面板" });
    const launcher = panel.getByTestId("work-panel-launcher");
    await expect(panel).toBeVisible();
    await expect(launcher.locator("[data-slot='command']")).toBeFocused();
    await expect(panel.getByText("工作面板", { exact: true })).toHaveCount(0);
    await expect(panel.getByRole("tablist")).toHaveCount(0);
    await expect(panel.getByRole("button", { name: "关闭工作面板" })).toBeVisible();
    await expect(panel.getByRole("option", { name: /^审阅/ })).toBeVisible();
    await expect(panel.getByRole("option", { name: /^终端/ })).toBeVisible();
    await expect(panel.getByRole("option", { name: /^预览/ })).toBeVisible();
    await expect(panel.getByRole("option", { name: /^打开文件/ })).toBeVisible();
    await expect(panel.getByRole("option", { name: /^侧边任务/ })).toBeVisible();
    await expect(panel.locator("iframe")).toHaveCount(0);
    await expect(panel.getByText("经验回忆")).toHaveCount(0);
    await expect(panel.getByText("项目档案", { exact: true })).toHaveCount(0);
    await page.keyboard.press("ArrowDown");
    await expect(panel.getByRole("option", { name: /^终端/ })).toHaveAttribute("aria-selected", "true");
    await expect(panel.getByRole("option", { name: /^终端/ })).toHaveAttribute("data-selected", "true");
    await expect(panel.getByRole("option", { name: /^审阅/ })).toHaveAttribute("aria-selected", "false");
    await expect(panel.getByRole("option", { name: /^审阅/ })).toHaveAttribute("data-selected", "false");
    await page.keyboard.press("Enter");
    await expect(panel.getByTestId("work-panel-terminal")).toBeVisible();
  });

  test("work panel desktop sheet keeps its outer breathing room inside the workbench", async ({ page }) => {
    await page.setViewportSize({ width: 1200, height: 900 });
    await page.getByRole("button", { name: "打开工作面板" }).click();
    await expect(page.getByRole("complementary", { name: "工作面板" })).toHaveAttribute("data-viewport-mode", "split");
    const metrics = await page.evaluate(() => {
      const panel = document.querySelector<HTMLElement>("aside[data-testid='work-panel']");
      const workbench = document.querySelector<HTMLElement>("[data-testid='main-workbench']");
      if (!panel || !workbench) return null;
      const panelRect = panel.getBoundingClientRect();
      const workbenchRect = workbench.getBoundingClientRect();
      return { panelTop: panelRect.top, panelBottom: panelRect.bottom, workbenchTop: workbenchRect.top, workbenchBottom: workbenchRect.bottom };
    });
    expect(metrics).not.toBeNull();
    expect(metrics!.panelTop - metrics!.workbenchTop).toBeGreaterThanOrEqual(9);
    expect(metrics!.panelBottom).toBeLessThanOrEqual(metrics!.workbenchBottom - 9);
  });

  test("work panel overlay starts below the app titlebar and preserves a reachable close control", async ({ page }) => {
    await page.setViewportSize({ width: 700, height: 900 });
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    const titlebar = page.getByTestId("app-titlebar");
    const [panelBox, titlebarBox] = await Promise.all([panel.boundingBox(), titlebar.boundingBox()]);
    expect(panelBox).not.toBeNull();
    expect(titlebarBox).not.toBeNull();
    expect(panelBox!.y).toBeGreaterThanOrEqual(titlebarBox!.y + titlebarBox!.height - 1);
    await expect(panel.getByRole("button", { name: "关闭工作面板" })).toBeVisible();
    await panel.getByRole("button", { name: "关闭工作面板" }).click();
    await expect(panel).toHaveCount(0);
  });

  test("work panel restores its default split width and keeps the narrow overlay usable", async ({ page }) => {
    await page.setViewportSize({ width: 1200, height: 900 });
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    const separator = page.getByRole("separator", { name: "调整工作面板宽度" });
    const reviewOption = panel.getByRole("option", { name: /^审阅/ });

    await expect(panel).toHaveAttribute("data-width-percent", "40");
    await expect(reviewOption).toHaveCSS("border-top-width", "0px");
    await expect(reviewOption).toHaveCSS("transform", "none");

    await separator.dblclick();
    await expect(panel).toHaveAttribute("data-width-percent", "40");
    await panel.getByRole("button", { name: "关闭工作面板" }).click();
    await page.getByRole("button", { name: "打开工作面板" }).click();
    await expect(panel).toHaveAttribute("data-width-percent", "40");

    await page.setViewportSize({ width: 700, height: 900 });
    await expect(panel).toHaveAttribute("data-viewport-mode", "overlay");
    await expect(page.getByTestId("main-workbench")).toBeVisible();

    await page.setViewportSize({ width: 1200, height: 900 });
    await expect(panel).toHaveAttribute("data-viewport-mode", "split");
    await expect(panel).toHaveAttribute("data-width-percent", "40");
    const renderedWidthPercent = () => panel.evaluate((element) => {
      const layout = element.closest<HTMLElement>(".forge-work-panel-layout");
      if (!layout) return null;
      return element.getBoundingClientRect().width / layout.getBoundingClientRect().width * 100;
    });
    await expect.poll(renderedWidthPercent).toBeGreaterThan(37);
    await expect.poll(renderedWidthPercent).toBeLessThan(41);
  });

  test("work panel terminal keeps toolbar text at accessible contrast in light theme", async ({ page }) => {
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    await panel.getByRole("option", { name: /^终端/ }).click();
    const shell = page.getByTestId("operating-surface");
    if (await shell.getAttribute("data-theme") !== "light") {
      await page.keyboard.press("Meta+K");
      await page.getByRole("option", { name: /切换主题/ }).click();
    }
    const contrast = await panel.locator(".forge-work-panel-terminal .forge-work-panel-content-toolbar").evaluate((toolbar) => {
      const parse = (value: string) => value.match(/\d+(?:\.\d+)?/g)?.slice(0, 3).map(Number) ?? [];
      const luminance = (value: string) => parse(value).map((channel) => {
        const normalized = channel / 255;
        return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
      }).reduce((total, channel, index) => total + channel * [0.2126, 0.7152, 0.0722][index], 0);
      const background = getComputedStyle(toolbar).backgroundColor;
      const values = Array.from(toolbar.querySelectorAll<HTMLElement>("small, button")).map((element) => getComputedStyle(element).color);
      return values.map((color) => {
        const [lighter, darker] = [luminance(color), luminance(background)].sort((a, b) => b - a);
        return (lighter + 0.05) / (darker + 0.05);
      });
    });
    expect(contrast.every((value) => value >= 4.5)).toBeTruthy();
    const restart = panel.getByRole("button", { name: "重启" });
    await restart.hover();
    const hoverContrast = await restart.evaluate((button) => {
      const parse = (value: string) => value.match(/\d+(?:\.\d+)?/g)?.slice(0, 3).map(Number) ?? [];
      const luminance = (value: string) => parse(value).map((channel) => {
        const normalized = channel / 255;
        return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
      }).reduce((total, channel, index) => total + channel * [0.2126, 0.7152, 0.0722][index], 0);
      const [lighter, darker] = [luminance(getComputedStyle(button).color), luminance(getComputedStyle(button.parentElement!).backgroundColor)].sort((a, b) => b - a);
      return (lighter + 0.05) / (darker + 0.05);
    });
    expect(hoverContrast).toBeGreaterThanOrEqual(4.5);
  });

  test("work panel preview and file adapters open selected objects as tabs", async ({ page }) => {
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });

    await panel.getByRole("option", { name: /^预览/ }).click();
    await expect(panel.getByPlaceholder("输入本机网址或搜索文件")).toBeVisible();
    await panel.getByPlaceholder("输入本机网址或搜索文件").fill("http://localhost:1420");
    await panel.getByRole("option", { name: /http:\/\/localhost:1420/ }).click();

    await expect(panel.getByRole("tablist", { name: "已打开的工作内容" })).toBeVisible();
    await expect(panel.getByRole("button", { name: "更多已打开内容" })).toBeVisible();
    await expect(panel.getByRole("button", { name: "新建工作面板标签" })).toBeVisible();
    await expect(panel.getByRole("tab", { name: /localhost:1420/ })).toBeVisible();
    await expect(panel.locator("iframe[title='localhost:1420']")).toBeVisible();
    const panelSurface = panel;
    const viewport = panel.locator(".forge-work-panel-preview-viewport");
    await expect(panelSurface).toHaveCSS("border-radius", "12px");
    await expect(panelSurface).toHaveCSS("box-shadow", /0px (?:18px 42px|16px 38px)/);
    await expect(viewport).toHaveCSS("border-top-width", "0px");
    await expect(viewport).toHaveCSS("box-shadow", "none");
    await panel.getByRole("button", { name: "新建工作面板标签" }).click();
    await panel.getByRole("option", { name: /^打开文件/ }).click();
    await panel.getByPlaceholder("搜索工作区文件").fill("README");
    await panel.getByRole("option", { name: /README.md/ }).click();

    await expect(panel.getByRole("tab", { name: /README.md/ })).toBeVisible();
    await expect(panel.getByTestId("work-panel-file-view")).toContainText("export function Demo()");

    await panel.getByRole("button", { name: "新建工作面板标签" }).click();
    await panel.getByRole("option", { name: /^打开文件/ }).click();
    await panel.getByPlaceholder("搜索工作区文件").fill("README");
    await panel.getByRole("option", { name: /README.md/ }).click();
    await expect(panel.getByRole("tab", { name: /README.md/ })).toHaveCount(1);
    await expect(panel.getByText("经验回忆")).toHaveCount(0);
  });

  test("work panel review sends one line feedback into the conversation", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "work-panel-review-session";
    });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });

    await panel.getByRole("option", { name: /^审阅/ }).click();
    await expect(panel.getByRole("button", { name: /^README\.md \+/ })).toBeVisible();
    await panel.getByLabel("README.md 第 2 行").click();
    await panel.getByPlaceholder("写下这一行需要调整的地方").fill("这里需要保留空状态说明");
    await panel.getByRole("button", { name: "发送到对话" }).click();

    const composer = page.locator("textarea.forge-composer-textarea");
    await expect(composer).toContainText("README.md:2");
    await expect(composer).toContainText("这里需要保留空状态说明");
    await expect(composer).toBeFocused();
  });

  test("work panel opens one selected subtask and sends instructions back to conversation", async ({ page }) => {
    const sessionId = "work-panel-subtask-session";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await simulateStream(page, sessionId, [
      {
        event_type: "agent_a2a_updated",
        session_id: sessionId,
        state: {
          running_count: 2,
          completed_count: 0,
          failed_count: 0,
          interrupted_count: 0,
          tasks: [
            workPanelSubtask({
              task_id: "subtask-settings",
              title: "设置诊断",
              latest_message: "正在核对诊断状态",
              latest_progress: "已完成数据映射",
              changed_file_count: 1,
              changed_files: ["apps/desktop/src/components/settings/Diagnostics.tsx"],
            }),
            workPanelSubtask({
              task_id: "subtask-history",
              title: "历史记录",
              latest_message: "正在整理会话列表",
            }),
          ],
        },
      },
    ]);

    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    await panel.getByRole("option", { name: /^侧边任务/ }).click();
    await panel.getByRole("option", { name: /设置诊断/ }).click();

    await expect(panel.getByRole("tab", { name: "设置诊断" })).toBeVisible();
    await expect(panel).toContainText("正在核对诊断状态");
    await expect(panel).toContainText("已完成数据映射");
    await expect(panel).toContainText("Diagnostics.tsx");
    await expect(panel.getByText("历史记录", { exact: true })).toHaveCount(0);
    await expect(panel.getByRole("button", { name: "接管子任务" })).toBeDisabled();

    await panel.getByRole("button", { name: "补充指令" }).click();
    await panel.getByPlaceholder("告诉这个子任务接下来要做什么").fill("只检查当前设置页，不要扩展范围");
    await panel.getByRole("button", { name: "发送到对话" }).click();

    const composer = page.locator("textarea.forge-composer-textarea");
    await expect(composer).toContainText("给子任务「设置诊断」补充指令");
    await expect(composer).toContainText("只检查当前设置页，不要扩展范围");
    await expect(composer).toBeFocused();
  });

  test("work panel terminal is temporary and scoped to the current task", async ({ page }) => {
    const sessionId = "work-panel-terminal-session";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });

    await panel.getByRole("option", { name: /^终端/ }).click();
    await expect(panel.getByRole("tab", { name: "终端" })).toBeVisible();
    const terminal = panel.getByTestId("work-panel-terminal");
    await expect(terminal).toBeVisible();
    await expect(terminal).toContainText("临时验证终端");

    const command = terminal.getByRole("textbox", { name: "临时验证命令" });
    await command.fill("printf 'verification passed'");
    await terminal.getByRole("button", { name: "运行验证命令" }).click();
    await expect(terminal.getByRole("log")).toContainText("verification passed");

    await panel.getByRole("button", { name: "关闭 终端" }).click();
    await expect.poll(() => page.evaluate(() => {
      // @ts-expect-error acceptance mock
      const startedId = window.__mockStartedTerminal?.terminalId;
      // @ts-expect-error acceptance mock
      return window.__mockClosedTerminalIds?.includes(startedId) ?? false;
    })).toBe(true);
    await expect(panel.getByTestId("work-panel-launcher")).toBeVisible();
  });

  test("work panel restore keeps tabs and supports keyboard navigation", async ({ page }) => {
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    const emptyLauncher = panel.getByTestId("work-panel-launcher");
    await expect(emptyLauncher).toHaveAttribute("data-mode", "empty");
    await expect(emptyLauncher.getByPlaceholder("搜索工作面板")).toHaveCount(0);
    const separator = page.getByRole("separator", { name: "调整工作面板宽度" });
    await expect(separator).toBeVisible();
    await expect(panel.getByRole("button", { name: "最大化工作面板" })).toBeVisible();

    await panel.getByRole("option", { name: /^预览/ }).click();
    await panel.getByPlaceholder("输入本机网址或搜索文件").fill("http://localhost:1420");
    await panel.getByRole("option", { name: /http:\/\/localhost:1420/ }).click();
    await panel.getByRole("button", { name: "新建工作面板标签" }).click();
    await expect(emptyLauncher).toHaveAttribute("data-mode", "new");
    await expect(emptyLauncher).toContainText("打开新的…");
    await expect(emptyLauncher.locator("[data-slot='command']")).toBeFocused();
    await panel.getByRole("option", { name: /^打开文件/ }).click();
    await panel.getByPlaceholder("搜索工作区文件").fill("README");
    await panel.getByRole("option", { name: /README.md/ }).click();

    const previewTab = panel.getByRole("tab", { name: /localhost:1420/ });
    const fileTab = panel.getByRole("tab", { name: /README.md/ });
    await previewTab.focus();
    await page.keyboard.press("ArrowRight");
    await expect(fileTab).toBeFocused();

    await panel.getByRole("button", { name: "关闭工作面板" }).click();
    await page.getByRole("button", { name: "打开工作面板" }).click();
    await expect(previewTab).toBeVisible();
    await expect(fileTab).toBeVisible();
  });

  test("work panel unavailable file stays isolated and can retry", async ({ page }) => {
    await page.evaluate(() => {
      localStorage.setItem("forge-work-panel-v1", JSON.stringify({
        version: 1,
        tasks: {
          "/Users/cabbos/project/forge": {
            tabs: [
              { kind: "file", id: "file:missing.ts", label: "missing.ts", path: "missing.ts" },
              { kind: "preview", id: "preview:http://localhost:1420", label: "localhost:1420", target: { type: "url", url: "http://localhost:1420" } },
            ],
            activeTabId: "file:missing.ts",
            launcherOpen: false,
          },
          "another-task": {
            tabs: [{ kind: "unknown", id: "bad", label: "bad" }],
            activeTabId: "bad",
            launcherOpen: false,
          },
        },
      }));
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockPreviewFileError = "文件已被移走";
    });
    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });

    await expect(panel.getByRole("alert")).toContainText("文件已被移走");
    await expect(panel.getByRole("button", { name: "重试读取文件" })).toBeVisible();
    await panel.getByRole("tab", { name: /localhost:1420/ }).click();
    await expect(panel.locator("iframe[title='localhost:1420']")).toBeVisible();
    await expect(panel.getByRole("tab", { name: "missing.ts" })).toBeVisible();
  });

  test("settings diagnostics surfaces doctor status and gateway runtime", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    await expect
      .poll(async () => {
        return page.evaluate(() => {
          // @ts-expect-error acceptance mock
          return Number(window.__providerCatalogRequestCount ?? 0);
        });
      })
      .toBeGreaterThan(0);
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "诊断" }).click();

    await expect(dialog.getByRole("heading", { name: "诊断", exact: true })).toBeVisible();
    await expect(dialog).toContainText("系统正常");
    await expect(dialog).toContainText("配置文件");
    await expect(dialog).toContainText("Gateway service");
    await expect(dialog).toContainText("后台运行时 · 运行中");
    await expect(dialog).toContainText("0 pending · 0 inputs · 0 claimed · 0 dead-letter");

    await dialog.getByRole("button", { name: "Refresh diagnostics" }).click();
    const refreshCount = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__diagnosticsReportRequestCount;
    });
    expect(refreshCount).toBeGreaterThan(1);
  });

  test("composer can trust the current project across conversations", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "project-status-trust-session-1";
    });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();

    await expect(page.getByTestId("composer-permission-mode")).toContainText("手动确认");
    await selectComposerPermissionMode(page, "trust");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");

    const trustArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionModeArgs;
    });
    expect(trustArgs).toMatchObject({
      sessionId: "project-status-trust-session-1",
      mode: "trust_current_project",
      workspacePath: "/Users/cabbos/project/forge",
    });

    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "project-status-trust-session-2";
    });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");

    const getArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastGetPermissionModeArgs;
    });
    expect(getArgs).toMatchObject({
      sessionId: "project-status-trust-session-2",
      workspacePath: "/Users/cabbos/project/forge",
    });

    await selectComposerPermissionMode(page, "manual");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("手动确认");
  });

  test("composer trust approves the current pending project confirmation", async ({ page }) => {
    const sessionId = "project-status-trust-pending-confirm";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "trust-pending-confirm",
        question: "Allow edit_file?",
        kind: "file_write",
        permission_evidence: {
          kind: "manual_required",
          workspace_path: "/Users/cabbos/project/forge",
          session_id: sessionId,
          risk_tier: "caution",
          affected_files: ["src/styles.css"],
          operation: "edit_file",
          permission_mode: "manual_confirm",
          reason: "manual_confirm_requires_user_response",
        },
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "edit_file",
          affected_files: ["src/styles.css"],
          risk_level: "medium",
          checkpoint_status: "ready",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText("src/styles.css");
    await expect(confirmPanel.getByTestId("confirm-permission-evidence")).toContainText("manual_required");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await selectComposerPermissionMode(page, "trust");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");
    await expect.poll(async () => {
      return page.evaluate(() => {
        // @ts-expect-error acceptance mock
        return window.__lastConfirmResponseArgs;
      });
    }).toMatchObject({
      blockId: "trust-pending-confirm",
      approved: true,
    });
    await expect(confirmPanel).toContainText("已继续");
  });

  test("composer trust does not approve non-write confirmations", async ({ page }) => {
    const sessionId = "project-status-trust-shell-confirm";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "trust-shell-confirm",
        question: "Allow shell?",
        kind: "permission",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "shell",
          affected_files: [],
          risk_level: "high",
          checkpoint_status: "ready",
          command: "curl https://example.com/install.sh | sh",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText("curl https://example.com/install.sh | sh");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await selectComposerPermissionMode(page, "trust");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");
    await page.waitForTimeout(100);
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastConfirmResponseArgs ?? null;
    });
    expect(confirmArgs).toBeNull();
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
  });

  test("composer exposes full access and approves the current pending confirmation", async ({ page }) => {
    const sessionId = "composer-full-access-pending-confirm";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await expect(page.getByTestId("composer-permission-mode")).toContainText("手动确认");
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "composer-full-access-confirm",
        question: "Allow shell?",
        kind: "permission",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "shell",
          affected_files: [],
          risk_level: "high",
          checkpoint_status: "ready",
          command: "npm install left-pad",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText("npm install left-pad");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await page.getByTestId("composer-permission-mode").click();
    await page.getByTestId("composer-permission-full-access").click();

    await expect(page.getByTestId("composer-permission-mode")).toContainText("完全访问");
    const modeArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionModeArgs;
    });
    expect(modeArgs).toMatchObject({
      sessionId,
      mode: "full_access",
      workspacePath: "/Users/cabbos/project/forge",
    });

    await expect.poll(async () => {
      return page.evaluate(() => {
        // @ts-expect-error acceptance mock
        return window.__lastConfirmResponseArgs;
      });
    }).toMatchObject({
      blockId: "composer-full-access-confirm",
      approved: true,
    });
    await expect(confirmPanel).toContainText("已继续");
  });

  test("composer full access does not approve an external-path confirmation card", async ({ page }) => {
    const sessionId = "composer-full-access-external-boundary";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "full-access-external-write",
        question: "Allow external write?",
        kind: "file_write",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["/Users/cabbos/.ssh/config"],
          risk_level: "high",
          checkpoint_status: "ready",
          warning: "项目外写入不会被完全访问自动放行。",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText("项目外写入不会被完全访问自动放行。");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await page.getByTestId("composer-permission-mode").click();
    await page.getByTestId("composer-permission-full-access").click();

    await expect(page.getByTestId("composer-permission-mode")).toContainText("完全访问");
    await page.waitForTimeout(100);
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastConfirmResponseArgs ?? null;
    });
    expect(confirmArgs).toBeNull();
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
  });

  test("project trust does not approve a sensitive workspace confirmation card", async ({ page }) => {
    const sessionId = "project-trust-sensitive-boundary";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "trust-sensitive-write",
        question: "Allow .env write?",
        kind: "file_write",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["/Users/cabbos/project/forge/.env"],
          risk_level: "high",
          checkpoint_status: "ready",
          warning: "敏感文件仍需手动确认。",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText(".env");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await selectComposerPermissionMode(page, "trust");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");
    await page.waitForTimeout(100);
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastConfirmResponseArgs ?? null;
    });
    expect(confirmArgs).toBeNull();
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
  });

  test("project trust does not approve an external-path confirmation card", async ({ page }) => {
    const sessionId = "project-trust-external-boundary";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "trust-external-write",
        question: "Allow external write?",
        kind: "file_write",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["/Users/cabbos/.ssh/config"],
          risk_level: "high",
          checkpoint_status: "ready",
          warning: "项目外写入不会被信任项目自动放行。",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText("项目外写入不会被信任项目自动放行。");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await selectComposerPermissionMode(page, "trust");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");
    await page.waitForTimeout(100);
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastConfirmResponseArgs ?? null;
    });
    expect(confirmArgs).toBeNull();
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
  });

  test("project trust does not approve a dotenv variant confirmation card", async ({ page }) => {
    const sessionId = "project-trust-dotenv-variant";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "trust-dotenv-local-write",
        question: "Allow .env.local write?",
        kind: "file_write",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["/Users/cabbos/project/forge/.env.local"],
          risk_level: "high",
          checkpoint_status: "ready",
          warning: "敏感环境文件仍需手动确认。",
        },
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmPanel).toContainText(".env.local");
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

    await selectComposerPermissionMode(page, "trust");
    await expect(page.getByTestId("composer-permission-mode")).toContainText("信任项目");
    await page.waitForTimeout(100);
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastConfirmResponseArgs ?? null;
    });
    expect(confirmArgs).toBeNull();
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
  });

  test("composer full access inherits to a new conversation in the same workspace", async ({ page }) => {
    const firstSessionId = "composer-full-access-inherit-1";
    const secondSessionId = "composer-full-access-inherit-2";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, firstSessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await expect(page.getByTestId("composer-permission-mode")).toContainText("手动确认");

    await page.getByTestId("composer-permission-mode").click();
    await page.getByTestId("composer-permission-full-access").click();
    await expect(page.getByTestId("composer-permission-mode")).toContainText("完全访问");

    const setArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionModeArgs;
    });
    expect(setArgs).toMatchObject({
      sessionId: firstSessionId,
      mode: "full_access",
      workspacePath: "/Users/cabbos/project/forge",
    });

    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, secondSessionId);
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();

    await expect.poll(async () => {
      return page.evaluate(() => {
        // @ts-expect-error acceptance mock
        return window.__lastGetPermissionModeArgs;
      });
    }).toMatchObject({
      sessionId: secondSessionId,
      workspacePath: "/Users/cabbos/project/forge",
    });
    await expect(page.getByTestId("composer-permission-mode")).toContainText("完全访问");
  });

  test("permission mode does not leak after workspace changes", async ({ page }) => {
    await page.addInitScript(() => {
      const now = Date.now();
      // @ts-expect-error acceptance mock
      window.__mockProfiles = [
        {
          id: "default",
          name: "默认",
          default_provider: "deepseek",
          default_model: "deepseek-v4-flash[1m]",
          default_workspace: null,
          credential_overrides: {},
          created_at_ms: now,
          updated_at_ms: now,
        },
      ];
      // @ts-expect-error acceptance mock
      window.__mockActiveProfileId = "default";
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    await page.getByTestId("workspace-trigger").click();
    await page.getByRole("menuitem", { name: "手动输入路径" }).click();
    await page.getByLabel("项目文件夹路径").fill("/Users/cabbos/project/forge-test-app");
    await page.getByRole("button", { name: "添加" }).click();
    await expect(page.getByTestId("workspace-trigger")).toContainText("forge-test-app");

    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "permission-leak-test-app";
    });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.getByTestId("composer-permission-mode").click();
    await page.getByTestId("composer-permission-full-access").click();
    await expect(page.getByTestId("composer-permission-mode")).toContainText("完全访问");

    const testAppArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionModeArgs;
    });
    expect(testAppArgs).toMatchObject({
      sessionId: "permission-leak-test-app",
      mode: "full_access",
      workspacePath: "/Users/cabbos/project/forge-test-app",
    });

    await page.getByTestId("workspace-trigger").click();
    await page.getByRole("menuitemradio", { name: "forge", exact: true }).click();
    await expect(page.getByTestId("workspace-trigger")).toContainText("forge");
    await expect(page.getByTestId("workspace-trigger")).not.toContainText("forge-test-app");

    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "permission-leak-forge";
    });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();

    await expect.poll(async () => {
      return page.evaluate(() => {
        // @ts-expect-error acceptance mock
        return window.__lastGetPermissionModeArgs;
      });
    }).toMatchObject({
      sessionId: "permission-leak-forge",
      workspacePath: "/Users/cabbos/project/forge",
    });
    await expect(page.getByTestId("composer-permission-mode")).toContainText("手动确认");
  });

  test("confirm response replay resolves a pending confirmation card", async ({ page }) => {
    const sessionId = "confirm-response-replay-session";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "confirm-response-replay",
        question: "Allow replayed confirmation?",
        kind: "ask_user",
      },
    ], 1);

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "Allow replayed confirmation?" });
    await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
    await expect(confirmPanel.getByTestId("confirm-cancel")).toBeVisible();

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_response",
        session_id: sessionId,
        block_id: "confirm-response-replay",
        question: "Allow replayed confirmation?",
        kind: "ask_user",
        approved: false,
        responded_at_ms: Date.now(),
        reason: "user_response",
      },
    ], 1);

    await expect(confirmPanel).toContainText("已取消");
    await expect(confirmPanel).not.toContainText("确认已中断");
    await expect(confirmPanel.getByTestId("confirm-approve")).toHaveCount(0);
    await expect(confirmPanel.getByTestId("confirm-cancel")).toHaveCount(0);
  });

  test("startup transcript hydration resolves a confirmation card", async ({ page }) => {
    const sessionId = "confirm-response-hydration-session";
    const blockId = "confirm-response-hydration";
    await page.addInitScript(({ sessionId, blockId }) => {
      const now = Date.now();
      // @ts-expect-error acceptance mock
      window.__mockListSessions = [
        {
          id: sessionId,
          provider: "deepseek",
          model: "deepseek-v4-flash[1m]",
          status: "running",
          created_at: new Date(now - 1_000).toISOString(),
          working_dir: "/Users/cabbos/project/forge",
          created_at_ms: now - 1_000,
          updated_at_ms: now,
          context_window_tokens: 1_000_000,
        },
      ];
      // @ts-expect-error acceptance mock
      window.__mockSessionTranscripts = {
        [sessionId]: [
          {
            event_type: "confirm_ask",
            session_id: sessionId,
            block_id: blockId,
            question: "Allow hydrated confirmation?",
            kind: "ask_user",
          },
          {
            event_type: "confirm_response",
            session_id: sessionId,
            block_id: blockId,
            question: "Allow hydrated confirmation?",
            kind: "ask_user",
            approved: false,
            responded_at_ms: now,
            reason: "user_response",
          },
        ],
      };
    }, { sessionId, blockId });

    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    const transcriptArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastLoadSessionTranscriptArgs;
    });
    expect(transcriptArgs).toMatchObject({ sessionId });

    const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "Allow hydrated confirmation?" });
    await expect(confirmPanel).toContainText("已取消");
    await expect(confirmPanel).not.toContainText("确认已中断");
    await expect(confirmPanel.getByTestId("confirm-approve")).toHaveCount(0);
    await expect(confirmPanel.getByTestId("confirm-cancel")).toHaveCount(0);
  });

  test("fresh same-session output clears stale session health alert", async ({ page }) => {
    const sessionId = "acceptance-stale-health-session";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await simulateStream(page, sessionId, [
      {
        event_type: "health_alert",
        session_id: sessionId,
        alert_id: `session-stale-${sessionId}`,
        level: "warn",
        title: "会话无响应",
        message: "会话在过去 5 分钟内没有产生新事件。",
        remediation: "请检查会话状态。",
      },
    ]);

    const banner = page.getByTestId("health-alert-banner");
    await expect(banner).toBeVisible();
    await expect(banner).toContainText("会话无响应");

    await simulateStream(page, sessionId, [
      {
        event_type: "text_start",
        session_id: sessionId,
        block_id: "fresh-output",
      },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "fresh-output",
        content: "fresh output",
      },
    ]);

    await expect(banner).toHaveCount(0);
  });

  test("stale alert from another session does not cover the active session", async ({ page }) => {
    const oldSessionId = "acceptance-old-stale-session";
    const activeSessionId = "acceptance-active-session";

    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, oldSessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await simulateStream(page, oldSessionId, [
      {
        event_type: "health_alert",
        session_id: oldSessionId,
        alert_id: `session-stale-${oldSessionId}`,
        level: "warn",
        title: "会话无响应",
        message: "旧会话在过去 5 分钟内没有产生新事件。",
        remediation: "请检查旧会话状态。",
      },
    ]);

    const banner = page.getByTestId("health-alert-banner");
    await expect(banner).toBeVisible();
    await expect(banner).toContainText("会话无响应");

    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, activeSessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await expect(banner).toHaveCount(0);
  });

  test("settings general toggles gateway autostart through service IPC", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockServiceStatus = {
        installed: false,
        running: false,
        message: "Gateway systemd user service is not installed.",
        supported: true,
        backend: "systemd",
        service_id: "forge-gateway.service",
        label: "forge-gateway.service",
        launch_domain: "systemd-user",
        service_path: "/home/alice/.config/systemd/user/forge-gateway.service",
        plist_path: "/home/alice/.config/systemd/user/forge-gateway.service",
        log_path: "/home/alice/.forge/logs/gateway.log",
        error_log_path: "/home/alice/.forge/logs/gateway-error.log",
        status_message: "Service 'forge-gateway.service' is not installed.",
      };
    });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "通用" }).click();

    await expect(dialog.getByRole("heading", { name: "通用" }).first()).toBeVisible();
    await expect(dialog).toContainText("systemd user service");
    const switchControl = dialog.getByRole("switch");
    await expect(switchControl).toHaveAttribute("aria-checked", "false");

    await switchControl.click();
    await expect(dialog).toContainText("已安装");
    await expect(dialog).toContainText("运行中");
    await expect(switchControl).toHaveAttribute("aria-checked", "true");
    let autostartArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetAutostartArgs;
    });
    expect(autostartArgs).toEqual({ enabled: true });

    await switchControl.click();
    await expect(dialog).toContainText("未安装");
    await expect(switchControl).toHaveAttribute("aria-checked", "false");
    autostartArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetAutostartArgs;
    });
    expect(autostartArgs).toEqual({ enabled: false });
  });

  test("settings models runs a mocked successful provider probe", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });

    await expect(providerRow).toContainText("已配置");
    const beforeClickCount = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return Number(window.__providerProbeRequestCount ?? 0);
    });
    expect(beforeClickCount).toBe(0);

    await providerRow.getByRole("button", { name: "检测 DeepSeek" }).click();
    await expect(providerRow).toContainText("DeepSeek probe passed.");
    await expect(providerRow).toContainText("Tool schema accepted");

    const probeArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastProbeProviderArgs;
    });
    expect(probeArgs).toEqual({ provider: "deepseek" });
  });

  test("settings models renders cached manual provider probe evidence", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = [{ provider: "deepseek", configured: true, source: "system_store", status: "available", error: null }];
      // @ts-expect-error acceptance mock
      window.__mockProviderCatalog = [
        {
          id: "deepseek",
          label: "DeepSeek",
          default_model: "deepseek-v4-flash[1m]",
          context_window_tokens: 1_000_000,
          aliases: [],
          requires_api_key: true,
          supports_streaming: true,
          supports_tools: true,
          source: "built_in",
          base_url: "https://api.deepseek.com/anthropic",
          transport: "anthropic_messages",
          api_key_env: ["DEEPSEEK_API_KEY"],
          base_url_env: ["DEEPSEEK_BASE_URL"],
          model_catalog_source: null,
          probe_evidence: {
            source: "manual_probe",
            status: "passed",
            recorded_at_ms: 1717891200000,
            model: "deepseek-v4-flash[1m]",
            base_url: "https://api.deepseek.com/anthropic",
            checks: [
              { id: "key_present", label: "Key present", status: "passed" },
              { id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed" },
            ],
          },
          models: [],
        },
      ];
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });

    await expect(providerRow).toContainText("上次手动检测通过");
    await expect(providerRow).toContainText("证据摘要");
    await expect(providerRow).toContainText("证据需复核");
    await expect(providerRow).toContainText("手动检测通过 · 检测 2024-06-09 · 检测已超过 14 天 · 目录未验证");
    await expect(providerRow).toContainText("模型 deepseek-v4-flash[1m]");
    await expect(providerRow).toContainText("检测 2024-06-09");
    await expect(providerRow).toContainText("Tool schema accepted");
  });

  test("settings models clears stale provider evidence after manual recheck", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = [{ provider: "deepseek", configured: true, source: "system_store", status: "available", error: null }];
      // @ts-expect-error acceptance mock
      window.__mockProviderCatalogCache = [
        {
          id: "deepseek",
          label: "DeepSeek",
          default_model: "deepseek-v4-flash[1m]",
          context_window_tokens: 1_000_000,
          aliases: [],
          requires_api_key: true,
          supports_streaming: true,
          supports_tools: true,
          source: "built_in",
          base_url: "https://api.deepseek.com/anthropic",
          transport: "anthropic_messages",
          api_key_env: ["DEEPSEEK_API_KEY"],
          base_url_env: ["DEEPSEEK_BASE_URL"],
          model_catalog_source: null,
          model_catalog_recorded_at_ms: null,
          probe_evidence: {
            source: "manual_probe",
            status: "passed",
            recorded_at_ms: 1717891200000,
            model: "deepseek-v4-flash[1m]",
            base_url: "https://api.deepseek.com/anthropic",
            checks: [
              { id: "key_present", label: "Key present", status: "passed" },
              { id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed" },
            ],
          },
          models: [],
        },
      ];
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });
    const summary = providerRow.getByTestId("settings-provider-evidence-summary");

    await expect(summary).toContainText("证据需复核");
    await providerRow.getByRole("button", { name: "检测 DeepSeek" }).click();
    await expect(providerRow).toContainText("DeepSeek probe passed.");
    await expect(summary).toContainText("手动检测通过");
    await expect(summary).not.toContainText("证据需复核");
    await expect(summary).not.toContainText("检测已超过 14 天");
  });

  test("settings models renders mainstream provider metadata without clipping", async ({ page }) => {
    const providerStatuses = [
      "deepseek",
      "anthropic",
      "kimi",
      "glm",
      "alibaba",
      "minimax",
      "openai",
      "openrouter",
      "gemini",
      "xai",
      "groq",
      "mistral",
      "ollama",
      "custom_openai",
      "custom_anthropic",
    ].map((provider) => ({
      provider,
      set: provider === "deepseek" || provider === "ollama",
      preview: provider === "deepseek" ? "sk-e0...23ef" : provider === "ollama" ? "not required" : "",
    }));
    await page.evaluate((statuses) => {
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = statuses;
    }, providerStatuses);

    await page.getByRole("button", { name: "设置" }).click();
    await page.setViewportSize({ width: 720, height: 900 });
    const dialog = page.getByRole("dialog");
    const rows = dialog.getByTestId("settings-provider-row");
    await expect(rows).toHaveCount(providerStatuses.length);

    const requiredMetadata = [
      { label: "Kimi / Moonshot", model: "Kimi K2.7 Code", meta: "上下文 262K" },
      { label: "Alibaba / Qwen", model: "Qwen3 Coder Plus", meta: "上下文 128K" },
      { label: "Custom Anthropic-Compatible", model: "Custom Model", meta: "默认模型" },
      { label: "Groq", model: "Llama 3.3 70B Versatile", meta: "上下文 128K" },
      { label: "Gemini", model: "Gemini 2.5 Pro", meta: "上下文 1M" },
    ];
    for (const expected of requiredMetadata) {
      const row = rows.filter({ hasText: expected.label });
      await expect(row.locator("[data-provider-readable='label']")).toHaveText(expected.label);
      await expect(row.locator("[data-provider-readable='model']")).toHaveText(expected.model);
      await expect(row.locator("[data-provider-readable='meta']")).toContainText(expected.meta);
    }

    const clippedTexts = await dialog.getByTestId("settings-provider-readable-text").evaluateAll((nodes) =>
      nodes
        .filter((node) => {
          const element = node as HTMLElement;
          return element.scrollWidth > element.clientWidth + 1 || element.scrollHeight > element.clientHeight + 1;
        })
        .map((node) => node.textContent?.trim() ?? ""),
    );
    expect(clippedTexts).toEqual([]);

    const overflowingRows = await rows.evaluateAll((nodes) =>
      nodes
        .filter((node) => {
          const element = node as HTMLElement;
          return element.scrollWidth > element.clientWidth + 1;
        })
        .map((node) => node.textContent?.replace(/\s+/g, " ").trim().slice(0, 80) ?? ""),
    );
    expect(overflowingRows).toEqual([]);
  });

  test("settings models refreshes a mocked provider model catalog", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });

    const beforeClickCount = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return Number(window.__providerModelCatalogRequestCount ?? 0);
    });
    expect(beforeClickCount).toBe(0);

    await providerRow.getByRole("button", { name: "刷新模型 DeepSeek" }).click();
    await expect(providerRow).toContainText("DeepSeek returned 2 models.");
    await expect(providerRow).toContainText("Live /models");
    await expect(providerRow).toContainText("目录刷新 2024-06-09");
    await expect(providerRow).toContainText("deepseek-reasoner");
    await expect(providerRow).toContainText("deepseek-v4-flash[1m]");
    await providerRow.getByRole("button", { name: "使用模型 deepseek-reasoner" }).click();
    await expect(dialog.locator(".forge-settings-info-row").filter({ hasText: "默认模型" })).toContainText("deepseek-reasoner");
    const metadata = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__tauriMockIPC("load_app_metadata", {});
    });
    expect(metadata.selectedProvider).toBe("deepseek");
    expect(metadata.selectedModel).toBe("deepseek-reasoner");

    const refreshArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastProviderModelCatalogArgs;
    });
    expect(refreshArgs).toEqual({ provider: "deepseek" });

    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.getByTestId("composer-lane")).toBeVisible();
    const modelButton = page.getByTestId("composer-model-chip");
    await modelButton.click();
    const refreshedModel = page.getByRole("menuitemradio", { name: /deepseek-reasoner/ });
    await expect(refreshedModel).toBeVisible();
    await refreshedModel.click();
    await expect(modelButton).toContainText("deepseek-reasoner");
  });

  test("settings models clears stale catalog evidence after manual refresh", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = [{ provider: "deepseek", configured: true, source: "system_store", status: "available", error: null }];
      // @ts-expect-error acceptance mock
      window.__mockProviderCatalogCache = [
        {
          id: "deepseek",
          label: "DeepSeek",
          default_model: "deepseek-v4-flash[1m]",
          context_window_tokens: 1_000_000,
          aliases: [],
          requires_api_key: true,
          supports_streaming: true,
          supports_tools: true,
          source: "built_in",
          base_url: "https://api.deepseek.com/anthropic",
          transport: "anthropic_messages",
          api_key_env: ["DEEPSEEK_API_KEY"],
          base_url_env: ["DEEPSEEK_BASE_URL"],
          model_catalog_source: "live_endpoint",
          model_catalog_recorded_at_ms: 1717891200000,
          probe_evidence: null,
          models: [
            { id: "deepseek-v4-flash[1m]", name: "deepseek-v4-flash[1m]", context_window_tokens: 1_000_000 },
          ],
        },
      ];
      // @ts-expect-error acceptance mock
      window.__mockProviderModelCatalogResult = {
        provider: "deepseek",
        provider_label: "DeepSeek",
        base_url: "https://api.deepseek.com/anthropic",
        source: "live_endpoint",
        status: "available",
        models: [
          { id: "deepseek-reasoner", name: "deepseek-reasoner" },
          { id: "deepseek-v4-flash[1m]", name: "deepseek-v4-flash[1m]" },
        ],
        message: "DeepSeek returned 2 models.",
        remediation: null,
      };
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });
    const summary = providerRow.getByTestId("settings-provider-evidence-summary");

    await expect(summary).toContainText("目录刷新已超过 14 天");
    await providerRow.getByRole("button", { name: "刷新模型 DeepSeek" }).click();
    await expect(providerRow).toContainText("DeepSeek returned 2 models.");
    await expect(summary).not.toContainText("目录刷新已超过 14 天");
  });

  test("settings models labels static provider model catalog fallback", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockProviderModelCatalogResult = {
        provider: "kimi",
        provider_label: "Kimi / Moonshot",
        base_url: "https://api.moonshot.cn/anthropic",
        source: "static_fallback",
        status: "available",
        recorded_at_ms: 1717891200000,
        models: [
          { id: "kimi-k2.7-code", name: "kimi-k2.7-code" },
          { id: "kimi-k2.5", name: "kimi-k2.5" },
          { id: "kimi-k2", name: "kimi-k2" },
        ],
        message: "Kimi / Moonshot uses Forge's static model catalog.",
        remediation: null,
      };
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = [
        { provider: "kimi", configured: false, source: "none", status: "not_configured", error: null },
      ];
    });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "Kimi / Moonshot" });

    await providerRow.getByRole("button", { name: "刷新模型 Kimi / Moonshot" }).click();
    await expect(providerRow).toContainText("Kimi / Moonshot uses Forge's static model catalog.");
    await expect(providerRow).toContainText("Forge static catalog");
    await expect(providerRow).toContainText("目录刷新 2024-06-09");
    await expect(providerRow).toContainText("not live-certified");
    await expect(providerRow.locator('[data-provider-readable="meta"]')).toContainText("目录 Forge static catalog · 目录刷新 2024-06-09");
  });

  test("settings models creates and deletes a custom provider profile", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");

    await dialog.getByRole("button", { name: "新增自定义 Provider" }).click();
    await dialog.getByLabel("Provider ID").fill("local-openai");
    await dialog.getByLabel("显示名称").fill("Local OpenAI");
    await dialog.getByLabel("Base URL", { exact: true }).fill("http://127.0.0.1:1234/v1");
    await dialog.getByLabel("默认模型").fill("local-model");
    await dialog.getByLabel("不需要 API Key").check();
    await dialog.getByLabel("Base URL env", { exact: true }).fill("LOCAL_OPENAI_BASE_URL");
    await dialog.getByLabel("Aliases").fill("local-lab, lmstudio");
    await dialog.getByRole("button", { name: "保存 Provider" }).click();

    const upsertArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertProviderProfileArgs;
    });
    expect(upsertArgs.input).toMatchObject({
      id: "local-openai",
      label: "Local OpenAI",
      transport: "openai_chat_completions",
      base_url: "http://127.0.0.1:1234/v1",
      api_key_env: [],
      base_url_env: ["LOCAL_OPENAI_BASE_URL"],
      default_model: "local-model",
      aliases: ["local-lab", "lmstudio"],
      supports_tools: true,
      supports_streaming: true,
    });

    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "Local OpenAI" });
    await expect(providerRow).toContainText("Local OpenAI");
    await expect(providerRow).toContainText("not required");
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockProviderModelCatalogResult = {
        provider: "local-openai",
        provider_label: "Local OpenAI",
        base_url: "http://127.0.0.1:1234/v1",
        source: "live_endpoint",
        status: "available",
        models: [
          { id: "local-model-v2", name: "local-model-v2" },
          { id: "local-model", name: "local-model" },
        ],
        message: "Local OpenAI returned 2 models.",
        remediation: null,
      };
    });
    await providerRow.getByRole("button", { name: "刷新模型 Local OpenAI" }).click();
    const defaultModelButton = providerRow.getByRole("button", { name: "设为 Provider 默认 local-model-v2" });
    await expect(defaultModelButton).toBeVisible();
    await expect(providerRow.getByRole("button", { name: "刷新模型 Local OpenAI" })).toBeEnabled();
    await defaultModelButton.click();
    const defaultArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertProviderProfileArgs;
    });
    expect(defaultArgs.input).toMatchObject({
      id: "local-openai",
      label: "Local OpenAI",
      base_url: "http://127.0.0.1:1234/v1",
      api_key_env: [],
      base_url_env: ["LOCAL_OPENAI_BASE_URL"],
      default_model: "local-model-v2",
      aliases: ["local-lab", "lmstudio"],
      supports_tools: true,
      supports_streaming: true,
    });
    await expect(providerRow).toContainText("local-model-v2");

    await providerRow.getByRole("button", { name: "删除 Provider Local OpenAI" }).click();
    const deleteArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastDeleteProviderProfileArgs;
    });
    expect(deleteArgs).toEqual({ provider: "local-openai" });
    await expect(dialog.getByTestId("settings-provider-row").filter({ hasText: "Local OpenAI" })).toHaveCount(0);
  });

  test("start readiness accepts a no-auth custom provider profile", async ({ page }) => {
    await page.evaluate(async () => {
      const workspace = {
        id: "/Users/cabbos/project/forge",
        name: "forge",
        path: "/Users/cabbos/project/forge",
        lastOpenedAt: Date.now(),
      };
      // @ts-expect-error acceptance mock
      await window.__tauriMockIPC("save_app_metadata", {
        metadata: {
          workspaces: [workspace],
          activeWorkspaceId: workspace.id,
          activeSessionId: null,
          selectedProvider: "local-openai",
          selectedModel: "local-model",
        },
      });
    });
    await page.addInitScript(() => {
      // @ts-expect-error acceptance mock
      window.__mockProviderCatalog = [
        {
          id: "local-openai",
          label: "Local OpenAI",
          default_model: "local-model",
          context_window_tokens: null,
          aliases: ["local-lab"],
          requires_api_key: false,
          supports_streaming: true,
          supports_tools: true,
          source: "user_defined",
          base_url: "http://127.0.0.1:1234/v1",
          transport: "openai_chat_completions",
          api_key_env: [],
          base_url_env: ["LOCAL_OPENAI_BASE_URL"],
          models: [{ id: "local-model", name: "local-model", context_window_tokens: null }],
        },
      ];
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = [{ provider: "local-openai", configured: false, source: "none", status: "not_configured", error: null }];
    });

    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const readiness = page.getByTestId("start-readiness-panel");
    await expect(readiness).toBeVisible();
    await expect(readiness.getByText("准备开始")).toBeVisible();
    await expect(readiness.getByRole("button", { name: "打开设置" })).toHaveCount(0);
    await expect(page.getByTestId("composer-model-chip")).toContainText("local-model");
  });

  test("start readiness blocks a provider profile with failed cached probe evidence", async ({ page }) => {
    await page.evaluate(async () => {
      const workspace = {
        id: "/Users/cabbos/project/forge",
        name: "forge",
        path: "/Users/cabbos/project/forge",
        lastOpenedAt: Date.now(),
      };
      // @ts-expect-error acceptance mock
      await window.__tauriMockIPC("save_app_metadata", {
        metadata: {
          workspaces: [workspace],
          activeWorkspaceId: workspace.id,
          activeSessionId: null,
          selectedProvider: "openai",
          selectedModel: "gpt-4o",
        },
      });
    });
    await page.addInitScript(() => {
      // @ts-expect-error acceptance mock
      window.__mockProviderCatalog = [
        {
          id: "openai",
          label: "OpenAI",
          default_model: "gpt-4o",
          context_window_tokens: 128000,
          aliases: ["gpt"],
          requires_api_key: true,
          supports_streaming: true,
          supports_tools: true,
          source: "built_in",
          base_url: "https://api.openai.com/v1",
          transport: "openai_chat_completions",
          api_key_env: ["OPENAI_API_KEY"],
          base_url_env: ["OPENAI_BASE_URL"],
          model_catalog_source: null,
          probe_evidence: {
            source: "manual_probe",
            status: "failed",
            model: "gpt-4o",
            base_url: "https://api.openai.com/v1",
            checks: [
              { id: "streaming_accepted", label: "Streaming accepted", status: "failed" },
            ],
          },
          models: [{ id: "gpt-4o", name: "GPT-4o", context_window_tokens: 128000 }],
        },
      ];
      // @ts-expect-error acceptance mock
      window.__mockApiKeyStatus = [{ provider: "openai", configured: true, source: "system_store", status: "available", error: null }];
    });

    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const readiness = page.getByTestId("start-readiness-panel");
    await expect(readiness).toBeVisible();
    await expect(readiness).toContainText("Provider 检测失败");
    await expect(readiness).toContainText("打开设置重新检测 provider。");

    await page.getByRole("button", { name: "设置", exact: true }).click();
    let dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "通用" }).click();
    await expect(dialog.getByRole("heading", { name: "通用" }).first()).toBeVisible();
    await dialog.getByRole("button", { name: "关闭" }).click();
    await expect(dialog).toHaveCount(0);

    await readiness.getByRole("button", { name: "打开设置" }).click();
    dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("heading", { name: "模型服务" })).toBeVisible();
    await expect(dialog.getByTestId("settings-provider-row").filter({ hasText: "手动检测失败" })).toBeVisible();
  });

  test("settings models edits a custom provider profile", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");

    await dialog.getByRole("button", { name: "新增自定义 Provider" }).click();
    await dialog.getByLabel("Provider ID").fill("local-openai");
    await dialog.getByLabel("显示名称").fill("Local OpenAI");
    await dialog.getByLabel("Base URL", { exact: true }).fill("http://127.0.0.1:1234/v1");
    await dialog.getByLabel("默认模型").fill("local-model");
    await dialog.getByLabel("不需要 API Key").check();
    await dialog.getByLabel("Aliases").fill("local-lab");
    await dialog.getByRole("button", { name: "保存 Provider" }).click();

    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "Local OpenAI" });
    await providerRow.getByRole("button", { name: "编辑 Provider Local OpenAI" }).click();

    await expect(dialog.getByLabel("Provider ID")).toHaveValue("local-openai");
    await expect(dialog.getByLabel("显示名称")).toHaveValue("Local OpenAI");
    await expect(dialog.getByLabel("Base URL", { exact: true })).toHaveValue("http://127.0.0.1:1234/v1");
    await expect(dialog.getByLabel("默认模型")).toHaveValue("local-model");
    await expect(dialog.getByLabel("不需要 API Key")).toBeChecked();
    await expect(dialog.getByLabel("Aliases")).toHaveValue("local-lab");

    await dialog.getByLabel("显示名称").fill("Local OpenAI Lab");
    await dialog.getByLabel("默认模型").fill("local-model-v2");
    await dialog.getByLabel("Aliases").fill("local-lab, local-v2");
    await dialog.getByRole("button", { name: "更新 Provider" }).click();

    const upsertArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertProviderProfileArgs;
    });
    expect(upsertArgs.input).toMatchObject({
      id: "local-openai",
      label: "Local OpenAI Lab",
      base_url: "http://127.0.0.1:1234/v1",
      api_key_env: [],
      default_model: "local-model-v2",
      aliases: ["local-lab", "local-v2"],
    });
    await expect(dialog.getByTestId("settings-provider-row").filter({ hasText: "Local OpenAI Lab" })).toBeVisible();
    await expect(dialog.getByTestId("settings-provider-row").filter({ hasText: "Local OpenAI等待密钥" })).toHaveCount(0);
  });

  test("settings models applies a custom provider template", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");

    await dialog.getByRole("button", { name: "新增自定义 Provider" }).click();
    await dialog.getByLabel("模板").selectOption("nvidia-nim");

    await expect(dialog.getByLabel("Provider ID")).toHaveValue("nvidia");
    await expect(dialog.getByLabel("显示名称")).toHaveValue("NVIDIA NIM");
    await expect(dialog.getByLabel("Base URL", { exact: true })).toHaveValue("https://integrate.api.nvidia.com/v1");
    await expect(dialog.getByLabel("默认模型")).toHaveValue("nvidia/llama-3.1-nemotron");
    await expect(dialog.getByLabel("API Key env")).toHaveValue("NVIDIA_API_KEY");
    await expect(dialog.getByLabel("Base URL env", { exact: true })).toHaveValue("NVIDIA_BASE_URL");
    await expect(dialog.getByLabel("Aliases")).toHaveValue("nim");
    await expect(dialog.getByLabel("不需要 API Key")).not.toBeChecked();

    await dialog.getByRole("button", { name: "保存 Provider" }).click();
    const upsertArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertProviderProfileArgs;
    });
    expect(upsertArgs.input).toMatchObject({
      id: "nvidia",
      label: "NVIDIA NIM",
      transport: "openai_chat_completions",
      base_url: "https://integrate.api.nvidia.com/v1",
      api_key_env: ["NVIDIA_API_KEY"],
      base_url_env: ["NVIDIA_BASE_URL"],
      default_model: "nvidia/llama-3.1-nemotron",
      aliases: ["nim"],
      supports_tools: true,
      supports_streaming: true,
    });
    await expect(dialog.getByTestId("settings-provider-row").filter({ hasText: "NVIDIA NIM" })).toBeVisible();
  });

  test("settings models disables provider probe buttons while a probe is running", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      const originalMockIpc = window.__tauriMockIPC;
      let releaseProbe: (() => void) | undefined;
      // @ts-expect-error acceptance mock
      window.__heldProviderProbe = {
        release: () => releaseProbe?.(),
      };
      // @ts-expect-error acceptance mock
      window.__tauriMockIPC = async (command, args) => {
        if (command === "probe_provider") {
          // @ts-expect-error acceptance mock
          window.__lastProbeProviderArgs = args;
          // @ts-expect-error acceptance mock
          window.__providerProbeRequestCount = Number(window.__providerProbeRequestCount ?? 0) + 1;
          await new Promise<void>((resolve) => {
            releaseProbe = resolve;
          });
          return {
            provider: "deepseek",
            provider_label: "DeepSeek",
            model: "deepseek-v4-flash[1m]",
            base_url: "https://api.deepseek.com/anthropic",
            status: "passed",
            checks: [
              { id: "key_present", label: "Key present", status: "passed", message: "API key is present." },
              { id: "base_url_reachable", label: "Base URL reachable", status: "passed", message: "Provider endpoint returned an HTTP response." },
              { id: "model_accepted", label: "Model accepted", status: "passed", message: "Model accepted streaming request." },
              { id: "streaming_accepted", label: "Streaming accepted", status: "passed", message: "Streaming SSE response confirmed." },
              { id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed", message: "Tool schema accepted." },
            ],
            message: "DeepSeek probe passed.",
            remediation: null,
          };
        }
        return originalMockIpc(command, args);
      };
    });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const deepSeekRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });
    const openAiRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "OpenAI等待密钥" });
    const deepSeekProbe = deepSeekRow.getByRole("button", { name: "检测 DeepSeek" });
    const openAiProbe = openAiRow.getByRole("button", { name: "检测 OpenAI" });

    await expect(openAiProbe).toBeEnabled();
    await deepSeekProbe.click();
    await expect(deepSeekProbe).toBeDisabled();
    await expect(openAiProbe).toBeDisabled();

    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__heldProviderProbe.release();
    });

    await expect(deepSeekRow).toContainText("DeepSeek probe passed.");
    await expect(openAiProbe).toBeEnabled();
  });

  test("settings models surfaces mocked unsupported-tool provider probe", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockProviderProbeResult = {
        provider: "deepseek",
        provider_label: "DeepSeek",
        model: "deepseek-v4-flash[1m]",
        base_url: "https://api.deepseek.com/anthropic",
        status: "failed",
        checks: [
          { id: "key_present", label: "Key present", status: "passed", message: "API key is present." },
          { id: "base_url_reachable", label: "Base URL reachable", status: "passed", message: "Provider endpoint returned an HTTP response." },
          { id: "model_accepted", label: "Model accepted", status: "passed", message: "Model was accepted before tool validation." },
          { id: "streaming_accepted", label: "Streaming accepted", status: "passed", message: "Streaming request was accepted before tool validation." },
          {
            id: "tool_schema_accepted",
            label: "Tool schema accepted",
            status: "failed",
            message: "Provider rejected the no-op tool schema: This model does not support tools.",
          },
        ],
        message: "DeepSeek tool schema unsupported.",
        remediation: "Use a DeepSeek model or endpoint that accepts tool/function schemas.",
      };
    });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "DeepSeek" });

    await providerRow.getByRole("button", { name: "检测 DeepSeek" }).click();
    await expect(providerRow).toContainText("DeepSeek tool schema unsupported.");
    await expect(providerRow).toContainText("Provider rejected the no-op tool schema");
    await expect(providerRow).toContainText("Use a DeepSeek model or endpoint");
  });

  test("settings models allows probing an unconfigured provider row", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockProviderProbeResult = {
        provider: "openai",
        provider_label: "OpenAI",
        model: "gpt-4o",
        base_url: "https://api.openai.com/v1",
        status: "failed",
        checks: [
          { id: "key_present", label: "Key present", status: "failed", message: "API key is missing." },
          { id: "base_url_reachable", label: "Base URL reachable", status: "failed", message: "Not run because the API key is missing." },
          { id: "model_accepted", label: "Model accepted", status: "failed", message: "Not run because the API key is missing." },
          { id: "streaming_accepted", label: "Streaming accepted", status: "failed", message: "Not run because the API key is missing." },
          { id: "tool_schema_accepted", label: "Tool schema accepted", status: "failed", message: "Not run because the API key is missing." },
        ],
        message: "OpenAI API key is missing.",
        remediation: "Add an OpenAI API key, then run the probe again.",
      };
    });

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    const providerRow = dialog.getByTestId("settings-provider-row").filter({ hasText: "OpenAI等待密钥" });

    await expect(providerRow).toContainText("未配置");
    await providerRow.getByRole("button", { name: "检测 OpenAI" }).click();
    await expect(providerRow).toContainText("OpenAI API key is missing.");

    const probeArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastProbeProviderArgs;
    });
    expect(probeArgs).toEqual({ provider: "openai" });
  });

  test("settings tools supports permission allow deny and reset round-trip", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockPermissionRules = [
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
    const writeRule = panel.getByTestId("settings-permission-rule-write_to_file");
    await expect(writeRule).toContainText("允许");

    await writeRule.getByRole("button", { name: "拒绝 write_to_file" }).click();
    await expect(writeRule).toContainText("拒绝");
    const setArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionRuleArgs;
    });
    expect(setArgs).toEqual({ toolName: "write_to_file", decision: "deny" });

    await writeRule.getByRole("button", { name: "重置 write_to_file" }).click();
    await expect(writeRule).toContainText("默认");
    const resetArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastResetPermissionRuleArgs;
    });
    expect(resetArgs).toEqual({ toolName: "write_to_file" });
  });

  test("settings tools can trust the current project and restore manual confirmation", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = "trust-current-project-session";
    });

    const composer = page.getByTestId("empty-start-composer");
    await composer.getByRole("textbox").fill("准备信任当前项目");
    await composer.getByRole("textbox").press("Enter");
    await expect(page.getByTestId("user-message").last()).toContainText("准备信任当前项目");

    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "工具" }).click();

    const mode = dialog.getByTestId("settings-permission-mode");
    await expect(mode).toContainText("手动确认");
    await mode.getByRole("button", { name: "信任当前项目" }).click();
    await expect(mode).toContainText("已信任");
    await expect(mode).toContainText("信任当前项目");

    const trustArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionModeArgs;
    });
    expect(trustArgs).toMatchObject({
      sessionId: "trust-current-project-session",
      mode: "trust_current_project",
      workspacePath: "/Users/cabbos/project/forge",
    });

    await mode.getByRole("button", { name: "恢复手动确认" }).click();
    await expect(mode).toContainText("手动确认");
    const restoreArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetPermissionModeArgs;
    });
    expect(restoreArgs).toMatchObject({
      sessionId: "trust-current-project-session",
      mode: "manual_confirm",
      workspacePath: "/Users/cabbos/project/forge",
    });
  });

  test("settings tools surfaces ecosystem health, tool inventory, search, and toggles", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "工具" }).click();

    const manager = dialog.getByTestId("capability-manager");
    await expect(manager.getByTestId("capability-summary-strip")).toContainText("已启用");
    await expect(manager.getByTestId("capability-summary-strip")).toContainText("5/6");
    await expect(manager.getByTestId("capability-summary-strip")).toContainText("可用工具");
    await expect(manager.getByTestId("capability-summary-strip")).toContainText("2/3");

    await expect(manager).toContainText("Code Review");
    await expect(manager).toContainText("Skill metadata loaded.");
    await manager.getByRole("button", { name: "Code Review 详情" }).click();
    await expect(dialog.getByRole("dialog", { name: "Code Review 详情" })).toContainText("正常");
    await dialog.getByRole("button", { name: "关闭详情" }).click();

    await manager.getByRole("tab", { name: /模型/ }).click();
    await expect(manager).toContainText("DeepSeek");
    await expect(manager).toContainText("API key configured");
    await expect(manager).toContainText("OpenAI");
    await expect(manager).toContainText("API key missing");
    await manager.getByRole("button", { name: "OpenAI 详情" }).click();
    await expect(dialog.getByRole("dialog", { name: "OpenAI 详情" })).toContainText("模型服务");
    await expect(dialog.getByRole("dialog", { name: "OpenAI 详情" })).toContainText("Default model: gpt-4o");
    await dialog.getByRole("button", { name: "关闭详情" }).click();

    await manager.getByRole("tab", { name: /连接/ }).click();
    await expect(manager).toContainText("Obsidian MCP");
    await expect(manager).toContainText("Token is missing.");
    await manager.getByRole("button", { name: "Obsidian MCP 详情" }).click();
    await expect(dialog.getByRole("dialog", { name: "Obsidian MCP 详情" })).toContainText("command: obsidian-mcp --stdio");
    await dialog.getByRole("button", { name: "关闭详情" }).click();

    const search = manager.getByRole("textbox", { name: "搜索连接" });
    await search.fill("linear");
    await expect(manager).toContainText("没有匹配的连接");
    await search.fill("obsidian");
    await expect(manager).toContainText("Obsidian MCP");

    await manager.getByRole("button", { name: "Obsidian MCP已启用" }).click();
    await expect(manager.getByRole("button", { name: "Obsidian MCP已停用" })).toBeVisible();
    const toggleArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastToggleCapabilityArgs;
    });
    expect(toggleArgs).toEqual({ capabilityId: "mcp:obsidian", enabled: false });
  });

  test("settings memory follows the active profile scope", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "资料" }).click();

    const workProfile = dialog.locator(".forge-memory-fact", { hasText: "Work profile" });
    await expect(workProfile).toContainText("openai");
    await workProfile.getByRole("button", { name: "设为活跃" }).click();
    await expect(workProfile.getByRole("button", { name: "当前活跃" })).toBeVisible();
    const activeProfileArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastSetActiveProfileArgs;
    });
    expect(activeProfileArgs).toEqual({ id: "work" });

    await dialog.getByRole("button", { name: "记忆" }).click();
    await expect(dialog).toContainText("Gateway rollout notes belong to work");
    await expect(dialog).toContainText("Work profile");
    await expect(dialog).not.toContainText("Personal tax note stays out of work");

    await dialog.getByPlaceholder("搜索记忆…").fill("personal");
    await expect(dialog).toContainText("未找到匹配项");
    await dialog.getByPlaceholder("搜索记忆…").fill("");

    await dialog.getByRole("button", { name: "新建" }).click();
    await dialog.getByPlaceholder("输入记忆事实…").fill("Acceptance profile fact");
    await dialog.getByPlaceholder("标签, 逗号分隔").fill("acceptance, profile");
    await dialog.getByRole("button", { name: "保存" }).click();
    await expect(dialog).toContainText("Acceptance profile fact");

    const upsertArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertMemoryFactArgs;
    });
    expect(upsertArgs).toMatchObject({
      input: {
        text: "Acceptance profile fact",
        tags: ["acceptance", "profile"],
        profile_id: "work",
      },
    });
  });

  test("settings profiles supports create edit and delete round-trip", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "资料" }).click();

    await expect(dialog).toContainText("3 个资料");
    await dialog.getByRole("button", { name: "新建资料" }).click();
    await dialog.getByPlaceholder("资料名称").fill("Research profile");
    await dialog.getByPlaceholder("默认服务 (可选)").fill("openai");
    await dialog.getByPlaceholder("默认模型 (可选)").fill("gpt-4o");
    await dialog.getByPlaceholder("默认工作区 (可选)").fill("/Users/test/research");
    await dialog.getByRole("button", { name: "创建" }).click();

    const createdProfile = dialog.locator(".forge-memory-fact", { hasText: "Research profile" });
    await expect(createdProfile).toBeVisible();
    await expect(createdProfile).toContainText("openai");
    await expect(createdProfile).toContainText("/Users/test/research");
    let upsertProfileArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertProfileArgs;
    });
    expect(upsertProfileArgs).toMatchObject({
      input: {
        id: null,
        name: "Research profile",
        default_provider: "openai",
        default_model: "gpt-4o",
        default_workspace: "/Users/test/research",
      },
    });

    await createdProfile.getByRole("button", { name: "编辑" }).click();
    await dialog.getByPlaceholder("资料名称").fill("Research profile updated");
    await dialog.getByPlaceholder("默认模型 (可选)").fill("gpt-4.1");
    await dialog.getByRole("button", { name: "更新" }).click();

    const updatedProfile = dialog.locator(".forge-memory-fact", { hasText: "Research profile updated" });
    await expect(updatedProfile).toBeVisible();
    await expect(updatedProfile).toContainText("gpt-4.1");
    upsertProfileArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastUpsertProfileArgs;
    });
    expect(upsertProfileArgs).toMatchObject({
      input: {
        name: "Research profile updated",
        default_provider: "openai",
        default_model: "gpt-4.1",
        default_workspace: "/Users/test/research",
      },
    });
    expect(String(upsertProfileArgs.input.id)).toBeTruthy();

    await updatedProfile.getByRole("button", { name: "删除" }).click();
    await expect(dialog.locator(".forge-memory-fact", { hasText: "Research profile updated" })).toHaveCount(0);
    const deleteProfileArgs = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__lastDeleteProfileArgs;
    });
    expect(deleteProfileArgs).toEqual({ id: upsertProfileArgs.input.id });
  });

  test("settings scheduler supports create, run, disable, and delete round-trip", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "调度" }).click();

    await expect(dialog.getByRole("heading", { name: "调度" })).toBeVisible();
    await dialog.getByRole("button", { name: "新建任务" }).click();
    await dialog.getByPlaceholder("任务名称").fill("Daily acceptance check");
    await dialog.getByPlaceholder("提示词 / 命令文本").fill("Run a compact product acceptance check.");
    await dialog.getByPlaceholder("间隔（秒），0 为手动").fill("3600");
    await dialog.getByRole("button", { name: "保存" }).click();

    const task = dialog.locator(".forge-scheduler-task-card", { hasText: "Daily acceptance check" });
    await expect(task).toBeVisible();
    await expect(task).toContainText("Run a compact product acceptance check.");

    await task.getByRole("button", { name: "立即运行" }).click();
    await task.getByText(/最近运行记录/).click();
    await expect(task).toContainText("已排队");
    await expect(task).toContainText("Queued Gateway trigger");
    const queuedTriggers = await page.evaluate(() => {
      // @ts-expect-error acceptance mock
      return window.__mockGatewayTriggers;
    });
    const expectedWorkspace = await page.evaluate(() => window.localStorage.getItem("forge-working-dir"));
    expect(queuedTriggers[0]).toMatchObject({
      message: "Run a compact product acceptance check.",
      profile_id: null,
      workspace_path: expectedWorkspace,
    });

    await task.getByRole("button", { name: "禁用" }).click();
    await expect(task).toContainText("已禁用");

    await task.getByRole("button", { name: "删除" }).click();
    await expect(dialog.locator(".forge-scheduler-task-card", { hasText: "Daily acceptance check" })).toHaveCount(0);
  });

  test("loop runtime appears in background drawer and A2A runtime facts", async ({ page }) => {
    const sessionId = "acceptance-loop-runtime-session";
    await page.evaluate((id) => {
      // @ts-expect-error acceptance mock
      window.__mockSessionId = id;
    }, sessionId);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "agent_a2a_updated",
        session_id: sessionId,
        state: {
          running_count: 0,
          completed_count: 2,
          failed_count: 0,
          interrupted_count: 0,
          tasks: [
            {
              task_id: "a2a-runtime-ui",
              agent_id: "agent-1",
              role: "implementer",
              execution_mode: "worktree_worker",
              status: "completed",
              title: "Runtime UI implementer",
              messages: [],
              latest_message: "Rendered runtime facts from stream events.",
              failure_message: null,
              updated_at_ms: Date.now(),
              artifact_count: 1,
              latest_artifact_kind: "patch_proposal",
              latest_artifact_title: "Runtime UI patch",
              needs_human_review: true,
              reason_codes: ["human_gated_commit"],
              tests_passed: true,
              diff_truncated: false,
              worktree_path: "/tmp/forge-loop-runtime",
              cleaned_up: false,
              suggested_action: "Review before merge; commit remains human-gated.",
              parent_task_id: null,
              child_task_ids: ["a2a-runtime-usage", "a2a-runtime-review"],
              created_at_ms: Date.now() - 60_000,
              started_at_ms: Date.now() - 58_000,
              ended_at_ms: Date.now() - 1_000,
              duration_ms: 57_000,
              retryable: null,
              failure_kind: null,
              resume_note: null,
              latest_progress: null,
              lease_owner: null,
              lease_acquired_at_ms: null,
              lease_expires_at_ms: null,
              last_heartbeat_at_ms: null,
              attempt_count: 1,
              max_attempts: 3,
              diff_available: true,
              changed_file_count: 2,
              changed_files: ["apps/desktop/src/lib/loopRuntime.ts", "apps/desktop/src/components/loop/LoopTaskPanel.tsx"],
              test_report_excerpt: "node --test apps/desktop/src/lib/loopRuntime.test.ts passed",
            },
            {
              task_id: "a2a-runtime-usage",
              agent_id: "agent-2",
              role: "reviewer",
              execution_mode: "worktree_worker",
              status: "completed",
              title: "Runtime usage auditor",
              messages: [],
              latest_message: "Usage facts recorded with unknown output/cost.",
              failure_message: null,
              updated_at_ms: Date.now(),
              artifact_count: 0,
              latest_artifact_kind: null,
              latest_artifact_title: null,
              needs_human_review: false,
              reason_codes: [],
              tests_passed: null,
              diff_truncated: null,
              worktree_path: null,
              cleaned_up: null,
              suggested_action: null,
              parent_task_id: "a2a-runtime-ui",
              child_task_ids: [],
              created_at_ms: Date.now() - 50_000,
              started_at_ms: Date.now() - 48_000,
              ended_at_ms: Date.now() - 1_000,
              duration_ms: 47_000,
              retryable: null,
              failure_kind: null,
              resume_note: null,
              latest_progress: null,
              lease_owner: null,
              lease_acquired_at_ms: null,
              lease_expires_at_ms: null,
              last_heartbeat_at_ms: null,
              attempt_count: 1,
              max_attempts: 3,
              diff_available: null,
              changed_file_count: null,
              changed_files: [],
              test_report_excerpt: null,
            },
          ],
        },
      },
      {
        event_type: "subagent_runtime_event",
        session_id: sessionId,
        loop_task_id: "loop-runtime-ui",
        task_id: "a2a-runtime-ui",
        event: { type: "file_io", operation: "diff_observed", path: "apps/desktop/src/components/loop/LoopTaskPanel.tsx" },
      },
      {
        event_type: "subagent_runtime_event",
        session_id: sessionId,
        loop_task_id: "loop-runtime-ui",
        task_id: "a2a-runtime-usage",
        event: {
          type: "usage_recorded",
          model: "claude-sonnet",
          source: "anthropic",
          reason: "pricing_unknown",
          input_tokens: 3200,
          output_tokens: null,
          estimated_cost_micros: null,
        },
      },
      {
        event_type: "loop_runtime_updated",
        session_id: sessionId,
        loop_task_id: "loop-runtime-ui",
        task: {
          id: "loop-runtime-ui",
          goal: "Ship Runtime UI and Dashboard Consumption",
          session_id: sessionId,
          profile_id: null,
          workspace_path: "/Users/cabbos/project/forge",
          status: "waiting_for_input",
          owner: { kind: "gateway" },
          policy: {},
          headless_resume_mode: "approved_for_task",
          headless_resume_approval: {
            task_id: "loop-runtime-ui",
            approved_by: "human-reviewer",
            approved_at_ms: Date.now() - 60_000,
            scope: "task",
            expires_at_ms: Date.now() + 86_400_000,
          },
          budget: {},
          completion_contract: {},
          created_at_ms: Date.now() - 120_000,
          updated_at_ms: Date.now(),
          lease: null,
          open_gates: [],
          evidence: [],
          policy_decisions: [],
          latest_budget_snapshot: {
            budget_exceeded: true,
            model_rounds_used: 6,
            tool_calls_used: 18,
            elapsed_ms: 95_000,
            has_unknown_cost: true,
          },
          latest_event_id: "evt-loop-runtime-review",
          outcome: { message: "Review approved; commit remains human-gated." },
          completion_result: {
            status: "complete",
            reasons: [],
            review_status: "approved",
            commit_eligible: true,
            commit_blockers: [],
            human_gate_id: "a2a-review-loop-runtime-ui",
            last_review_decision: {
              kind: "approved",
              decided_at_ms: Date.now(),
              decided_by: "controller",
              reason: "review passed",
            },
          },
        },
      },
    ], 1);

    await expect(page.getByTestId("background-task-status")).toContainText("1 待审阅");
    await page.getByRole("button", { name: "展开后台任务列表" }).click();
    const drawer = page.getByTestId("background-task-list");
    await expect(drawer).toContainText("Ship Runtime UI and Dashboard Consumption");
    await expect(drawer).toContainText("commit eligible after human review");
    await expect(drawer).toContainText("commit remains human-gated");
    await expect(drawer.getByTestId("loop-commit-gated")).toContainText("commit remains human-gated");
    const readiness = drawer.getByTestId("loop-headless-resume-readiness");
    await expect(readiness).toContainText("approval recorded");
    await expect(readiness).toContainText("Lease/desktop owner pending");
    await expect(readiness).not.toContainText(/will continue automatically|continue automatically|自动继续/i);
    await expect(drawer).toContainText("成本未知");
    await expect(drawer.getByRole("button", { name: /commit|merge|push|提交|合并|推送/i })).toHaveCount(0);
    await expect(drawer).not.toContainText("git commit");
    await expect(drawer).not.toContainText("git merge");
    await expect(drawer).not.toContainText("git push");

    await page.getByRole("button", { name: "打开后台任务面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    await panel.getByRole("option", { name: /^侧边任务/ }).click();
    await panel.getByRole("option", { name: /Runtime UI implementer/ }).click();
    const workbench = panel.getByRole("region", { name: "子任务 Runtime UI implementer" });
    await expect(workbench).toContainText("Runtime UI implementer");
    await expect(workbench.locator('[aria-label="子任务 2 个: a2a-runtime-usage, a2a-runtime-review"]')).toContainText("2");
    await expect(workbench).toContainText("需要人工审阅");
    await expect(workbench.locator(".forge-a2a-runtime-facts", { hasText: "文件 IO" })).toContainText("LoopTaskPanel.tsx");

    await panel.getByRole("button", { name: "新建工作面板标签" }).click();
    await panel.getByRole("option", { name: /^侧边任务/ }).click();
    await panel.getByRole("option", { name: /Runtime usage auditor/ }).click();
    const usageTask = panel.getByRole("region", { name: "子任务 Runtime usage auditor" });
    await expect(usageTask.locator(".forge-a2a-runtime-facts", { hasText: "用量" })).toContainText("input 3200 / output unknown / cost unknown");
    await expect(usageTask.locator(".forge-a2a-runtime-facts", { hasText: "用量" })).toContainText("pricing unknown");
  });
});
