import { test, expect, type Page } from "@playwright/test";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

/** Setup: inject mock IPC before the app loads */
async function setup(page: Page) {
  await page.addInitScript(() => {
    let callbackId = 0;
    const callbacks = new Map<number, (data: unknown) => void>();
    const workingDir = "/Users/cabbos/project/forge";
    window.localStorage.setItem("forge-working-dir", workingDir);
    const projectRuntimeStatus = {
      working_dir: workingDir,
      has_package_json: true,
      package_manager: "npm",
      dev_script: "dev",
      command: "npm run dev",
      port: 1420,
      url: "http://localhost:1420",
      running: false,
      managed: false,
      pid: null,
      can_start: true,
      can_stop: false,
      can_open: true,
      message: "Preview not running",
      logs: [],
    };
    const projectCheckpointStatus = {
      working_dir: workingDir,
      is_git_repo: true,
      dirty: false,
      last_checkpoint: null,
      message: "No checkpoint yet",
    };
    let forgeWikiExists = false;
    const forgeWikiPages = [
      {
        id: "index",
        project_path: workingDir,
        path: "index.md",
        title: "项目概览",
        kind: "index",
        summary: "项目目标、边界和当前结构。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
      {
        id: "tasks",
        project_path: workingDir,
        path: "tasks.md",
        title: "当前任务",
        kind: "tasks",
        summary: "当前任务、验收步骤和后续事项。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
      {
        id: "decisions",
        project_path: workingDir,
        path: "decisions.md",
        title: "决策记录",
        kind: "decisions",
        summary: "重要方案和取舍。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
    ];
    const forgeWikiProposals = new Map<string, Record<string, unknown>>();
    const forgeWikiState = (projectPath: string, exists: boolean) => ({
      project_path: projectPath,
      exists,
      wiki_dir: `${projectPath}/.forge/wiki`,
      pages: exists ? forgeWikiPages.map((page) => ({ ...page, project_path: projectPath })) : [],
      message: exists ? "项目记录已就绪。" : "还没有项目记录",
    });
    const forgeWikiProposal = (projectPath: string, args: Record<string, unknown>) => ({
      id: String(args.proposalId ?? args.id ?? "forge-wiki-proposal"),
      project_path: projectPath,
      session_id: typeof args.sessionId === "string" ? args.sessionId : null,
      target_pages: Array.isArray(args.targetPages) ? args.targetPages.map(String) : ["tasks.md"],
      title: String(args.title ?? "记录项目进展"),
      summary: String(args.summary ?? "补充本轮任务产生的项目记录。"),
      patch_preview: typeof args.patchPreview === "string" ? args.patchPreview : null,
      status: "pending",
      created_at: "2026-05-13T00:00:00.000Z",
    });
    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
      const projectPath = String(args.projectPath ?? workingDir);
      switch (cmd) {
        case "create_session":
          // @ts-expect-error mock
          window.__lastCreateSessionArgs = args;
          // @ts-expect-error mock
          return {
            session_id: window.__mockSessionId ?? crypto.randomUUID(),
            provider: "deepseek",
            model: "deepseek-v4-flash[1m]",
            // @ts-expect-error mock
            missing_api_key: Boolean(window.__mockMissingApiKey),
          };
        case "send_input":
          // @ts-expect-error mock
          window.__lastSentText = args.text;
          return undefined;
        case "kill_session":
        case "confirm_response":
        case "set_api_key":
          return undefined;
        case "list_sessions":
          return [];
        case "get_default_working_dir":
          return workingDir;
        case "list_capabilities":
          return [
            { id: "read_file", name: "File Reader", description: "Read files", kind: "tool", source: "builtin", version: "1.0", enabled: true },
            { id: "code-review", name: "Code Review", description: "Review code", kind: "skill", source: "github", version: "1.2", enabled: true },
          ];
        case "toggle_capability":
          return undefined;
        case "get_api_key_status":
          return [{ provider: "deepseek", set: true, preview: "sk-e0...23ef" }];
        case "get_project_runtime_status":
          return projectRuntimeStatus;
        case "get_project_checkpoint_status":
          return projectCheckpointStatus;
        case "list_memories":
          return [];
        case "get_workflow_state":
          return null;
        case "override_workflow_route":
          return {
            session_id: String(args.sessionId ?? "session"),
            route: args.action === "debug" ? "recovery" : args.action === "verify" ? "verification" : args.action === "plan_first" ? "workflow" : "direct",
            phase: args.action === "debug" ? "debugging" : args.action === "verify" ? "verifying" : args.action === "plan_first" ? "clarifying" : "idle",
            beginner_label: args.action === "debug" ? "遇到问题，正在排查" : args.action === "verify" ? "正在检查结果" : args.action === "plan_first" ? "先梳理想法" : "直接回答",
            developer_label: String(args.action ?? "direct"),
            matched_signals: ["manual override"],
            reason: "用户手动切换了当前工作方式。",
            gate: "none",
            override_actions: ["direct", "plan_first", "debug", "verify"],
            spec_path: null,
            plan_path: null,
            checkpoint_id: null,
            updated_at: Date.now(),
          };
        case "get_forge_wiki_state":
          return forgeWikiState(projectPath, forgeWikiExists);
        case "init_forge_wiki":
          forgeWikiExists = true;
          return forgeWikiState(projectPath, true);
        case "list_forge_wiki_pages":
          return forgeWikiExists ? forgeWikiPages.map((page) => ({ ...page, project_path: projectPath })) : [];
        case "read_forge_wiki_page":
          return args.pagePath === "tasks.md" ? "# 当前任务\n\n覆盖项目档案面板。" : "# 项目概览\n\n项目记录预览。";
        case "select_forge_wiki_context":
          return [
            {
              page_id: "tasks",
              title: "当前任务",
              path: "tasks.md",
              kind: "tasks",
              summary: "当前任务、验收步骤和后续事项。",
              score: 0.96,
              reason: "和当前请求最相关",
              injected: true,
            },
          ];
        case "create_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposal(projectPath, args);
          forgeWikiProposals.set(String(proposal.id), proposal);
          return proposal;
        }
        case "accept_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
          const accepted = { ...proposal, status: "accepted" };
          forgeWikiProposals.set(String(accepted.id), accepted);
          return accepted;
        }
        case "discard_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
          const discarded = { ...proposal, status: "discarded" };
          forgeWikiProposals.set(String(discarded.id), discarded);
          return discarded;
        }
        default:
          return undefined;
      }
    };
    // @ts-expect-error mock
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "plugin:event|listen") {
          // @ts-expect-error listeners
          if (!window.__tauriListeners[args.event as string]) window.__tauriListeners[args.event as string] = [];
          const callback = callbacks.get(args.handler as number);
          if (callback) {
            // @ts-expect-error listeners
            window.__tauriListeners[args.event as string].push(callback);
          }
          return args.handler;
        }
        if (cmd === "plugin:event|unlisten") {
          const event = args.event as string;
          const id = args.eventId as number;
          // @ts-expect-error listeners
          window.__tauriListeners[event] = (window.__tauriListeners[event] ?? []).filter((fn: unknown) => fn !== callbacks.get(id));
          callbacks.delete(id);
          return undefined;
        }
        return window.__tauriMockIPC?.(cmd, args);
      },
      transformCallback: (callback: (data: unknown) => void) => {
        callbackId += 1;
        callbacks.set(callbackId, callback);
        return callbackId;
      },
      unregisterCallback: (id: number) => {
        callbacks.delete(id);
      },
      callbacks,
    };
    // @ts-expect-error mock
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (_event: string, id: number) => {
        callbacks.delete(id);
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

function projectArchive(page: Page) {
  return page.locator("aside").last();
}

async function expandArchiveRecords(page: Page) {
  const archive = projectArchive(page);
  const records = archive.getByTestId("archive-disclosure-records");
  await records.getByRole("button").click();
  return records;
}

async function expandArchiveFiles(page: Page) {
  const archive = projectArchive(page);
  const files = archive.getByTestId("archive-disclosure-files");
  await files.getByRole("button").click();
  return files;
}

test.describe("Timeline Message Flow", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("app loads and shows empty state", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(page.getByTestId("app-titlebar")).toHaveAttribute("data-tauri-drag-region", "true");
    await expect(main.locator("p", { hasText: "从当前对话开始" })).toBeVisible();
    await expect(main.getByText("Forge 会带着项目档案，把结果推进到可预览、可检查、可继续。")).toHaveCount(0);
    await expect(main.getByText("当前任务", { exact: true })).toHaveCount(0);
    await expect(main.getByText("交付", { exact: true })).toHaveCount(0);
    await expect(main.getByText("创建一个任务开始")).toHaveCount(0);
  });

  test("creating a session shows chat input", async ({ page }) => {
    await page.goto("http://localhost:1420");
    // Click new session button
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    // Input should appear
    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByText("运行中", { exact: true })).toHaveCount(0);
    const composer = page.getByTestId("composer-lane");
    await expect(composer).toBeVisible();
    await expect(composer.getByRole("button", { name: "引用文件" })).toBeVisible();
    await expect(composer.getByRole("button", { name: "常用请求" })).toBeVisible();
    await expect(page.getByRole("button", { name: "我想做一个番茄钟小工具，可以开始、暂停、重置。" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "我想做一个记账小工具，先能记录一笔收入或支出。" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "我想做一个文案小工具，输入主题后生成一版短文案。" })).toHaveCount(0);
    await expect(page.getByText("可以继续描述任务")).toHaveCount(0);
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
    await page.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "模型服务" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "本机数据" })).toBeVisible();
    await expect(page.getByText("API Key")).toHaveCount(0);
    await expect(page.getByText("~/.forge/config.json")).toHaveCount(0);
    await expect(page.getByText("IndexedDB")).toHaveCount(0);
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

  test("settings show provider defaults and context window quietly", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();

    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "模型服务" })).toBeVisible();
    const deepseek = dialog.locator("section").filter({ hasText: "DeepSeek" });
    await expect(deepseek.getByText("DeepSeek V4 Flash 1M")).toBeVisible();
    await expect(deepseek.getByText("默认模型 · 上下文 1M")).toBeVisible();
    await expect(deepseek.getByText("deepseek-v4-flash[1m]")).toHaveCount(0);
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

    await expect(page.getByRole("button", { name: /思考记录/ })).toBeVisible({ timeout: 5000 });
    await expect(page.getByText("I'll create a fibonacci function.")).toBeVisible();

    // Tool card should show write_to_file
    await expect(page.locator("text=write_to_file")).toBeVisible({ timeout: 5000 });

    // Shell card should show terminal output
    await expect(page.locator("text=python test.py")).toBeVisible();

    // Final text should be visible
    await expect(page.locator("text=The fibonacci function works correctly")).toBeVisible();
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
    const toolTrigger = page.getByTestId("tool-card-trigger").first();
    const shellTrigger = page.getByTestId("shell-card-trigger").first();
    await expect(thinkingTrigger).toBeVisible();
    await expect(toolTrigger).toBeVisible();
    await expect(shellTrigger).toBeVisible();

    const widths = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']")?.getBoundingClientRect();
      const tool = document.querySelector("[data-testid='tool-card-trigger']")?.getBoundingClientRect();
      const shell = document.querySelector("[data-testid='shell-card-trigger']")?.getBoundingClientRect();
      return thinking && tool && shell
        ? { thinking: Math.round(thinking.width), tool: Math.round(tool.width), shell: Math.round(shell.width) }
        : null;
    });
    expect(widths).not.toBeNull();
    expect(widths!.thinking).toBeLessThanOrEqual(220);
    expect(widths!.tool).toBeLessThanOrEqual(520);
    expect(widths!.shell).toBeLessThanOrEqual(520);
    await expect(thinkingTrigger).toHaveCSS("border-top-width", "0px");

    await toolTrigger.click();
    await expect(page.getByRole("button", { name: "复制工具输出" }).first()).toBeVisible();
    await shellTrigger.click();
    await expect(page.getByRole("button", { name: "复制命令输出" }).first()).toBeVisible();
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
    await expect(userMessage).toHaveCSS("border-top-width", "0px");
    const userRadius = await userMessage.evaluate((node) =>
      Number.parseFloat(getComputedStyle(node).borderTopLeftRadius),
    );
    expect(userRadius).toBeLessThanOrEqual(8);
    await expect(page.getByTestId("assistant-message").last()).toHaveCSS("border-top-width", "0px");
  });

  test("scroll-to-bottom control stays quiet and editor-like", async ({ page }) => {
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
      { event_type: "text_start" as const, session_id: sessionId, block_id: `scroll-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `scroll-${index}`,
        content: `第 ${index + 1} 条输出，用来撑开滚动区域。这里保持足够长度，让对话区出现滚动。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `scroll-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, events, 1);

    await page.evaluate(() => {
      const lane = document.querySelector("[data-testid='message-lane']");
      const scroller = lane?.parentElement;
      if (!scroller) return;
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    const control = page.getByTestId("scroll-to-bottom");
    await expect(control).toBeVisible();
    const metrics = await control.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        radius: Number.parseFloat(style.borderTopLeftRadius),
        shadow: style.boxShadow,
        width: Math.round(node.getBoundingClientRect().width),
        height: Math.round(node.getBoundingClientRect().height),
      };
    });
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.shadow).toBe("none");
    expect(metrics.width).toBe(28);
    expect(metrics.height).toBe(28);
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

    await page.evaluate(() => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "send_input") {
          await new Promise((resolve) => setTimeout(resolve, 500));
          return undefined;
        }
        return original?.(cmd, args);
      };
    });

    await page.locator("textarea").fill("继续把对话区域靠近 Codex。");
    await page.locator("textarea").press("Enter");

    const pending = page.getByTestId("pending-block");
    await expect(pending).toBeVisible();
    await expect(pending).toHaveText(/正在处理/);
    await expect(pending).toHaveCSS("border-top-width", "0px");
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

    const metrics = await page.evaluate(() => {
      const surface = document.querySelector("[data-testid='composer-surface']");
      const send = document.querySelector("[data-testid='composer-send']");
      if (!surface || !send) return null;
      const surfaceStyle = getComputedStyle(surface);
      const sendStyle = getComputedStyle(send);
      return {
        surfaceShadow: surfaceStyle.boxShadow,
        surfaceRadius: Number.parseFloat(surfaceStyle.borderTopLeftRadius),
        sendRadius: Number.parseFloat(sendStyle.borderTopLeftRadius),
        sendBackground: sendStyle.backgroundColor,
        sendWidth: Math.round(send.getBoundingClientRect().width),
        sendHeight: Math.round(send.getBoundingClientRect().height),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.surfaceShadow).toBe("none");
    expect(metrics!.surfaceRadius).toBeLessThanOrEqual(8);
    expect(metrics!.sendRadius).toBeLessThanOrEqual(8);
    expect(metrics!.sendBackground).not.toBe("rgb(212, 168, 83)");
    expect(metrics!.sendWidth).toBe(28);
    expect(metrics!.sendHeight).toBe(28);
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
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["src/App.tsx"],
          risk_level: "low",
          checkpoint_status: "ready",
          command: null,
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(card.getByText("准备修改项目")).toBeVisible();
    await expect(card.getByText("目标项目", { exact: true })).toBeVisible();
    await expect(card.getByText("forge")).toBeVisible();
    await expect(card.getByText("/Users/cabbos/project/forge")).toBeVisible();
    await expect(card.getByText("写入文件")).toBeVisible();
    await expect(card.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "继续" })).toBeVisible();
    await expect(card.getByRole("button", { name: "取消" })).toBeVisible();
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
    await expect(panels.filter({ hasText: "准备修改项目" })).toBeVisible();
    await expect(panels.filter({ hasText: "文件改动" })).toContainText("src/App.tsx");
    await expect(panels.filter({ hasText: "文件改动" }).getByRole("button", { name: "复制文件改动" })).toBeVisible();
    await expect(panels.filter({ hasText: "本轮交付" })).toBeVisible();

    const widths = await panels.evaluateAll((nodes) =>
      nodes.map((node) => Math.round(node.getBoundingClientRect().width)),
    );
    expect(widths.every((width) => width <= 780)).toBeTruthy();
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
    const thinkingTrigger = page.getByRole("button", { name: /思考记录/ });
    await expect(thinkingTrigger).toBeVisible({ timeout: 5000 });

    // Click to expand
    await thinkingTrigger.click();
    await page.waitForTimeout(200);

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
    await expect(page.getByRole("button", { name: /正在读取文件/ })).toBeVisible({ timeout: 3000 });
    await expect(page.getByText("进行中", { exact: true })).toHaveCount(0);

    // Send tool_result (done state)
    await simulateStream(page, sessionId, [
      { event_type: "tool_call_result", session_id: sessionId, block_id: toolId, result: "fn main() {}", is_error: false, duration_ms: 100 },
    ], 30);

    // Should show done
    const doneTool = page.getByRole("button", { name: /已读取文件/ });
    await expect(doneTool).toBeVisible({ timeout: 3000 });
    await expect(doneTool).toContainText("100ms");
    await expect(page.getByText("完成", { exact: true })).toHaveCount(0);
  });

  test("sidebar shows persistent navigation", async ({ page }) => {
    const sidebar = page.locator("aside").first();

    const width = (await sidebar.boundingBox())?.width ?? 0;
    expect(width).toBeGreaterThanOrEqual(212);
    expect(width).toBeLessThanOrEqual(224);
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "插件" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "自动化" })).toBeVisible();
    await expect(sidebar.getByText("当前工作空间", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("插件", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("自动化", { exact: true })).toHaveCount(0);

    await sidebar.getByRole("button", { name: "插件" }).click();
    const drawer = page.getByRole("complementary", { name: "插件" });
    await expect(drawer.getByText("插件", { exact: true }).first()).toBeVisible();
    await expect(drawer.getByRole("tab", { name: /插件/ })).toHaveAttribute("aria-selected", "true");
    await expect(drawer.getByRole("textbox", { name: "搜索插件" })).toBeVisible();
    await page.waitForTimeout(300);
    const drawerX = Math.round((await drawer.boundingBox())?.x ?? 0);
    expect(drawerX).toBe(Math.round(width));
    await expect(drawer.getByText(/[☖⎔◈●]/)).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "插件" })).toHaveCount(0);
  });

  test("global new conversation shortcut starts from the active workspace", async ({ page }) => {
    await page.keyboard.down("Control");
    await page.keyboard.press("n");
    await page.keyboard.up("Control");

    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByRole("main").getByText("选择一个项目开始")).toHaveCount(0);
  });

  test("command palette shows compact desktop shortcuts", async ({ page }) => {
    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");

    const palette = page.getByRole("dialog");
    await expect(palette.getByRole("option", { name: /新建对话/ })).toContainText("⌘N");
    await expect(palette.getByRole("option", { name: /设置/ })).toContainText("⌘,");

    await page.keyboard.press("Escape");
    await page.keyboard.down("Control");
    await page.keyboard.press(",");
    await page.keyboard.up("Control");
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  });
});

test.describe("Browser dev fallback", () => {
  test("new conversation opens an input without the Tauri runtime", async ({ page }) => {
    const dialogs: string[] = [];
    page.on("dialog", async (dialog) => {
      dialogs.push(dialog.message());
      await dialog.dismiss();
    });

    await page.goto("http://localhost:1420");
    await page.evaluate(() => {
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge-playground");
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(page.locator("textarea")).toBeVisible();
    expect(dialogs).toEqual([]);
  });
});

test.describe("Workspace Safety v0", () => {
  test("first launch asks the user to choose a workspace before creating a conversation", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    await expect(page.getByRole("main").getByText("选择一个项目开始")).toBeVisible();
    await expect(page.getByRole("button", { name: "新对话", exact: true })).toBeDisabled();
  });

  test("conversation list follows the active workspace", async ({ page }) => {
    const workspaceA = "/Users/cabbos/project/app-one";
    const workspaceB = "/Users/cabbos/project/app-two";
    const sessionA = crypto.randomUUID();
    const sessionB = crypto.randomUUID();

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspaceA, workspaceB, sessionA, sessionB }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: workspaceA, name: "app-one", path: workspaceA, lastOpenedAt: 2 },
        { id: workspaceB, name: "app-two", path: workspaceB, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(workspaceA, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionA,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
          workingDir: workspaceA,
          workspaceId: workspaceA,
        },
        {
          id: sessionB,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
          workingDir: workspaceB,
          workspaceId: workspaceB,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put([
        {
          block_id: "workspace-a-message",
          event_type: "user_message",
          content: "Build A timer",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionA}`);
      tx.objectStore("keyval").put([
        {
          block_id: "workspace-b-message",
          event_type: "user_message",
          content: "Build B dashboard",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionB}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspaceA, workspaceB, sessionA, sessionB });

    await page.reload();

    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("button", { name: /app-one/ })).toBeVisible();
    await expect(sidebar.getByText(workspaceA, { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("对话", { exact: true })).toBeVisible();
    await expect(sidebar.getByText("任务", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("Build A timer")).toBeVisible();
    await expect(sidebar.getByText("Build B dashboard")).toHaveCount(0);
    await expect(sidebar.getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);

    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");
    await expect(page.getByRole("option", { name: /Build A timer/ })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("当前项目 · app-one")).toBeVisible();
    await expect(page.getByRole("dialog").getByText("最近对话")).toBeVisible();
    await expect(page.getByRole("dialog").getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);
    await page.keyboard.press("Escape");

    const workspaceTrigger = sidebar.getByRole("button", { name: /app-one/ });
    await expect(workspaceTrigger).toHaveAttribute("aria-haspopup", "menu");
    await workspaceTrigger.click();
    const workspaceMenu = page.getByRole("menu", { name: "项目工作空间" });
    await expect(workspaceMenu).toBeVisible();
    await expect(workspaceMenu.getByRole("menuitemradio", { name: /app-one/ })).toHaveAttribute("aria-checked", "true");
    await expect(workspaceMenu.getByRole("menuitemradio", { name: /app-two/ })).toHaveAttribute("aria-checked", "false");
    await expect(sidebar.getByText(workspaceA, { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText(workspaceB, { exact: true })).toHaveCount(0);
    await workspaceMenu.getByRole("menuitemradio", { name: /app-two/ }).click();

    await expect(sidebar.getByRole("button", { name: /app-two/ })).toBeVisible();
    await expect(sidebar.getByText("Build B dashboard")).toBeVisible();
    await expect(sidebar.getByText("Build A timer")).toHaveCount(0);
    await expect(sidebar.getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);
  });

  test("conversation list supports keyboard navigation", async ({ page }) => {
    const workspace = "/Users/cabbos/project/forge";
    const sessions = [
      { id: "keyboard-a", title: "Build alpha tool" },
      { id: "keyboard-b", title: "Build beta tool" },
      { id: "keyboard-c", title: "Build gamma tool" },
    ];

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspace, sessions }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([{ id: workspace, name: "forge", path: workspace, lastOpenedAt: 3 }], "forge-workspaces");
      tx.objectStore("keyval").put(workspace, "forge-active-workspace");
      tx.objectStore("keyval").put(sessions.map((session) => ({
        id: session.id,
        agentType: "deepseek",
        model: "deepseek-v4-flash[1m]",
        contextWindowTokens: 1_000_000,
        status: "stopped",
        workflowState: null,
        workingDir: workspace,
        workspaceId: workspace,
      })), "forge-sessions");
      for (const session of sessions) {
        tx.objectStore("keyval").put([
          {
            block_id: `${session.id}-message`,
            event_type: "user_message",
            content: session.title,
            isComplete: true,
            metadata: {},
          },
        ], `forge-blocks:${session.id}`);
      }
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspace, sessions });

    await page.reload();
    const sidebar = page.locator("aside").first();
    const first = sidebar.getByRole("button", { name: "Build alpha tool", exact: true });
    const second = sidebar.getByRole("button", { name: "Build beta tool", exact: true });
    const third = sidebar.getByRole("button", { name: "Build gamma tool", exact: true });
    await expect(first).toBeVisible();
    await first.focus();

    await page.keyboard.press("ArrowDown");
    await expect(second).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(third).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(second).toBeFocused();
    await page.keyboard.press("Enter");

    await expect(page.getByRole("main").getByText("Build beta tool").last()).toBeVisible();
  });

  test("conversation list groups sessions by recency", async ({ page }) => {
    const workspace = "/Users/cabbos/project/forge";
    const now = Date.now();
    const sessions = [
      { id: "recent-today", title: "Today build", updatedAt: now },
      { id: "recent-yesterday", title: "Yesterday build", updatedAt: now - 26 * 60 * 60 * 1000 },
      { id: "recent-older", title: "Older build", updatedAt: now - 8 * 24 * 60 * 60 * 1000 },
    ];

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspace, sessions }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([{ id: workspace, name: "forge", path: workspace, lastOpenedAt: 3 }], "forge-workspaces");
      tx.objectStore("keyval").put(workspace, "forge-active-workspace");
      tx.objectStore("keyval").put(sessions.map((session) => ({
        id: session.id,
        agentType: "deepseek",
        model: "deepseek-v4-flash[1m]",
        contextWindowTokens: 1_000_000,
        status: "stopped",
        workflowState: null,
        workingDir: workspace,
        workspaceId: workspace,
        createdAt: session.updatedAt,
        updatedAt: session.updatedAt,
      })), "forge-sessions");
      for (const session of sessions) {
        tx.objectStore("keyval").put([
          {
            block_id: `${session.id}-message`,
            event_type: "user_message",
            content: session.title,
            isComplete: true,
            metadata: {},
          },
        ], `forge-blocks:${session.id}`);
      }
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspace, sessions });

    await page.reload();
    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByText("今天", { exact: true })).toBeVisible();
    await expect(sidebar.getByText("昨天", { exact: true })).toBeVisible();
    await expect(sidebar.getByText("更早", { exact: true })).toBeVisible();

    const order = await sidebar.getByRole("button").evaluateAll((nodes) =>
      nodes
        .map((node) => node.getAttribute("aria-label") ?? "")
        .filter((label) => ["Today build", "Yesterday build", "Older build"].includes(label)),
    );
    expect(order).toEqual(["Today build", "Yesterday build", "Older build"]);
  });

  test("folder picker activates new conversations", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockDirectoryPicker = async () => "/Users/cabbos/project/demo-tool";
    });
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: /选择项目/ }).click();
    await page.getByRole("menuitem", { name: "选择文件夹" }).click();

    await expect(sidebar.getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeEnabled();
  });

  test("workspace menu can remove the current project from the recent list", async ({ page }) => {
    const workspaceA = "/Users/cabbos/project/remove-one";
    const workspaceB = "/Users/cabbos/project/remove-two";
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspaceA, workspaceB }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: workspaceA, name: "remove-one", path: workspaceA, lastOpenedAt: 2 },
        { id: workspaceB, name: "remove-two", path: workspaceB, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(workspaceA, "forge-active-workspace");
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspaceA, workspaceB });

    await page.reload();
    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("button", { name: /remove-one/ })).toBeVisible();
    await sidebar.getByRole("button", { name: /remove-one/ }).click();
    await page.getByRole("menuitem", { name: "从列表移除当前项目" }).click();

    await expect(sidebar.getByRole("button", { name: /remove-two/ })).toBeVisible();
    await sidebar.getByRole("button", { name: /remove-two/ }).click();
    await expect(page.getByRole("menuitemradio", { name: /remove-one/ })).toHaveCount(0);
  });

  test("manual workspace path entry remains available as fallback", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: /选择项目/ }).click();
    await page.getByRole("menuitem", { name: "手动输入路径" }).click();

    const pathInput = page.getByLabel("项目文件夹路径");
    await expect(pathInput).toBeVisible();
    await pathInput.fill("/Users/cabbos/project/demo-tool");
    await page.getByRole("button", { name: "添加" }).click();

    await expect(sidebar.getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeEnabled();
  });

  test("workspace identity stays visible when starting a sandbox conversation", async ({ page }) => {
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    await setup(page);
    await page.addInitScript((path) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", path);
    }, sandboxPath);

    await page.goto("http://localhost:1420");

    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("button", { name: /forge-test-app/ })).toBeVisible();
    await expect(sidebar.getByText(sandboxPath, { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("新对话会创建在 forge-test-app")).toHaveCount(0);

    const workspaceBoundary = page.getByLabel("当前项目边界");
    await expect(workspaceBoundary.getByText("当前项目")).toBeVisible();
    await expect(workspaceBoundary.getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(workspaceBoundary.getByText(sandboxPath)).toHaveCount(0);
    await expect(workspaceBoundary.getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);

    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    expect(createArgs.workingDir).toBe(sandboxPath);
    const main = page.getByRole("main");
    await expect(main.getByText(`本轮会作用于 forge-test-app · ${sandboxPath}`)).toHaveCount(0);
    await expect(main.getByText("准备开始")).toBeVisible();
  });
});

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
});

test.describe("First loop v0", () => {
  test("supports the first small-tool loop skeleton", async ({ page }) => {
    const sessionId = "first-loop-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const request = "我想做一个番茄钟小工具，可以开始、暂停、重置。";
    await page.locator("textarea").fill(request);
    await page.locator("textarea").press("Enter");

    await expect(page.getByRole("main").getByText(request, { exact: true }).last()).toBeVisible();

    await page.getByTitle("打开项目档案").click();
    const archive = page.locator("aside").last();

    await expect(archive.getByText("项目档案", { exact: true }).first()).toBeVisible();
    const firstVersion = archive.locator("section").filter({ hasText: "第一版" });
    await expect(firstVersion.getByRole("heading", { name: "第一版" })).toBeVisible();
    await expect(firstVersion.getByText("可见、可点、可继续")).toBeVisible();
    await expect(firstVersion.getByText("番茄钟小工具")).toBeVisible();
    await expect(firstVersion.getByText("开始、暂停、重置")).toBeVisible();
    await expect(firstVersion.getByText("下一步", { exact: true })).toBeVisible();
    await expect(archive.getByRole("heading", { name: "本轮参考" })).toHaveCount(0);
    await expect(archive.getByText("工作台", { exact: true })).toHaveCount(0);
  });

  test("shows a delivery summary after sending a first-loop request", async ({ page }) => {
    const sessionId = "first-loop-delivery-summary";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");

    const main = page.getByRole("main");
    await expect(main.getByText("本轮交付")).toBeVisible();
    await expect(main.getByText("预览未运行")).toBeVisible();
    await expect(main.getByText("下一步", { exact: true })).toBeVisible();
  });
});

test.describe("First loop v1", () => {
  test("empty session shows start readiness", async ({ page }) => {
    const sessionId = "first-loop-readiness";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const main = page.getByRole("main");
    await expect(main.getByText("准备开始")).toBeVisible();
    const readiness = main.getByTestId("start-readiness");
    await expect(readiness).toBeVisible();
    await expect(readiness).toHaveCSS("border-top-width", "0px");
    await expect(main.getByText("工作空间")).toHaveCount(0);
    await expect(main.getByText("模型密钥")).toHaveCount(0);
    await expect(main.getByText("预览", { exact: true })).toHaveCount(0);
    await expect(main.getByText("检查点", { exact: true })).toHaveCount(0);
    await expect(main.getByText("理解目标")).toHaveCount(0);
    await expect(main.getByText("准备修改")).toHaveCount(0);
  });

  test("first loop keeps progress implicit in the conversation", async ({ page }) => {
    const sessionId = "first-loop-progress";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(page.getByText("理解目标")).toHaveCount(0);

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");

    await expect(page.getByText("正在制作")).toHaveCount(0);
    await expect(page.getByText("等你验收")).toHaveCount(0);
    await expect(page.getByText("本轮交付")).toBeVisible();
  });

  test("delivery summary offers follow-up actions", async ({ page }) => {
    const sessionId = "first-loop-delivery-actions";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");

    await expect(page.getByText("验收提示", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "检查风险" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "开始验收" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "继续优化" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "检查这版" })).toBeVisible();

    await page.getByRole("button", { name: "检查这版" }).click();
    await expect(page.locator("textarea")).toHaveValue(/检查当前版本有没有明显问题/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
  });
});

test.describe("Project Archive v1", () => {
  test("project archive hides empty loop and low-level metadata by default", async ({ page }) => {
    const sessionId = "project-archive-quiet-empty";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.getByTitle("打开项目档案").click();
    const archive = projectArchive(page);
    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    const archiveWidth = (await archive.boundingBox())?.width ?? 0;
    expect(archiveWidth).toBeLessThanOrEqual(304);
    const modalBackdropCount = await page.evaluate(() =>
      [...document.querySelectorAll("div")].filter((node) => {
        const className = String(node.getAttribute("class") ?? "");
        return className.includes("fixed inset-0") && className.includes("bg-black/20");
      }).length,
    );
    expect(modalBackdropCount).toBe(0);

    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("forge", { exact: true }).first()).toBeVisible();
    await expect(archive.getByTestId("archive-disclosure-records")).toBeVisible();
    await expect(archive.getByTestId("archive-disclosure-files")).toBeVisible();
    await expect(archive.getByText("还没有项目记录", { exact: true })).toHaveCount(0);
    await expect(archive.getByText("文件名", { exact: true })).toHaveCount(0);
    await expect(archive.getByRole("heading", { name: "第一版" })).toHaveCount(0);
    await expect(archive.getByText("小工具闭环")).toHaveCount(0);
    await expect(archive.getByText(projectPath, { exact: true })).toHaveCount(0);
    await expect(archive.getByText("上下文长度")).toHaveCount(0);
    await expect(archive.getByText("$0.00")).toHaveCount(0);
    await expect(archive.getByText("工作方式")).toHaveCount(0);

    await expandArchiveFiles(page);
    await expect(archive.getByText("文件名", { exact: true })).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "项目档案" })).toHaveCount(0);
  });

  test("restored project archive shows overview and continuation actions", async ({ page }) => {
    const sessionId = "project-archive-return-session";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, projectPath }) => {
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
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "return-user-message",
          event_type: "user_message",
          content: "我想做一个番茄钟小工具，可以开始、暂停、重置。",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "return-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: {
            summary: {
              project_path: projectPath,
              preview_label: "预览可打开",
              checkpoint_label: "检查点已就绪",
              next_action: "下一步：继续调整计时器的视觉反馈。",
            },
          },
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, projectPath });

    await page.reload();
    await page.getByTitle("打开项目档案").click();

    const archive = projectArchive(page);
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("番茄钟小工具")).toBeVisible();
    await expect(archive.getByText("预览可打开 · 检查点已就绪")).toBeVisible();
    await expect(archive.getByText("下一步：继续调整计时器的视觉反馈。")).toBeVisible();
    await expect(archive.getByRole("button", { name: "继续上次任务" })).toBeVisible();

    await archive.getByRole("button", { name: "继续上次任务" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续上次任务/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
  });
});

test.describe("Project records context panel", () => {
  test("project records panel initializes records and shows selected pages", async ({ page }) => {
    const sessionId = "forge-wiki-session";
    const projectPath = "/Users/cabbos/project/forge";
    const now = "2026-05-13T00:00:00.000Z";
    const selectedPage = {
      page_id: "tasks",
      title: "当前任务",
      path: "tasks.md",
      kind: "tasks" as const,
      summary: "覆盖当前 e2e 任务和验收步骤。",
      score: 0.97,
      reason: "和当前任务最相关",
      injected: true,
    };
    const proposal = {
      id: "proposal-1",
      project_path: projectPath,
      session_id: sessionId,
      target_pages: ["tasks.md"],
      title: "记录项目进展覆盖",
      summary: "补充上下文面板初始化、带入页面和更新建议的测试记录。",
      patch_preview: "追加 e2e 覆盖说明。",
      status: "pending" as const,
      created_at: now,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.getByTitle("打开项目档案").click();
    const recordsDisclosure = await expandArchiveRecords(page);
    const projectRecords = recordsDisclosure.locator("section").filter({ has: page.getByRole("heading", { name: "项目记录" }) });
    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });
    const updateProposals = recordsDisclosure.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

    await expect(projectRecords.getByText("还没有项目记录", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "建立项目记录" }).click();
    await expect(projectRecords.getByText(/当前任务|项目概览/).first()).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await expect(selectedContext.getByText(selectedPage.summary)).toBeVisible();
    await expect(selectedContext.getByText("已参考 1 条档案")).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    await expect(updateProposals.getByText(proposal.summary)).toBeVisible();
  });

  test("shows selected context, project records, delivery status, and scoped saved background", async ({ page }) => {
    const sessionId = "living-wiki-session";
    const projectPath = "/Users/cabbos/project/forge";
    const otherProjectPath = "/Users/cabbos/project/elsewhere";
    const now = "2026-05-13T00:00:00.000Z";
    const selectedMemory = {
      id: "memory-selected-1",
      category: "preference",
      scope: "user_profile",
      status: "accepted",
      title: "使用项目记录",
      body: "Selected background should travel with the next prompt.",
      project_path: null,
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.91,
      created_at: now,
      updated_at: now,
      last_used_at: now,
      use_count: 2,
      tags: ["context"],
    };
    const projectMemory = {
      id: "memory-project-1",
      category: "project_fact",
      scope: "project",
      status: "pinned",
      title: "项目档案",
      body: "当前项目使用项目档案查看本轮参考。",
      project_path: projectPath,
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.88,
      created_at: now,
      updated_at: now,
      last_used_at: null,
      use_count: 1,
      tags: ["wiki"],
    };
    const otherProjectMemory = {
      ...projectMemory,
      id: "memory-other-project",
      title: "Other project fact",
      body: "这条背景属于另一个项目，不应该显示。",
      project_path: otherProjectPath,
    };
    const candidateMemory = {
      ...projectMemory,
      id: "memory-candidate-1",
      category: "decision",
      status: "candidate",
      title: "建议记录项目档案变化",
      body: "This candidate should be visible before it is accepted.",
      confidence: 0.72,
      tags: ["candidate"],
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath, memories }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);

      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string) => {
        switch (cmd) {
          case "create_session":
            return { session_id: sessionId };
          case "get_default_working_dir":
            return projectPath;
          case "get_project_runtime_status":
            return {
              working_dir: projectPath,
              has_package_json: true,
              package_manager: "npm",
              dev_script: "dev",
              command: "npm run dev",
              port: 1420,
              url: "http://localhost:1420",
              running: true,
              managed: true,
              pid: 4242,
              can_start: false,
              can_stop: true,
              can_open: true,
              message: "Preview running",
              logs: [],
            };
          case "get_project_checkpoint_status":
            return {
              working_dir: projectPath,
              is_git_repo: true,
              dirty: false,
              last_checkpoint: null,
              message: "No checkpoint yet",
            };
          case "list_memories":
            return memories;
          default:
            return undefined;
        }
      };
    }, { sessionId, projectPath, memories: [selectedMemory, projectMemory, otherProjectMemory, candidateMemory] });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "memory_updated", session_id: sessionId, memory: selectedMemory },
      { event_type: "memory_updated", session_id: sessionId, memory: projectMemory },
      { event_type: "memory_updated", session_id: sessionId, memory: otherProjectMemory },
      { event_type: "memory_candidate", session_id: sessionId, memory: candidateMemory },
      {
        event_type: "memory_selection",
        session_id: sessionId,
        selected: [
          {
            memory_id: selectedMemory.id,
            title: selectedMemory.title,
            body: selectedMemory.body,
            category: selectedMemory.category,
            scope: selectedMemory.scope,
            score: 0.96,
            reason: "Relevant to the active task",
            injected: true,
          },
        ],
      },
    ], 5);

    await expect(page.getByRole("main").getByText("本轮已参考 1 条档案")).toHaveCount(0);

    await page.getByTitle("打开项目档案").click();
    const recordsDisclosure = await expandArchiveRecords(page);

    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });
    const projectMemories = recordsDisclosure.locator("section").filter({ has: page.getByRole("heading", { name: "已保存背景" }) });

    await expect(selectedContext.getByText(selectedMemory.body)).toBeVisible();
    await expect(recordsDisclosure.getByRole("heading", { name: "建议更新记录" })).toBeVisible();
    await expect(page.getByText(candidateMemory.body)).toBeVisible();
    await expect(page.getByTitle("接受")).toBeVisible();
    await expect(recordsDisclosure.getByRole("heading", { name: "项目记录", exact: true })).toBeVisible();
    await expect(projectMemories.getByText(projectMemory.body)).toBeVisible();
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByText("最近状态")).toBeVisible();
    await expect(page.getByText("预览运行中")).toBeVisible();
    await expect(page.getByText(otherProjectMemory.body)).toHaveCount(0);
  });

  test("delivery shows preview action and checkpoint next step", async ({ page }) => {
    const sessionId = "delivery-action-session";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);

      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string) => {
        switch (cmd) {
          case "create_session":
            return { session_id: sessionId };
          case "get_default_working_dir":
            return projectPath;
          case "get_project_runtime_status":
            return {
              working_dir: projectPath,
              has_package_json: true,
              package_manager: "npm",
              dev_script: "dev",
              command: "npm run dev",
              port: 1420,
              url: "http://localhost:1420",
              running: false,
              managed: false,
              pid: null,
              can_start: true,
              can_stop: false,
              can_open: true,
              message: "Preview not running",
              logs: [],
            };
          case "get_project_checkpoint_status":
            return {
              working_dir: projectPath,
              is_git_repo: true,
              dirty: false,
              last_checkpoint: null,
              message: "No checkpoint yet",
            };
          case "start_project_dev_server":
          case "create_project_checkpoint":
            return undefined;
          case "list_memories":
            return [];
          default:
            return undefined;
        }
      };
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.getByTitle("打开项目档案").click();
    const delivery = page.locator("aside").last();

    await expect(delivery.getByText("预览未运行")).toBeVisible();
    await expect(delivery.getByRole("button", { name: "启动预览" })).toBeVisible();
    await expect(delivery.getByText("还没有检查点")).toBeVisible();
    await expect(delivery.getByRole("button", { name: "创建检查点" })).toBeVisible();
  });

  test("groups suggested background and project record updates", async ({ page }) => {
    const sessionId = "memory-inbox-session";
    const projectPath = "/Users/cabbos/project/forge";
    const now = "2026-05-13T00:00:00.000Z";
    const candidateMemory = {
      id: "candidate-1",
      category: "decision" as const,
      scope: "project" as const,
      status: "candidate" as const,
      title: "项目已定方案：项目档案优先",
      body: "右侧面板优先展示当前任务和本轮参考。",
      project_path: projectPath,
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.76,
      created_at: now,
      updated_at: now,
      last_used_at: null,
      use_count: 0,
      tags: ["decision"],
    };
    const proposal = {
      id: "proposal-1",
      project_path: projectPath,
      session_id: sessionId,
      target_pages: ["tasks.md"],
      title: "记录本轮参考计划",
      summary: "补充工作方式和本轮参考的下一步。",
      patch_preview: "追加任务记录。",
      status: "pending" as const,
      created_at: now,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await page.getByTitle("打开项目档案").click();
    await expandArchiveRecords(page);

    await simulateStream(page, sessionId, [
      { event_type: "memory_candidate", session_id: sessionId, memory: candidateMemory },
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    const inbox = projectArchive(page).locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

    await expect(inbox.getByText("确认后会进入项目记录或已保存背景")).toBeVisible();
    await expect(inbox.getByText("建议保存为已保存背景")).toBeVisible();
    await expect(inbox.getByText("建议写入项目记录")).toBeVisible();
    await expect(inbox.getByText("保存位置").first()).toBeVisible();
    await expect(inbox.getByText("项目记录页面")).toBeVisible();
    await expect(inbox.getByText("tasks.md")).toBeVisible();
    await expect(inbox.getByText(candidateMemory.body)).toBeVisible();
    await expect(inbox.getByText(proposal.summary)).toBeVisible();
    await expect(inbox.getByRole("button", { name: "接受" }).first()).toBeVisible();
    await expect(inbox.getByRole("button", { name: /忘记|丢弃/ }).first()).toBeVisible();

    await inbox.getByRole("button", { name: "接受" }).last().click();
    await expect(inbox.getByText("已写入项目记录")).toBeVisible();
  });
});

test.describe("Work style controls", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
  });

  test("shows soft workflow state and allows command palette override", async ({ page }) => {
    const sessionId = "workflow-router-session";
    const softWorkflow: WorkflowState = {
      session_id: sessionId,
      route: "workflow",
      phase: "clarifying",
      beginner_label: "先梳理想法",
      developer_label: "workflow",
      matched_signals: ["multi-part request"],
      reason: "这个需求会影响多个部分。",
      gate: "soft",
      override_actions: ["direct", "plan_first", "debug", "verify"],
      spec_path: null,
      plan_path: null,
      checkpoint_id: null,
      updated_at: Date.now(),
    };

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
      { event_type: "workflow_updated", session_id: sessionId, state: softWorkflow },
    ], 5);

    await expect(page.getByTestId("workflow-status-pill")).toHaveCount(0);
    await expect(page.getByText("这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。", { exact: true })).toHaveCount(0);

    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");
    await expect(page.getByRole("option", { name: "打开项目档案" })).toBeVisible();
    await expect(page.getByRole("option", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("工作方式", { exact: true })).toHaveCount(0);
    await page.getByRole("option", { name: "排查问题" }).click();

    await page.getByTitle("打开项目档案").click();
    const currentTask = page.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });
    await expect(currentTask.getByText("排查问题", { exact: true })).toBeVisible();
    await expect(currentTask.getByText("正在定位问题")).toBeVisible();
  });
});

test.describe("Current task work style", () => {
  test("shows stable mode copy without inline override controls", async ({ page }) => {
    const sessionId = "task-mode-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "workflow_updated",
        session_id: sessionId,
        state: {
          session_id: sessionId,
          route: "workflow",
          phase: "planning",
          beginner_label: "router raw label should not be the final UI label",
          developer_label: "workflow/planning",
          matched_signals: ["new feature", "multi component"],
          reason: "用户正在规划一个新能力。",
          gate: "soft",
          override_actions: ["direct", "plan_first", "debug", "verify"],
          spec_path: null,
          plan_path: null,
          checkpoint_id: null,
          updated_at: Date.now(),
        },
      },
    ], 5);

    await page.getByTitle("打开项目档案").click();
    const currentTask = page.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });

    await expect(currentTask.getByText("拆成步骤")).toBeVisible();
    await expect(currentTask.getByText("正在拆成可执行步骤")).toBeVisible();
    await expect(currentTask.getByText("这个需求会影响多个部分")).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "直接回答" })).toHaveCount(0);
    await expect(currentTask.getByRole("button", { name: "先拆方案" })).toHaveCount(0);
    await expect(currentTask.getByRole("button", { name: "排查问题" })).toHaveCount(0);
    await expect(currentTask.getByRole("button", { name: "检查结果" })).toHaveCount(0);
    await expect(currentTask.getByText("开发者详情")).toHaveCount(0);
    await expect(currentTask.getByText("workflow/planning")).toHaveCount(0);
    await expect(currentTask.getByText("route")).toHaveCount(0);
    await expect(currentTask.getByText("phase")).toHaveCount(0);
  });

  test("keeps current task out of the top bar and shows it in Project Archive", async ({ page }) => {
    const sessionId = "top-level-mode-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "workflow_updated",
        session_id: sessionId,
        state: {
          session_id: sessionId,
          route: "workflow",
          phase: "clarifying",
          beginner_label: "raw",
          developer_label: "workflow/clarifying",
          matched_signals: ["idea"],
          reason: "用户正在描述一个新工具。",
          gate: "soft",
          override_actions: ["direct", "plan_first", "debug", "verify"],
          spec_path: null,
          plan_path: null,
          checkpoint_id: null,
          updated_at: Date.now(),
        },
      },
      {
        event_type: "forge_wiki_context_selected",
        session_id: sessionId,
        selected: [{
          page_id: "tasks",
          title: "当前任务",
          path: "tasks.md",
          kind: "tasks",
          summary: "正在收拢项目档案。",
          score: 0.9,
          reason: "这页项目记录与本轮请求相关",
          injected: true,
        }],
      },
    ], 5);

    await expect(page.getByTestId("workflow-status-pill")).toHaveCount(0);
    await page.getByTitle("打开项目档案").click();

    const workbench = page.locator("aside").last();
    await expect(workbench.getByText("项目档案", { exact: true }).first()).toBeVisible();
    const currentTask = workbench.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });
    await expect(currentTask.getByText("梳理想法", { exact: true })).toBeVisible();
    await expect(currentTask.getByText("已参考 1 条档案")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "当前任务" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "本轮参考" })).toBeVisible();
    const resources = await expandArchiveFiles(page);
    await expect(resources.getByText("文件名", { exact: true })).toBeVisible();
    await expect(resources.getByText("类型", { exact: true })).toBeVisible();
    await expect(resources.getByText("解析状态", { exact: true })).toBeVisible();
    await expect(resources.getByText("参考", { exact: true })).toBeVisible();
    await expect(workbench.getByTitle("刷新交付状态")).toBeVisible();
    const legacyProjectStatusLabel = ["项目", "状态"].join("");
    await expect(workbench.getByText(legacyProjectStatusLabel)).toHaveCount(0);
  });
});

test.describe("Turn context", () => {
  test("shows saved background and project records for the current turn", async ({ page }) => {
    const sessionId = "context-activation-session";
    const projectPath = "/Users/cabbos/project/forge";
    const selectedMemory = {
      memory_id: "memory-1",
      title: "中文优先",
      body: "用户偏好中文沟通，英文能力稍弱。",
      category: "preference" as const,
      scope: "user_profile" as const,
      score: 0.93,
      reason: "这是你固定的偏好",
      injected: true,
    };
    const selectedPage = {
      page_id: "tasks",
      title: "当前任务",
      path: "tasks.md",
      kind: "tasks" as const,
      summary: "当前正在收拢工作方式和本轮参考。",
      score: 0.91,
      reason: "这页项目记录与本轮请求相关",
      injected: true,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "memory_selection", session_id: sessionId, selected: [selectedMemory] },
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await page.getByTitle("打开项目档案").click();
    const activeContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });

    await expect(activeContext.getByText("已参考 2 条档案")).toBeVisible();
    await expect(activeContext.getByText("中文优先")).toBeVisible();
    await expect(activeContext.getByText("当前任务")).toBeVisible();
    await expect(activeContext.getByText("偏好", { exact: true })).toBeVisible();
    await expect(activeContext.getByText("项目记录 · tasks.md")).toBeVisible();
    await expect(activeContext.getByText("为什么参考")).toHaveCount(0);
    await expect(activeContext.getByText("本轮状态")).toHaveCount(0);
  });

  test("does not suggest saved background when user says not to remember", async ({ page }) => {
    const sessionId = "no-memory-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("不要记住这个，只是临时测试：以后默认用亮色主题。");
    await page.locator("textarea").press("Enter");
    await page.getByTitle("打开项目档案").click();
    await expandArchiveRecords(page);

    const inbox = projectArchive(page).locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });
    await expect(inbox.getByText("没有待确认的记录更新")).toBeVisible();
    await expect(inbox.getByText("以后默认用亮色主题")).not.toBeVisible();
  });
});
