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

test.describe("Timeline Message Flow", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("app loads and shows empty state", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(main.locator("p", { hasText: "从当前任务开始" })).toBeVisible();
    await expect(main.getByText("Forge 会带着项目档案，把结果推进到可预览、可检查、可继续。")).toBeVisible();
    await expect(main.getByText("当前任务", { exact: true })).toBeVisible();
    await expect(main.locator("span").filter({ hasText: "项目档案" })).toBeVisible();
    await expect(main.getByText("交付", { exact: true })).toBeVisible();
    await expect(main.getByText("创建一个任务开始")).toHaveCount(0);
  });

  test("creating a session shows chat input", async ({ page }) => {
    await page.goto("http://localhost:1420");
    // Click new session button
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    // Input should appear
    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByRole("button", { name: "我想做一个番茄钟小工具，可以开始、暂停、重置。" })).toBeVisible();
    await expect(page.getByRole("button", { name: "我想做一个记账小工具，先能记录一笔收入或支出。" })).toBeVisible();
    await expect(page.getByRole("button", { name: "我想做一个文案小工具，输入主题后生成一版短文案。" })).toBeVisible();
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
    await expect(page.getByRole("heading", { name: "模型密钥" })).toBeVisible();
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

    const card = page.getByText("准备修改项目").locator("xpath=ancestor::div[contains(@class,'mb-3')][1]");
    await expect(card.getByText("准备修改项目")).toBeVisible();
    await expect(card.getByText("工作空间", { exact: true })).toBeVisible();
    await expect(card.getByText("forge")).toBeVisible();
    await expect(card.getByText("/Users/cabbos/project/forge")).toBeVisible();
    await expect(card.getByText("写入文件")).toBeVisible();
    await expect(card.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "继续" })).toBeVisible();
    await expect(card.getByRole("button", { name: "取消" })).toBeVisible();
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
    await expect(page.getByText("进行中")).toBeVisible();

    // Send tool_result (done state)
    await simulateStream(page, sessionId, [
      { event_type: "tool_call_result", session_id: sessionId, block_id: toolId, result: "fn main() {}", is_error: false, duration_ms: 100 },
    ], 30);

    // Should show done
    await expect(page.getByRole("button", { name: /已读取文件/ })).toBeVisible({ timeout: 3000 });
    await expect(page.getByText("完成")).toBeVisible();
  });

  test("sidebar shows persistent navigation", async ({ page }) => {
    const sidebar = page.locator("aside").first();

    const width = (await sidebar.boundingBox())?.width ?? 0;
    expect(width).toBeGreaterThanOrEqual(220);
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "插件" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "自动化" })).toBeVisible();
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
    await expect(sidebar.getByText("Build A timer")).toBeVisible();
    await expect(sidebar.getByText("Build B dashboard")).toHaveCount(0);

    await sidebar.getByRole("button", { name: /app-one/ }).click();
    await page.getByRole("button", { name: /app-two/ }).click();

    await expect(sidebar.getByRole("button", { name: /app-two/ })).toBeVisible();
    await expect(sidebar.getByText("Build B dashboard")).toBeVisible();
    await expect(sidebar.getByText("Build A timer")).toHaveCount(0);
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
    await sidebar.getByRole("button", { name: /选择一个项目开始/ }).click();
    await page.getByRole("button", { name: "选择文件夹" }).click();

    await expect(sidebar.getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeEnabled();
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
    await sidebar.getByRole("button", { name: /选择一个项目开始/ }).click();
    await page.getByRole("button", { name: "手动输入路径" }).click();

    const pathInput = page.getByLabel("项目文件夹路径");
    await expect(pathInput).toBeVisible();
    await pathInput.fill("/Users/cabbos/project/demo-tool");
    await page.getByRole("button", { name: "添加" }).click();

    await expect(sidebar.getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeEnabled();
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

    await expect(page.getByRole("button", { name: "我想做一个番茄钟小工具，可以开始、暂停、重置。" })).toBeVisible();

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
    await expect(archive.getByRole("heading", { name: "本轮参考" })).toBeVisible();
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
    await expect(main.getByText("工作空间")).toBeVisible();
    await expect(main.getByText("模型密钥")).toBeVisible();
    await expect(main.getByText("预览", { exact: true })).toBeVisible();
    await expect(main.getByText("检查点", { exact: true })).toBeVisible();
  });

  test("first loop progress advances after a request", async ({ page }) => {
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

    await expect(page.getByText("理解目标")).toBeVisible();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");

    await expect(page.getByText("正在制作")).toBeVisible();
    await expect(page.getByText("等你验收")).toBeVisible();
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

    await expect(page.getByRole("button", { name: "检查风险" })).toBeVisible();
    await expect(page.getByRole("button", { name: "继续优化" })).toBeVisible();

    await page.getByRole("button", { name: "检查风险" }).click();
    await expect(page.locator("textarea")).toHaveValue(/检查刚才的改动有没有风险/);
  });
});

test.describe("Project Archive v1", () => {
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

    const archive = page.locator("aside").last();
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("番茄钟小工具")).toBeVisible();
    await expect(archive.getByText("预览可打开 · 检查点已就绪")).toBeVisible();
    await expect(archive.getByText("下一步：继续调整计时器的视觉反馈。")).toBeVisible();
    await expect(archive.getByRole("button", { name: "继续上次任务" })).toBeVisible();

    await archive.getByRole("button", { name: "继续上次任务" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续上次任务/);
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
    const projectRecords = page.locator("section").filter({ has: page.getByRole("heading", { name: "项目记录" }) });
    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });
    const updateProposals = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

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

    await expect(page.getByText("本轮已参考 1 条档案")).toBeVisible();

    await page.getByTitle("打开项目档案").click();

    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });
    const projectMemories = page.locator("section").filter({ has: page.getByRole("heading", { name: "已保存背景" }) });

    await expect(selectedContext.getByText(selectedMemory.body)).toBeVisible();
    await expect(page.getByRole("heading", { name: "建议更新记录" })).toBeVisible();
    await expect(page.getByText(candidateMemory.body)).toBeVisible();
    await expect(page.getByTitle("接受")).toBeVisible();
    await expect(page.getByRole("heading", { name: "项目记录", exact: true })).toBeVisible();
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

    await simulateStream(page, sessionId, [
      { event_type: "memory_candidate", session_id: sessionId, memory: candidateMemory },
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    const inbox = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

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

    const workflowPill = page.getByTestId("workflow-status-pill");
    await expect(workflowPill.getByText("梳理想法", { exact: true })).toBeVisible();
    await expect(page.getByText("这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。", { exact: true })).toBeVisible();

    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");
    await page.getByRole("option", { name: "排查问题" }).click();

    await expect(workflowPill.getByText("排查问题", { exact: true })).toBeVisible();
  });
});

test.describe("Current task work style", () => {
  test("shows stable mode copy and manual override actions", async ({ page }) => {
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
    await expect(currentTask.getByRole("button", { name: "直接回答" })).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "先拆方案" })).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "排查问题" })).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "检查结果" })).toBeVisible();
  });

  test("top-level mode pill opens the Context panel and shows context count", async ({ page }) => {
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

    const pill = page.getByTestId("workflow-status-pill");
    await expect(pill).toContainText("梳理想法");
    await expect(pill).toContainText("已参考 1");
    await pill.click();

    const workbench = page.locator("aside").last();
    await expect(workbench.getByText("项目档案", { exact: true }).first()).toBeVisible();
    await expect(page.getByRole("heading", { name: "当前任务" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "本轮参考" })).toBeVisible();
    const resources = workbench.locator("section").filter({ has: page.getByRole("heading", { name: "资料" }) });
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
    await expect(activeContext.getByText("为什么参考").first()).toBeVisible();
    await expect(activeContext.getByText("来源").first()).toBeVisible();
    await expect(activeContext.getByText("本轮状态").first()).toBeVisible();
    await expect(activeContext.getByText("这是你固定的偏好")).toBeVisible();
    await expect(activeContext.getByText("这页项目记录与本轮请求相关")).toBeVisible();
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

    const inbox = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });
    await expect(inbox.getByText("没有待确认的记录更新")).toBeVisible();
    await expect(inbox.getByText("以后默认用亮色主题")).not.toBeVisible();
  });
});
