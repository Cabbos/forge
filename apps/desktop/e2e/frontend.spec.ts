import { test, expect, type Page } from "@playwright/test";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

/** Setup: inject mock IPC before the app loads */
async function setup(page: Page) {
  await page.addInitScript(() => {
    let callbackId = 0;
    const callbacks = new Map<number, (data: unknown) => void>();
    const workingDir = "/Users/cabbos/project/crusted-spinning-lynx-agent";
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
      message: exists ? "Forge Wiki is ready." : "还没有项目 Wiki",
    });
    const forgeWikiProposal = (projectPath: string, args: Record<string, unknown>) => ({
      id: String(args.proposalId ?? args.id ?? "forge-wiki-proposal"),
      project_path: projectPath,
      session_id: typeof args.sessionId === "string" ? args.sessionId : null,
      target_pages: Array.isArray(args.targetPages) ? args.targetPages.map(String) : ["tasks.md"],
      title: String(args.title ?? "记录 Forge Wiki 更新"),
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
          return { session_id: window.__mockSessionId ?? crypto.randomUUID() };
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
          return args.pagePath === "tasks.md" ? "# 当前任务\n\n覆盖 Forge Wiki 上下文面板。" : "# 项目概览\n\nForge Wiki mock project overview.";
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
    await expect(page.getByRole("main").getByText("创建一个任务开始").last()).toBeVisible();
  });

  test("creating a session shows chat input", async ({ page }) => {
    await page.goto("http://localhost:1420");
    // Click new session button
    await page.getByRole("button", { name: "新对话" }).click();
    // Input should appear
    await expect(page.locator("textarea")).toBeVisible();
  });

  test("timeline messages render correctly", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    // Create session
    await page.getByRole("button", { name: "新对话" }).click();
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

  test("thinking block expands and shows content", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
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
    await page.getByRole("button", { name: "新对话" }).click();
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
    await expect(sidebar.getByRole("button", { name: "新对话" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "插件" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "自动化" })).toBeVisible();
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
    await page.getByRole("button", { name: "新对话" }).click();
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
    await page.getByRole("button", { name: "新对话" }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("line1");
    await textarea.press("Shift+Enter");
    await textarea.pressSequentially("line2");

    // Should still be in the textarea, not sent
    await expect(textarea).toContainText("line1\nline2");
  });
});

test.describe("Living Wiki context panel", () => {
  test("Forge Wiki context panel initializes wiki and shows selected pages", async ({ page }) => {
    const sessionId = "forge-wiki-session";
    const projectPath = "/Users/cabbos/project/crusted-spinning-lynx-agent";
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
      title: "记录 Forge Wiki e2e 覆盖",
      summary: "补充上下文面板初始化、带入页面和更新建议的测试记录。",
      patch_preview: "追加 e2e 覆盖说明。",
      status: "pending" as const,
      created_at: now,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("tui-to-gui-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.getByTitle("打开工作台").click();
    const projectRecords = page.locator("section").filter({ has: page.getByRole("heading", { name: "项目记录" }) });
    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮上下文" }) });
    const updateProposals = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

    await expect(projectRecords.getByText("还没有项目记录", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "建立项目记录" }).click();
    await expect(projectRecords.getByText(/当前任务|项目概览/).first()).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await expect(selectedContext.getByText(selectedPage.summary)).toBeVisible();
    await expect(selectedContext.getByText("已带入 1 条背景")).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    await expect(updateProposals.getByText(proposal.summary)).toBeVisible();
  });

  test("shows selected context, project wiki, project status, and scopes project memories", async ({ page }) => {
    const sessionId = "living-wiki-session";
    const projectPath = "/Users/cabbos/project/crusted-spinning-lynx-agent";
    const otherProjectPath = "/Users/cabbos/project/elsewhere";
    const now = "2026-05-13T00:00:00.000Z";
    const selectedMemory = {
      id: "memory-selected-1",
      category: "preference",
      scope: "user_profile",
      status: "accepted",
      title: "Use Living Wiki context",
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
      title: "Project Wiki fact",
      body: "The active project uses the HubPanel for Living Wiki review.",
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
      body: "This memory belongs to another project and should stay hidden.",
      project_path: otherProjectPath,
    };
    const candidateMemory = {
      ...projectMemory,
      id: "memory-candidate-1",
      category: "decision",
      status: "candidate",
      title: "Candidate Wiki note",
      body: "This candidate should be visible before it is accepted.",
      confidence: 0.72,
      tags: ["candidate"],
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath, memories }) => {
      window.localStorage.clear();
      window.localStorage.setItem("tui-to-gui-working-dir", projectPath);

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
    await page.getByRole("button", { name: "新对话" }).click();
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

    await expect(page.getByText("本轮已带入 1 条背景")).toBeVisible();

    await page.getByTitle("打开工作台").click();

    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮上下文" }) });
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

  test("groups memory candidates and Wiki proposals as update suggestions", async ({ page }) => {
    const sessionId = "memory-inbox-session";
    const projectPath = "/Users/cabbos/project/crusted-spinning-lynx-agent";
    const now = "2026-05-13T00:00:00.000Z";
    const candidateMemory = {
      id: "candidate-1",
      category: "decision" as const,
      scope: "project" as const,
      status: "candidate" as const,
      title: "项目已定方案：上下文优先",
      body: "右侧面板优先展示当前任务和本轮上下文。",
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
      title: "记录上下文激活计划",
      summary: "补充 Task Mode 和 Context Activation 的下一步。",
      patch_preview: "追加任务记录。",
      status: "pending" as const,
      created_at: now,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("tui-to-gui-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await page.getByTitle("打开工作台").click();

    await simulateStream(page, sessionId, [
      { event_type: "memory_candidate", session_id: sessionId, memory: candidateMemory },
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    const inbox = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

    await expect(inbox.getByText(candidateMemory.body)).toBeVisible();
    await expect(inbox.getByText(proposal.summary)).toBeVisible();
    await expect(inbox.getByRole("button", { name: "接受" }).first()).toBeVisible();
    await expect(inbox.getByRole("button", { name: /忘记|丢弃/ }).first()).toBeVisible();
  });
});

test.describe("Workflow Router", () => {
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
    await page.getByRole("button", { name: "新对话" }).click();
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

test.describe("Task Mode", () => {
  test("shows stable mode copy and manual override actions", async ({ page }) => {
    const sessionId = "task-mode-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
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

    await page.getByTitle("打开工作台").click();
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
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
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
          summary: "正在做上下文激活。",
          score: 0.9,
          reason: "这页项目记录与本轮请求相关",
          injected: true,
        }],
      },
    ], 5);

    const pill = page.getByTestId("workflow-status-pill");
    await expect(pill).toContainText("梳理想法");
    await expect(pill).toContainText("已带入 1");
    await pill.click();

    await expect(page.locator("aside").last().getByText("工作台", { exact: true }).first()).toBeVisible();
    await expect(page.getByRole("heading", { name: "当前任务" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "上下文", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "本轮上下文" })).toBeVisible();
  });
});

test.describe("Context Activation", () => {
  test("shows active memories and Forge Wiki pages for the current turn", async ({ page }) => {
    const sessionId = "context-activation-session";
    const projectPath = "/Users/cabbos/project/crusted-spinning-lynx-agent";
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
      summary: "当前正在做 Task Mode 和 Context Activation。",
      score: 0.91,
      reason: "这页项目记录与本轮请求相关",
      injected: true,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("tui-to-gui-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "memory_selection", session_id: sessionId, selected: [selectedMemory] },
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await page.getByTitle("打开工作台").click();
    const activeContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮上下文" }) });

    await expect(activeContext.getByText("已带入 2 条背景")).toBeVisible();
    await expect(activeContext.getByText("中文优先")).toBeVisible();
    await expect(activeContext.getByText("当前任务")).toBeVisible();
    await expect(activeContext.getByText("这是你固定的偏好")).toBeVisible();
    await expect(activeContext.getByText("这页项目记录与本轮请求相关")).toBeVisible();
  });

  test("does not show a memory candidate when user says not to remember", async ({ page }) => {
    const sessionId = "no-memory-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("不要记住这个，只是临时测试：以后默认用亮色主题。");
    await page.locator("textarea").press("Enter");
    await page.getByTitle("打开工作台").click();

    const inbox = page.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });
    await expect(inbox.getByText("没有待确认的记录更新")).toBeVisible();
    await expect(inbox.getByText("以后默认用亮色主题")).not.toBeVisible();
  });
});
