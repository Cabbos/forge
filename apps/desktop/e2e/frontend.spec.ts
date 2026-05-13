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
    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
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

    await expect(page.getByText("上轮带入 1 条相关背景")).toBeVisible();

    await page.getByTitle("打开上下文").click();

    await expect(page.getByRole("heading", { name: "相关背景" })).toBeVisible();
    await expect(page.getByText(selectedMemory.body)).toBeVisible();
    await expect(page.getByRole("heading", { name: "待确认" })).toBeVisible();
    await expect(page.getByText(candidateMemory.body)).toBeVisible();
    await expect(page.getByTitle("确认记忆")).toBeVisible();
    await expect(page.getByRole("heading", { name: "项目 Wiki" })).toBeVisible();
    await expect(page.getByText(projectMemory.body)).toBeVisible();
    await expect(page.getByRole("heading", { name: "项目状态" })).toBeVisible();
    await expect(page.getByText("轻量")).toBeVisible();
    await expect(page.getByText("预览运行中")).toBeVisible();
    await expect(page.getByText(otherProjectMemory.body)).toHaveCount(0);
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

    const topBar = page.locator("main > div").first();
    await expect(topBar.getByText("先梳理想法", { exact: true })).toBeVisible();
    await expect(page.getByText("这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。", { exact: true })).toBeVisible();

    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");
    await page.getByRole("option", { name: "排查问题" }).click();

    await expect(topBar.getByText("遇到问题，正在排查", { exact: true })).toBeVisible();
  });
});
