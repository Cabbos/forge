import { test, expect } from "@playwright/test";
import { setup } from "./fixtures/app";
import { simulateStream } from "./mock-ipc";

test.describe("Phase 7 acceptance surfaces", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("settings diagnostics surfaces doctor status and gateway runtime", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();
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
              parent_task_id: null,
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
          status: "waiting_for_review",
          owner: { kind: "gateway" },
          policy: {},
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
          outcome: { message: "Waiting for controller review before any commit." },
          completion_result: {
            status: "blocked",
            reasons: ["missing_required_check:npm --prefix apps/desktop run build"],
          },
        },
      },
    ], 1);

    await expect(page.getByTestId("background-task-status")).toContainText("1 Loop 任务");
    await page.getByRole("button", { name: "展开后台任务列表" }).click();
    const drawer = page.getByTestId("background-task-list");
    await expect(drawer).toContainText("Ship Runtime UI and Dashboard Consumption");
    await expect(drawer).toContainText("缺少检查 npm --prefix apps/desktop run build");
    await expect(drawer.getByTestId("loop-review-required")).toContainText("需要人工审阅");
    await expect(drawer).toContainText("成本未知");

    await page.getByRole("button", { name: "打开后台任务面板" }).click();
    const workbench = page.getByRole("region", { name: "子任务" });
    await expect(workbench).toContainText("Runtime UI implementer");
    await expect(workbench).toContainText("需要人工审阅");
    await expect(workbench.locator(".forge-a2a-runtime-facts", { hasText: "文件 IO" })).toContainText("LoopTaskPanel.tsx");
    await expect(workbench.locator(".forge-a2a-runtime-facts", { hasText: "用量" })).toContainText("input 3200 / output unknown / cost unknown");
  });
});
