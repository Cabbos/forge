import { test, expect, type Page } from "@playwright/test";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

/** Setup: inject mock IPC before the app loads */
async function setup(page: Page) {
  await page.addInitScript(() => {
    let callbackId = 0;
    const callbacks = new Map<number, (data: unknown) => void>();
    const workingDir = "/Users/cabbos/project/forge";
    const sessionWorkingDirs = new Map<string, string>();
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
          {
            // @ts-expect-error mock
            if (window.__mockCreateSessionError) throw new Error(String(window.__mockCreateSessionError));
            // @ts-expect-error mock
            const sessionId = window.__mockSessionId ?? crypto.randomUUID();
            sessionWorkingDirs.set(sessionId, String(args.workingDir ?? workingDir));
            // @ts-expect-error mock
            window.__lastCreateSessionArgs = args;
            // @ts-expect-error mock
            return {
              session_id: sessionId,
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              // @ts-expect-error mock
              missing_api_key: Boolean(window.__mockMissingApiKey),
            };
          }
        case "resume_session":
          {
            const sessionId = String(args.sessionId ?? "");
            // @ts-expect-error mock
            window.__lastResumedSessionId = sessionId;
            // @ts-expect-error mock
            const deliverySummary = window.__mockResumeDeliverySummary;
            if (deliverySummary) {
              window.setTimeout(() => {
                // @ts-expect-error listeners
                for (const listener of window.__tauriListeners?.["session-output"] ?? []) {
                  listener({
                    payload: {
                      event_type: "delivery_summary",
                      session_id: sessionId,
                      block_id: "resume-delivery-summary",
                      summary: deliverySummary,
                    },
                  });
                }
                // @ts-expect-error mock
                window.__resumeDeliveryEmitted = true;
              }, 0);
            }
            return {
              session_id: sessionId,
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              missing_api_key: false,
            };
          }
        case "send_input":
          // @ts-expect-error mock
          window.__lastSentText = args.text;
          return undefined;
        case "kill_session":
          // @ts-expect-error mock
          window.__lastKilledSessionId = args.sessionId;
          return undefined;
        case "delete_session":
          // @ts-expect-error mock
          window.__lastDeletedSessionId = args.sessionId;
          return undefined;
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
        case "search_workspace_files":
          return ["src/App.tsx", "src/components/session/InputBar.tsx", "README.md"]
            .filter((path) => path.toLowerCase().includes(String(args.query ?? "").toLowerCase()));
        case "toggle_capability":
          return undefined;
        case "get_api_key_status":
          return [{ provider: "deepseek", set: true, preview: "sk-e0...23ef" }];
        case "get_project_runtime_status":
          return {
            ...projectRuntimeStatus,
            working_dir: sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir,
          };
        case "get_project_checkpoint_status":
          return {
            ...projectCheckpointStatus,
            working_dir: sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir,
          };
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
  const trigger = records.getByRole("button", { name: /项目记录/ }).first();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await trigger.click();
  }
  return records;
}

async function expandArchiveFiles(page: Page) {
  const archive = projectArchive(page);
  const files = archive.getByTestId("archive-disclosure-files");
  const trigger = files.getByRole("button", { name: /资料/ }).first();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await trigger.click();
  }
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
    await expect(page.getByTestId("app-titlebar")).toHaveCSS("height", "44px");
    await expect(main.getByTestId("empty-workbench")).toBeVisible();
    await expect(main.getByTestId("empty-workbench").getByRole("button", { name: "开始新对话" })).toBeVisible();
    await expect(main.locator("img")).toHaveCount(0);
    await expect(main.locator("p", { hasText: "从当前对话开始" })).toHaveCount(0);
    await expect(main.getByText("Forge 会带着项目档案，把结果推进到可预览、可检查、可继续。")).toHaveCount(0);
    await expect(main.getByText("当前任务", { exact: true })).toHaveCount(0);
    await expect(main.getByText("交付", { exact: true })).toHaveCount(0);
    await expect(main.getByText("创建一个任务开始")).toHaveCount(0);
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

  test("resume does not duplicate persisted delivery summary blocks", async ({ page }) => {
    const sessionId = "legacy-delivery-resume";
    const projectPath = "/Users/cabbos/project/forge";
    const summary = {
      project_path: projectPath,
      preview_label: "预览未运行",
      checkpoint_label: "检查点已就绪",
      next_action: "下一步：交付状态可以继续验收。",
    };
    await setup(page);
    await page.addInitScript((summary) => {
      // @ts-expect-error mock
      window.__mockResumeDeliverySummary = summary;
    }, summary);
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
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "legacy-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: { summary },
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
    await page.getByRole("button", { name: "继续会话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error mock
      return window.__lastResumedSessionId === "legacy-delivery-resume";
    });
    await page.waitForFunction(() => {
      // @ts-expect-error mock
      return window.__resumeDeliveryEmitted === true;
    });

    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(1);
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

    const processSummary = page.getByTestId("tool-activity-summary");
    await expect(processSummary).toBeVisible({ timeout: 5000 });
    await expect(processSummary).toContainText("已处理 2 步");
    await processSummary.click();

    // Tool card should show write_to_file after expanding handled work.
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
    const processSummary = page.getByTestId("tool-activity-summary").first();
    await expect(thinkingTrigger).toBeVisible();
    await expect(processSummary).toBeVisible();
    await expect(processSummary).toHaveAttribute("aria-expanded", "false");

    const widths = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']")?.getBoundingClientRect();
      const process = document.querySelector("[data-testid='tool-activity-summary']")?.getBoundingClientRect();
      return thinking && process
        ? { thinking: Math.round(thinking.width), process: Math.round(process.width) }
        : null;
    });
    expect(widths).not.toBeNull();
    expect(widths!.thinking).toBeLessThanOrEqual(220);
    expect(widths!.process).toBeLessThanOrEqual(520);
    await expect(thinkingTrigger).toHaveCSS("border-top-width", "0px");

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
    await expect(pending).toHaveText(/正在组织回答/);
    await expect(pending).toHaveCSS("border-top-width", "0px");
  });

  test("conversation and composer share the same vertical gutters", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const gutters = await page.evaluate(() => {
      const scroll = document.querySelector("[data-testid='conversation-scroll']")?.getBoundingClientRect();
      const messageLane = document.querySelector("[data-testid='message-lane']")?.getBoundingClientRect();
      const composerFrameNode = document.querySelector("[data-testid='composer-frame']");
      const composerFrame = composerFrameNode?.getBoundingClientRect();
      const composerLane = document.querySelector("[data-testid='composer-lane']")?.getBoundingClientRect();
      if (!scroll || !messageLane || !composerFrameNode || !composerFrame || !composerLane) return null;
      const composerBorderTop = Number.parseFloat(getComputedStyle(composerFrameNode).borderTopWidth);

      return {
        transcriptTop: Math.round(messageLane.top - scroll.top),
        composerTop: Math.round(composerLane.top - composerFrame.top - composerBorderTop),
      };
    });

    expect(gutters).not.toBeNull();
    expect(gutters!.transcriptTop).toBe(16);
    expect(gutters!.composerTop).toBe(16);
    expect(Math.abs(gutters!.transcriptTop - gutters!.composerTop)).toBeLessThanOrEqual(1);
  });

  test("conversation shell uses one vertical rhythm token", async ({ page }) => {
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
    expect(rhythm!.token).toBe("16px");
    expect(rhythm!.scrollTop).toBe(16);
    expect(rhythm!.scrollBottom).toBe(16);
    expect(rhythm!.composerTop).toBe(16);
    expect(rhythm!.composerBottom).toBe(16);
    expect(rhythm!.scrollButtonBottom).toBe(16);
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
      };
    });

    expect(rhythm).not.toBeNull();
    expect(rhythm!.innerX).toBe("16px");
    expect(rhythm!.innerY).toBe("12px");
    expect(rhythm!.textPadLeft).toBe(16);
    expect(rhythm!.textPadTop).toBe(12);
    expect(rhythm!.toolbarPadLeft).toBe(16);
    expect(rhythm!.toolbarPadBottom).toBe(10);
    expect(rhythm!.textareaLineHeight).toBe(22);
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
        menuShadow: menuStyle.boxShadow,
        optionHeight: Math.round(option.getBoundingClientRect().height),
        radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.gapToken).toBe("8px");
    expect(metrics!.menuBottomGap).toBe(8);
    expect(metrics!.menuShadow).not.toContain("0px 25px");
    expect(metrics!.optionHeight).toBe(28);
    expect(metrics!.radius).toBeLessThanOrEqual(8);
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
    expect(metrics!.assistantLineToken).toBe("26px");
    expect(metrics!.userLineToken).toBe("22px");
    expect(metrics!.assistantLineHeight).toBe(26);
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
      const action = document.querySelector("[data-testid='message-copy-action']");
      if (!action) return null;
      const style = getComputedStyle(action);
      return {
        width: Math.round(action.getBoundingClientRect().width),
        height: Math.round(action.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        position: style.position,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.width).toBe(24);
    expect(metrics!.height).toBe(24);
    expect(metrics!.radius).toBeLessThanOrEqual(8);
    expect(metrics!.position).toBe("absolute");
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
      const paragraph = assistant.querySelector("p");
      const heading = assistant.querySelector("h2");
      const list = assistant.querySelector("ul");
      const listItem = assistant.querySelector("li");
      const quote = assistant.querySelector("blockquote");
      const inlineCode = assistant.querySelector("p code");
      const table = assistant.querySelector("table");
      const tableCell = assistant.querySelector("td");
      if (!paragraph || !heading || !list || !listItem || !quote || !inlineCode || !table || !tableCell) return null;
      const paragraphStyle = getComputedStyle(paragraph);
      const headingStyle = getComputedStyle(heading);
      const listStyle = getComputedStyle(list);
      const listItemStyle = getComputedStyle(listItem);
      const quoteStyle = getComputedStyle(quote);
      const codeStyle = getComputedStyle(inlineCode);
      const tableStyle = getComputedStyle(table);
      const cellStyle = getComputedStyle(tableCell);

      return {
        paragraphGapToken: getComputedStyle(root).getPropertyValue("--forge-markdown-paragraph-gap").trim(),
        blockGapToken: getComputedStyle(root).getPropertyValue("--forge-markdown-block-gap").trim(),
        paragraphMarginBottom: Math.round(Number.parseFloat(paragraphStyle.marginBottom)),
        headingFontSize: Math.round(Number.parseFloat(headingStyle.fontSize)),
        headingLineHeight: Math.round(Number.parseFloat(headingStyle.lineHeight)),
        headingMarginTop: Math.round(Number.parseFloat(headingStyle.marginTop)),
        listPaddingLeft: Math.round(Number.parseFloat(listStyle.paddingLeft)),
        listItemMarginBottom: Math.round(Number.parseFloat(listItemStyle.marginBottom)),
        quoteBorderWidth: Math.round(Number.parseFloat(quoteStyle.borderLeftWidth)),
        quotePaddingLeft: Math.round(Number.parseFloat(quoteStyle.paddingLeft)),
        codeBackground: codeStyle.backgroundColor,
        codePaddingLeft: Math.round(Number.parseFloat(codeStyle.paddingLeft)),
        tableDisplay: tableStyle.display,
        tableMarginTop: Math.round(Number.parseFloat(tableStyle.marginTop)),
        cellPaddingTop: Math.round(Number.parseFloat(cellStyle.paddingTop)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.paragraphGapToken).toBe("10px");
    expect(metrics!.blockGapToken).toBe("12px");
    expect(metrics!.paragraphMarginBottom).toBe(10);
    expect(metrics!.headingFontSize).toBe(15);
    expect(metrics!.headingLineHeight).toBe(24);
    expect(metrics!.headingMarginTop).toBe(16);
    expect(metrics!.listPaddingLeft).toBe(20);
    expect(metrics!.listItemMarginBottom).toBe(3);
    expect(metrics!.quoteBorderWidth).toBe(2);
    expect(metrics!.quotePaddingLeft).toBe(12);
    expect(metrics!.codeBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.codePaddingLeft).toBeGreaterThanOrEqual(4);
    expect(metrics!.tableDisplay).toBe("block");
    expect(metrics!.tableMarginTop).toBe(12);
    expect(metrics!.cellPaddingTop).toBe(6);
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
    expect(metrics!.marginTop).toBe(10);
    expect(metrics!.marginBottom).toBe(10);
    expect(metrics!.headerHeight).toBe(32);
    expect(metrics!.headerBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.labelFontSize).toBe(10);
    expect(metrics!.codeLineHeight).toBe(20);
    expect(metrics!.codeFontSize).toBeCloseTo(12.5);
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
    await expect(assistant.locator(".code-surface")).toBeVisible();

    const streamingMetrics = await page.evaluate(() => {
      const assistant = document.querySelector("[data-testid='assistant-message']");
      if (!assistant) return null;
      const heading = assistant.querySelector("h2");
      const listItem = assistant.querySelector("li");
      const codeSurface = assistant.querySelector(".code-surface");
      const plaintextWrapper = assistant.querySelector(".whitespace-pre-wrap");
      return {
        hasHeading: Boolean(heading),
        hasListItem: Boolean(listItem),
        hasCodeSurface: Boolean(codeSurface),
        hasPlaintextWrapper: Boolean(plaintextWrapper),
      };
    });

    expect(streamingMetrics).toEqual({
      hasHeading: true,
      hasListItem: true,
      hasCodeSurface: true,
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

  test("message stream uses one gap token without component margins", async ({ page }) => {
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
      const blocks = [...document.querySelectorAll("[data-testid='message-block']")];
      if (!lane || blocks.length < 2) return null;
      const laneStyle = getComputedStyle(lane);
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-message-gap").trim(),
        gap: Math.round(Number.parseFloat(laneStyle.rowGap)),
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
    expect(layout!.token).toBe("12px");
    expect(layout!.gap).toBe(12);
    expect(layout!.margins.every((margin) => margin.top === 0 && margin.bottom === 0)).toBeTruthy();
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
      return {
        rowGap: Math.round(Number.parseFloat(firstStyle.rowGap)),
        secondPaddingTop: Math.round(Number.parseFloat(secondStyle.paddingTop)),
        firstBackground: firstStyle.backgroundColor,
        firstBorderTop: Math.round(Number.parseFloat(firstStyle.borderTopWidth)),
        firstRadius: Number.parseFloat(firstStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.rowGap).toBe(12);
    expect(metrics!.secondPaddingTop).toBeGreaterThanOrEqual(8);
    expect(metrics!.firstBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.firstBorderTop).toBe(0);
    expect(metrics!.firstRadius).toBe(0);
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
      const tool = document.querySelector("[data-testid='tool-card-trigger']");
      const shell = document.querySelector("[data-testid='shell-card-trigger']");
      if (!tool || !shell) return null;
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-log-row-height").trim(),
        toolHeight: Math.round(tool.getBoundingClientRect().height),
        shellHeight: Math.round(shell.getBoundingClientRect().height),
        toolMargin: getComputedStyle(tool.parentElement as Element).marginBottom,
        shellMargin: getComputedStyle(shell.parentElement as Element).marginBottom,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.token).toBe("30px");
    expect(metrics!.toolHeight).toBe(30);
    expect(metrics!.shellHeight).toBe(30);
    expect(metrics!.toolMargin).toBe("0px");
    expect(metrics!.shellMargin).toBe("0px");
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
      { event_type: "shell_output", session_id: sessionId, block_id: "detail-shell", content: "done" },
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
        return {
          maxHeightToken: getComputedStyle(root).getPropertyValue("--forge-log-output-max-height").trim(),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          headerHeight: header ? Math.round(header.getBoundingClientRect().height) : 0,
          outputMaxHeight: output ? getComputedStyle(output).maxHeight : "",
        };
      });
    });

    expect(surfaces).toHaveLength(2);
    expect(surfaces.every((surface) => surface.maxHeightToken === "260px")).toBeTruthy();
    expect(surfaces.every((surface) => surface.radius <= 8)).toBeTruthy();
    expect(surfaces.every((surface) => surface.headerHeight === 36)).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputMaxHeight === "260px")).toBeTruthy();
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
      const wrapper = trigger.parentElement;
      const wrapperStyle = wrapper ? getComputedStyle(wrapper) : null;
      return {
        height: Math.round(trigger.getBoundingClientRect().height),
        marginTop: wrapperStyle ? Math.round(Number.parseFloat(wrapperStyle.marginTop)) : -1,
        marginBottom: wrapperStyle ? Math.round(Number.parseFloat(wrapperStyle.marginBottom)) : -1,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.height).toBe(30);
    expect(metrics!.marginTop).toBe(0);
    expect(metrics!.marginBottom).toBe(0);
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

  test("design system materials stay subtle and token-driven", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const titlebar = document.querySelector("[data-testid='app-titlebar']");
      const sidebar = document.querySelector("aside");
      const composer = document.querySelector("[data-testid='composer-surface']");
      if (!titlebar || !sidebar || !composer) return null;
      const rootStyle = getComputedStyle(root);
      const titlebarStyle = getComputedStyle(titlebar);
      const sidebarStyle = getComputedStyle(sidebar);
      const composerStyle = getComputedStyle(composer);
      return {
        borderSubtle: rootStyle.getPropertyValue("--forge-border-subtle").trim(),
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
    expect(metrics!.borderSubtle).toBe("rgba(148, 163, 184, 0.14)");
    expect(metrics!.bgRaised).toBe("rgba(255, 255, 255, 0.018)");
    expect(metrics!.hover).toBe("rgba(255, 255, 255, 0.036)");
    expect(metrics!.focusRing).toBe("rgba(212, 168, 83, 0.42)");
    expect(metrics!.titlebarBorder).toBe("rgba(148, 163, 184, 0.14)");
    expect(metrics!.sidebarBorder).toBe("rgba(148, 163, 184, 0.14)");
    expect(metrics!.composerBorder).toBe("rgba(148, 163, 184, 0.14)");
    expect(metrics!.composerBg).toBe("rgba(255, 255, 255, 0.02)");
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
          risk: "caution",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: null,
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
  });

  test("failed delivery check offers continue repair prompt", async ({ page }) => {
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
        block_id: "failed-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：先修复检查未通过的问题。",
          verification_label: "检查未通过",
          verification_status: "failed",
          verification_command: "npm run build",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "本轮交付" });
    await expect(card.getByText("检查未通过", { exact: true })).toBeVisible();
    await expect(card).toHaveCSS("border-color", "rgba(212, 119, 119, 0.3)");
    await card.getByRole("button", { name: "继续修复" }).click();

    await expect(page.locator("textarea")).toHaveValue(/npm run build/);
    await expect(page.locator("textarea")).toHaveValue(/继续修复/);
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
    await expect(diff.getByRole("button", { name: "展开完整改动" })).toBeVisible();
    await expect(diff.getByText("line34")).toHaveCount(0);

    const metrics = await diff.evaluate((node) => {
      const added = node.querySelector("[data-testid='diff-line-added']");
      const removed = node.querySelector("[data-testid='diff-line-removed']");
      const hunk = node.querySelector("[data-testid='diff-line-hunk']");
      const oldNo = node.querySelector("[data-testid='diff-line-old-number']");
      const newNo = node.querySelector("[data-testid='diff-line-new-number']");
      if (!added || !removed || !hunk || !oldNo || !newNo) return null;
      const addedStyle = getComputedStyle(added);
      const removedStyle = getComputedStyle(removed);
      const hunkStyle = getComputedStyle(hunk);
      return {
        grid: getComputedStyle(added).display,
        oldNumberWidth: Math.round(oldNo.getBoundingClientRect().width),
        newNumberWidth: Math.round(newNo.getBoundingClientRect().width),
        addedBackground: addedStyle.backgroundColor,
        removedBackground: removedStyle.backgroundColor,
        hunkBorderTop: hunkStyle.borderTopWidth,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.grid).toBe("grid");
    expect(metrics!.oldNumberWidth).toBeGreaterThanOrEqual(36);
    expect(metrics!.newNumberWidth).toBeGreaterThanOrEqual(36);
    expect(metrics!.addedBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removedBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.hunkBorderTop).toBe("1px");

    await diff.getByRole("button", { name: "展开完整改动" }).click();
    await expect(diff.getByText("line34")).toBeVisible();
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
    await expect(summary).toContainText("已处理 3 步");
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
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");

    await summary.click();
    await expect(summary).toHaveAttribute("aria-expanded", "true");
    await expect(group.getByText("已读取文件")).toBeVisible();
    await expect(group.getByText("npm run build")).toBeVisible();
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
    await expect(userMessage.locator(".forge-file-ref")).toContainText("src/App.tsx:12");

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

    await page.locator("textarea").fill("继续优化等待状态");
    await page.locator("textarea").press("Enter");

    const pending = page.getByTestId("pending-block");
    await expect(pending).toHaveText(/正在组织回答/);
    await expect(pending.getByTestId("pending-dots")).toBeVisible();
    await expect(pending).toHaveCSS("border-top-width", "0px");
    const pendingMetrics = await pending.evaluate((node) => ({
      height: Math.round(node.getBoundingClientRect().height),
      color: getComputedStyle(node).color,
    }));

    await simulateStream(page, sessionId, [
      { event_type: "thinking_start", session_id: sessionId, block_id: "quiet-thinking" },
      { event_type: "thinking_chunk", session_id: sessionId, block_id: "quiet-thinking", content: "Need to inspect the failure before editing." },
    ], 1);

    const thinking = page.getByTestId("thinking-trigger");
    await expect(thinking).toHaveText(/正在梳理思路/);
    await expect(thinking.getByTestId("thinking-dots")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']");
      if (!thinking) return null;
      return {
        thinkingHeight: Math.round(thinking.getBoundingClientRect().height),
        thinkingColor: getComputedStyle(thinking).color,
      };
    });

    expect(metrics).not.toBeNull();
    expect(pendingMetrics.height).toBeLessThanOrEqual(30);
    expect(metrics!.thinkingHeight).toBeLessThanOrEqual(30);
    expect(pendingMetrics.color).toBe(metrics!.thinkingColor);
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
    await expect(sidebar.getByRole("button", { name: "设置" })).toBeVisible();
    await expect(sidebar.getByText("当前工作空间", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("插件", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("自动化", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("设置", { exact: true })).toHaveCount(0);
    const utilityMetrics = await sidebar.locator("[data-testid='sidebar-utility-nav']").evaluate((node) => {
      const rect = node.getBoundingClientRect();
      const buttons = Array.from(node.querySelectorAll("button")).map((button) => {
        const item = button.getBoundingClientRect();
        return Math.round(item.width);
      });
      return { height: Math.round(rect.height), buttons };
    });
    expect(utilityMetrics.height).toBeLessThanOrEqual(40);
    expect(utilityMetrics.buttons).toEqual([28, 28, 28]);

    await sidebar.getByRole("button", { name: "插件" }).click();
    const drawer = page.getByRole("complementary", { name: "插件" });
    await expect(drawer.getByText("插件", { exact: true }).first()).toBeVisible();
    await expect(drawer.getByRole("tab", { name: /插件/ })).toHaveAttribute("aria-selected", "true");
    await expect(drawer.getByRole("textbox", { name: "搜索插件" })).toBeVisible();
    await expect(drawer.getByTestId("capability-drawer-header")).toHaveCSS("height", "44px");
    await page.waitForTimeout(300);
    const drawerX = Math.round((await drawer.boundingBox())?.x ?? 0);
    const drawerWidth = Math.round((await drawer.boundingBox())?.width ?? 0);
    expect(drawerX).toBe(Math.round(width));
    expect(drawerWidth).toBe(320);
    await expect(drawer.getByText(/[☖⎔◈●]/)).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "插件" })).toHaveCount(0);
  });

  test("sidebar history rows stay compact and scannable", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.locator("textarea").fill("Build a compact scanner");
    await page.locator("textarea").press("Enter");

    const sidebar = page.locator("aside").first();
    const row = sidebar.getByRole("button", { name: "Build a compact scanner", exact: true });
    await expect(row).toBeVisible();

    const metrics = await row.evaluate((node) => {
      const root = document.documentElement;
      const style = getComputedStyle(node);
      const deleteButton = node.querySelector("button");
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-sidebar-row-height").trim(),
        height: Math.round(node.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        deleteOpacity: deleteButton ? getComputedStyle(deleteButton).opacity : null,
      };
    });

    expect(metrics.token).toBe("28px");
    expect(metrics.height).toBe(28);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.deleteOpacity).toBe("0");
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
    expect(metrics!.optionHeight).toBe(28);
    expect(metrics!.shadow).not.toContain("0px 25px");
    expect(metrics!.radius).toBeLessThanOrEqual(8);
  });

  test("project archive opens from the keyboard as a quiet inspector", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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
    await expect(archive.getByText("Project Status")).toHaveCount(0);
    await expect(archive.getByText("Context Activation")).toHaveCount(0);

    const metrics = await archive.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        width: Math.round(node.getBoundingClientRect().width),
        bg: style.backgroundColor,
      };
    });
    expect(metrics.width).toBeLessThanOrEqual(320);
    expect(metrics.bg).not.toBe("rgba(0, 0, 0, 0)");

    await page.keyboard.press("Escape");
    await expect(archive).toHaveCount(0);
  });

  test("project archive disclosure rows use inspector rhythm tokens", async ({ page }) => {
    const sessionId = crypto.randomUUID();
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
      const archive = document.querySelector("[data-testid='project-archive-panel']");
      const body = document.querySelector("[data-testid='project-archive-body']");
      const disclosure = document.querySelector("[data-testid='archive-disclosure-records'] button");
      if (!archive || !body || !disclosure) return null;
      return {
        widthToken: getComputedStyle(root).getPropertyValue("--forge-inspector-width").trim(),
        gapToken: getComputedStyle(root).getPropertyValue("--forge-inspector-gap").trim(),
        rowToken: getComputedStyle(root).getPropertyValue("--forge-disclosure-row-height").trim(),
        width: Math.round(archive.getBoundingClientRect().width),
        bodyGap: Math.round(Number.parseFloat(getComputedStyle(body).rowGap)),
        rowHeight: Math.round(disclosure.getBoundingClientRect().height),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.widthToken).toBe("300px");
    expect(metrics!.gapToken).toBe("10px");
    expect(metrics!.rowToken).toBe("28px");
    expect(metrics!.width).toBe(300);
    expect(metrics!.bodyGap).toBe(10);
    expect(metrics!.rowHeight).toBe(28);
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
    const workspaceMenu = page.getByRole("menu", { name: "项目文件夹" });
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

  test("manual workspace path rejects broad user directory", async ({ page }) => {
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
    for (const path of ["/Users", "/Users/cabbos", "/home"]) {
      await pathInput.fill(path);
      await page.getByRole("button", { name: "添加" }).click();
      await expect(page.getByText("请选择具体项目文件夹，不要直接使用用户主目录。")).toBeVisible();
    }

    await expect(sidebar.getByRole("button", { name: /^Users$/ })).toHaveCount(0);
    await expect(sidebar.getByRole("button", { name: /^home$/ })).toHaveCount(0);
    await expect(sidebar.getByRole("button", { name: /cabbos/ })).toHaveCount(0);
  });

  test("create session broad project failure is visible and manual project selection can recover", async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockCreateSessionError = "请选择具体项目文件夹，不要直接使用用户主目录。";
    });
    await page.goto("http://localhost:1420");

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(sidebar.getByRole("status")).toContainText("请选择具体项目文件夹，不要直接使用用户主目录。");

    await sidebar.getByRole("button", { name: /forge/ }).click();
    await page.getByRole("menuitem", { name: "手动输入路径" }).click();
    await page.getByLabel("项目文件夹路径").fill("/Users/cabbos/project/recovered-app");
    await page.getByRole("button", { name: "添加" }).click();
    await expect(sidebar.getByRole("button", { name: /recovered-app/ })).toBeVisible();

    await page.evaluate(() => {
      // @ts-expect-error mock
      window.__mockCreateSessionError = "";
    });
    await sidebar.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(page.locator("textarea")).toBeVisible();
    await expect(sidebar.getByRole("status")).toHaveCount(0);
  });

  test("create session inaccessible project failure is visible", async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockCreateSessionError = "无法打开项目文件夹：No such file or directory";
    });
    await page.goto("http://localhost:1420");

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(sidebar.getByRole("status")).toContainText("这个项目文件夹打不开。请重新选择一个具体项目文件夹。");
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
    await expect(page.getByText("发送失败")).toBeVisible();
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
    expect(metrics!.token).toBe("140px");
    expect(metrics!.height).toBeLessThanOrEqual(140);
    expect(metrics!.maxHeight).toBe(140);
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
    const sentText = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSentText;
    });
    expect(sentText).toBeUndefined();
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
    await expect(firstVersion.getByText("番茄钟小工具").first()).toBeVisible();
    await expect(firstVersion.getByText("开始、暂停、重置").first()).toBeVisible();
    await expect(firstVersion.getByText("下一步", { exact: true }).first()).toBeVisible();
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
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-v0-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

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

  test("start readiness surfaces missing provider setup before the first prompt", async ({ page }) => {
    const sessionId = "first-loop-missing-provider";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge-test-app");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "get_api_key_status") return [{ provider: "deepseek", set: false, preview: "" }];
        return original?.(cmd, args);
      };
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const readiness = page.getByTestId("start-readiness");
    await expect(readiness).toBeVisible();
    await expect(readiness.getByText("需要配置模型密钥")).toBeVisible();
    await expect(readiness.getByText("还没有配置 DeepSeek")).toBeVisible();
    await expect(readiness.getByText("forge-test-app")).toBeVisible();
    await expect(readiness.getByText("/Users/cabbos/project/forge-test-app")).toHaveCount(0);
    await expect(readiness.getByText("工作空间")).toHaveCount(0);
    await expect(readiness.getByText("检查点")).toHaveCount(0);

    await readiness.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "模型服务" })).toBeVisible();
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
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-progress-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

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
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-actions-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

    await expect(page.getByText("验收提示", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "检查风险" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "开始验收" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "继续优化" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "检查这版" })).toBeVisible();

    await page.getByRole("button", { name: "检查这版" }).click();
    await expect(page.locator("textarea")).toHaveValue(/检查当前版本有没有明显问题/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
  });

  test("first loop binds to the active test app without exposing the full path", async ({ page }) => {
    const sessionId = "first-loop-test-app";
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    await setup(page);
    await page.addInitScript(({ sessionId, sandboxPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", sandboxPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, sandboxPath });

    await page.goto("http://localhost:1420");
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-test-app-delivery",
        summary: {
          project_path: sandboxPath,
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    const sentText = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSentText;
    });
    expect(createArgs.workingDir).toBe(sandboxPath);
    expect(sentText).toContain("Forge 第一闭环提示");
    expect(sentText).toContain("可见、可点、可继续");
    expect(sentText).not.toContain("目标项目：");

    const main = page.getByRole("main");
    const delivery = main.locator("div").filter({ hasText: "本轮交付" }).filter({ hasText: "预览未运行" }).last();
    await expect(delivery).toBeVisible();
    await expect(delivery.getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(delivery.getByText(sandboxPath, { exact: true })).toHaveCount(0);
  });

  test("demo ledger first loop reaches repair, delivery, and project archive", async ({ page }) => {
    const sessionId = "demo-ledger-first-loop";
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    const request = "请为收支记录工具做第一版：支持新增收入或支出、展示明细列表，并在页面顶部汇总当前结余。";
    const proposal = {
      id: "demo-ledger-record-proposal",
      project_path: sandboxPath,
      session_id: sessionId,
      target_pages: ["tasks.md", "log.md"],
      title: "记录收支小工具第一版",
      summary: "补充收支记录第一版、检查结果和下一步验收事项。",
      patch_preview: "追加本轮第一版验收记录。",
      status: "pending" as const,
      created_at: "2026-05-17T00:00:00.000Z",
    };

    await setup(page);
    await page.addInitScript(({ sessionId, sandboxPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", sandboxPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "confirm_response") {
          // @ts-expect-error mock
          window.__lastConfirmResponseArgs = args;
          return undefined;
        }
        return original?.(cmd, args);
      };
    }, { sessionId, sandboxPath });

    await page.goto("http://localhost:1420");
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(page.getByLabel("当前项目边界").getByText(sandboxPath, { exact: true })).toHaveCount(0);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill(request);
    await page.locator("textarea").press("Enter");
    await expect(page.getByRole("main").getByText(request, { exact: true }).last()).toBeVisible();

    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    expect(createArgs.workingDir).toBe(sandboxPath);

    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "demo-ledger-progress" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "demo-ledger-progress",
        content: "我先把收支记录的最小闭环接起来，再跑一次构建检查。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "demo-ledger-progress" },
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "demo-ledger-confirm",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          title: "准备修改项目",
          workspace_name: "forge-test-app",
          workspace_path: sandboxPath,
          operation: "write_file",
          affected_files: ["src/App.tsx", "src/App.css"],
          impact: "将修改 2 个文件",
          risk: "caution",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: null,
        },
      },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "demo-ledger-read", tool_name: "read_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "demo-ledger-read", result: "找到现有入口。", is_error: false, duration_ms: 24 },
      { event_type: "shell_start", session_id: sessionId, block_id: "demo-ledger-failed-build", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "demo-ledger-failed-build", content: "src/App.tsx: 收支金额字段类型需要修复\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "demo-ledger-failed-build", exit_code: 1 },
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "demo-ledger-failed-delivery",
        summary: {
          project_path: sandboxPath,
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：先修复构建检查未通过的问题。",
          verification_label: "检查未通过",
          verification_status: "failed",
          verification_command: "npm run build",
        },
      },
    ], 1);

    const confirmCard = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmCard.getByText("forge-test-app")).toBeVisible();
    await expect(confirmCard).not.toContainText(sandboxPath);
    await expect(confirmCard).not.toContainText("/Users/");
    await expect(confirmCard.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(confirmCard.getByText(/ConfirmAsk|permission/i)).toHaveCount(0);
    await expect(confirmCard.getByText("forge", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("main")).not.toContainText(sandboxPath);
    await confirmCard.getByRole("button", { name: "继续" }).click();
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastConfirmResponseArgs;
    });
    expect(confirmArgs).toEqual({ blockId: "demo-ledger-confirm", approved: true });

    const failedDelivery = page.getByTestId("message-panel").filter({ hasText: "本轮交付" }).filter({ hasText: "检查未通过" });
    await expect(failedDelivery.getByText("forge-test-app", { exact: true })).toBeVisible();
    await failedDelivery.getByRole("button", { name: "继续修复" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续修复/);
    await expect(page.locator("textarea")).toHaveValue(/npm run build/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);

    await page.locator("textarea").press("Enter");
    const repairPrompt = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSentText;
    });
    expect(repairPrompt).toContain("继续修复");
    expect(repairPrompt).toContain("npm run build");
    expect(repairPrompt).not.toContain("目标项目：");

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "demo-ledger-repair-progress" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "demo-ledger-repair-progress",
        content: "金额字段已经收窄，收支合计可以继续验收。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "demo-ledger-repair-progress" },
      { event_type: "shell_start", session_id: sessionId, block_id: "demo-ledger-success-build", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "demo-ledger-success-build", content: "✓ built in 640ms\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "demo-ledger-success-build", exit_code: 0 },
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "demo-ledger-success-delivery",
        summary: {
          project_path: sandboxPath,
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：验收添加收支和合计展示。",
          verification_label: "检查通过",
          verification_status: "passed",
          verification_command: "npm run build",
          record_label: "建议更新项目记录",
          record_status: "pending",
          record_target_pages: ["tasks.md", "log.md"],
        },
      },
    ], 1);

    const successfulDelivery = page.getByTestId("message-panel").filter({ hasText: "本轮交付" }).filter({ hasText: "检查通过" });
    await expect(successfulDelivery.getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(successfulDelivery.getByText("预览未运行")).toBeVisible();
    await expect(successfulDelivery.getByText("检查点已就绪")).toBeVisible();
    await expect(successfulDelivery.getByText("检查通过", { exact: true })).toBeVisible();
    await expect(successfulDelivery.getByText("自动记录")).toBeVisible();
    await expect(page.getByRole("main").getByText(sandboxPath, { exact: true })).toHaveCount(0);
    await expect(page.getByRole("main").getByText(/Workflow Router|Task Mode|Living Wiki|Forge Wiki|writeback|ConfirmAsk|permission/i)).toHaveCount(0);
    await expect(page.getByRole("main").getByText(/示例|玩具|临时/)).toHaveCount(0);

    await successfulDelivery.getByRole("button", { name: "查看记录" }).click();

    const archive = projectArchive(page);
    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("forge-test-app", { exact: true }).first()).toBeVisible();
    await expect(archive.getByText(sandboxPath, { exact: true })).toHaveCount(0);

    const records = await expandArchiveRecords(page);
    await expect(records.getByRole("heading", { name: "建议更新记录" })).toBeVisible();
    await expect(records.getByText(proposal.summary)).toBeVisible();
    await expect(records.getByText("保存位置")).toBeVisible();
    await expect(records.getByText("项目记录页面")).toBeVisible();
    await expect(records.getByText("tasks.md, log.md")).toBeVisible();
    await expect(records.getByRole("button", { name: "接受" })).toBeVisible();
    await expect(records.getByRole("button", { name: "丢弃" })).toBeVisible();
    await expect(records.getByText(/Workflow Router|Task Mode|Living Wiki|Forge Wiki|writeback/)).toHaveCount(0);
  });

  test("demo workspace resume returns to project overview without path leakage", async ({ page }) => {
    const sessionId = "demo-ledger-return-session";
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    const summary = {
      project_path: sandboxPath,
      preview_label: "预览未运行",
      checkpoint_label: "检查点已就绪",
      next_action: "下一步：验收添加收支和合计展示。",
      verification_label: "检查通过",
      verification_status: "passed",
      verification_command: "npm run build",
      record_label: "建议更新项目记录",
      record_status: "pending",
      record_target_pages: ["tasks.md", "log.md"],
    };

    await setup(page);
    await page.addInitScript((sandboxPath) => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "get_project_runtime_status") {
          return {
            working_dir: sandboxPath,
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
        }
        if (cmd === "get_project_checkpoint_status") {
          return {
            working_dir: sandboxPath,
            is_git_repo: true,
            dirty: false,
            last_checkpoint: null,
            message: "No checkpoint yet",
          };
        }
        return original?.(cmd, args);
      };
    }, sandboxPath);

    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, sandboxPath, summary }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", sandboxPath);

      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: sandboxPath, name: "forge-test-app", path: sandboxPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(sandboxPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: sandboxPath,
          workspaceId: sandboxPath,
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
          deliverySummary: summary,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "demo-return-user-message",
          event_type: "user_message",
          content: "请为收支记录工具做第一版：支持新增收入或支出、展示明细列表，并在页面顶部汇总当前结余。",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "demo-return-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: { summary },
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, sandboxPath, summary });

    await page.reload();
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(page.getByLabel("当前项目边界").getByText(sandboxPath, { exact: true })).toHaveCount(0);

    await page.getByTitle("打开项目档案").click();

    const archive = projectArchive(page);
    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("forge-test-app", { exact: true }).first()).toBeVisible();
    await expect(archive.getByText("收支记录工具")).toBeVisible();
    await expect(archive.getByText("预览未运行 · 检查点已就绪")).toBeVisible();
    await expect(archive.getByText("下一步：验收添加收支和合计展示。")).toBeVisible();
    await expect(archive.getByText(sandboxPath, { exact: true })).toHaveCount(0);

    await archive.getByRole("button", { name: "继续上次任务" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续上次任务/);
    await expect(page.locator("textarea")).toHaveValue(/收支记录工具/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
    await expect(page.locator("textarea")).not.toHaveValue(new RegExp(sandboxPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
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
