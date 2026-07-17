import { test, expect } from "@playwright/test";
import {
  setup,
  expectLastSendInputArgs,
  expectNoSendInput,
} from "./fixtures/app";
import { simulateStream } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

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

    await page.getByRole("button", { name: "打开工作面板" }).click();
    const panel = page.getByRole("complementary", { name: "工作面板" });
    await expect(panel.getByTestId("work-panel-launcher")).toBeVisible();
    await expect(panel.getByRole("option", { name: /^预览/ })).toBeVisible();
    await expect(panel.getByRole("option", { name: /^文件/ })).toBeVisible();
    await expect(panel.getByText("项目档案", { exact: true })).toHaveCount(0);
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
    await setup(page, { workingDir: "/Users/cabbos/project/forge-test-app" });
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge-test-app");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "get_api_key_status") return [{ provider: "deepseek", configured: false, source: "none", status: "not_configured", error: null }];
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
    await setup(page, { workingDir: sandboxPath });
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
    const sendArgs = await expectLastSendInputArgs(page, { sessionId });
    const sentText = String(sendArgs.text);
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

  test("demo ledger first loop reaches repair and delivery without exposing background records", async ({ page }) => {
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

    await setup(page, { workingDir: sandboxPath });
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
    await expect(confirmCard.getByText("forge-test-app", { exact: true })).toBeVisible();
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
    const repairSendArgs = await expectLastSendInputArgs(page, { sessionId });
    const repairPrompt = String(repairSendArgs.text);
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
    await expect(successfulDelivery.getByText("自动记录")).toHaveCount(0);
    await expect(page.getByRole("main").getByText(sandboxPath, { exact: true })).toHaveCount(0);
    await expect(page.getByRole("main").getByText(/Workflow Router|Task Mode|Living Wiki|Forge Wiki|writeback|ConfirmAsk|permission/i)).toHaveCount(0);
    await expect(page.getByRole("main").getByText(/示例|玩具|临时/)).toHaveCount(0);

    await expect(successfulDelivery.getByText(proposal.summary)).toHaveCount(0);
    await successfulDelivery.getByRole("button", { name: "检查这版" }).click();
    await expect(page.locator("textarea")).toHaveValue(/检查当前版本/);
  });

  test("demo workspace restore keeps the latest result without exposing continuity internals", async ({ page }) => {
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

    await setup(page, { workingDir: sandboxPath });
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

    await page.addInitScript(({ sessionId, summary }) => {
      // @ts-expect-error mock transcript restored by the Tauri fixture
      window.__mockSessionTranscripts = {
        [sessionId]: [
          {
            event_type: "user_message",
            session_id: sessionId,
            block_id: "demo-return-user-message",
            content: "请为收支记录工具做第一版：支持新增收入或支出、展示明细列表，并在页面顶部汇总当前结余。",
          },
          {
            event_type: "delivery_summary",
            session_id: sessionId,
            block_id: "demo-return-delivery-summary",
            summary,
          },
        ],
      };
    }, { sessionId, summary });

    await page.reload();
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(page.getByLabel("当前项目边界").getByText(sandboxPath, { exact: true })).toHaveCount(0);

    const delivery = page.getByTestId("message-panel").filter({ hasText: "本轮交付" });
    await expect(delivery.getByText("预览未运行")).toBeVisible();
    await expect(delivery.getByText("检查点已就绪")).toBeVisible();
    await expect(delivery.getByText("下一步：验收添加收支和合计展示。")).toBeVisible();
    await expect(delivery.getByText("自动记录")).toHaveCount(0);
    await expect(page.getByText("继续上次任务", { exact: true })).toHaveCount(0);
    await expect(page.getByText(sandboxPath, { exact: true })).toHaveCount(0);
  });
});
