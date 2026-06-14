import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  setup,
  expandArchiveRecords,
  expandArchiveFiles,
  expectLastSendInputArgs,
  openProjectArchive,
  projectArchive,
} from "./fixtures/app";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

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

  test("project archive lists available connector materials without reading them", async ({ page }) => {
    const sessionId = "project-archive-connector-materials";
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
    await page.getByTitle("打开项目档案").click();

    const archive = projectArchive(page);
    const materials = await expandArchiveFiles(page);

    await expect(materials.getByText("Forge 研发记录")).toBeVisible();
    await expect(materials.getByText("summarize_issue")).toBeVisible();
    await expect(materials.getByText("连接资料 · obsidian")).toBeVisible();
    await expect(materials.getByText("连接提示词 · linear")).toBeVisible();
    await expect(materials.getByText("可用")).toHaveCount(2);
    await expect(materials.getByText("未加入")).toHaveCount(2);
    await expect(archive.getByText("还没有添加资料")).toHaveCount(0);
  });

  test("selected connector material is sent as hidden turn context", async ({ page }) => {
    const sessionId = "project-archive-selected-material";
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
    await page.getByTitle("打开项目档案").click();

    const materials = await expandArchiveFiles(page);
    await materials.getByRole("button", { name: /Forge 研发记录/ }).click();
    await expect(materials.getByText("已加入")).toHaveCount(1);

    await page.locator("textarea").fill("根据资料总结下一步");
    await page.locator("textarea").press("Enter");

    await expectLastSendInputArgs(page, {
      sessionId,
      mcpContext: [
        {
          kind: "resource",
          server_id: "obsidian",
          uri: "file:///notes/forge.md",
          name: "Forge 研发记录",
        },
      ],
    });
    const userMessage = page.getByTestId("user-message").last();
    await expect(userMessage.getByText("根据资料总结下一步", { exact: true })).toBeVisible();
    await expect(userMessage.getByText("file:///notes/forge.md", { exact: true })).toHaveCount(0);
  });

  test("connector prompt arguments are collected before joining context", async ({ page }) => {
    const sessionId = "project-archive-prompt-arguments";
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
    await page.getByTitle("打开项目档案").click();

    const materials = await expandArchiveFiles(page);
    await materials.getByRole("button", { name: /summarize_issue/ }).click();
    await materials.getByLabel("focus").fill("安全风险");
    await materials.getByRole("button", { name: "加入本轮" }).click();
    await expect(materials.getByText("已加入")).toHaveCount(1);

    await page.locator("textarea").fill("按提示词整理一下");
    await page.locator("textarea").press("Enter");

    await expectLastSendInputArgs(page, {
      sessionId,
      mcpContext: [
        {
          kind: "prompt",
          server_id: "linear",
          name: "summarize_issue",
          arguments: {
            focus: "安全风险",
          },
        },
      ],
    });
  });

  test("connector material row shows read failure status", async ({ page }) => {
    const sessionId = "project-archive-connector-read-failed";
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
    await page.getByTitle("打开项目档案").click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const materials = await expandArchiveFiles(page);
    const row = materials.getByRole("button", { name: /Forge 研发记录/ });
    await row.click();

    await page.evaluate((sessionId) => {
      // @ts-expect-error listeners
      for (const listener of window.__tauriListeners?.["session-output"] ?? []) {
        listener({
          payload: {
            event_type: "mcp_context_status",
            session_id: sessionId,
            source_id: "mcp-resource:obsidian:file:///notes/forge.md",
            status: "failed",
            message: "连接资料读取失败",
          },
        });
      }
    }, sessionId);

    await expect(row.getByText("读取失败", { exact: true })).toBeVisible();
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
              record_label: "建议更新项目记录",
              record_status: "pending",
              record_target_pages: ["tasks.md", "decisions.md"],
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
    const overview = archive.locator("section").filter({ has: page.getByRole("heading", { name: "项目概览" }) });
    await expect(overview.getByText("自动记录", { exact: true })).toBeVisible();
    await expect(overview.getByText("建议更新项目记录")).toBeVisible();
    await expect(overview.getByText("tasks.md, decisions.md")).toBeVisible();
    await expect(overview.getByRole("button", { name: "查看记录" })).toBeVisible();
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

    await openProjectArchive(page, "records");
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
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
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
            return original?.(cmd, args);
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
    await expect(projectMemories.getByText("项目信息", { exact: true })).toBeVisible();
    await expect(projectMemories.getByText("项目事实", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByText("最近状态")).toBeVisible();
    await expect(projectArchive(page).getByText("预览运行中")).toBeVisible();
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
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
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
            return original?.(cmd, args);
        }
      };
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const delivery = await openProjectArchive(page);

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
    await expect(inbox.getByText("写入预览")).toBeVisible();
    await expect(inbox.getByText(proposal.patch_preview)).toBeVisible();
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

test.describe("Timeline Archive", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.evaluate(() => {
      document.querySelectorAll('[data-conversation-theme="light"]').forEach((el) => {
        el.setAttribute("data-conversation-theme", "dark");
      });
    });
  });

  
    test("settings show provider defaults and context window quietly", async ({ page }) => {
      await page.setViewportSize({ width: 1280, height: 720 });
      await page.getByRole("button", { name: "设置" }).click();
  
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("heading", { name: "模型服务" })).toBeVisible();
      await expect(dialog.getByTestId("settings-preferences-panel")).toBeVisible();
      const providerRows = dialog.getByTestId("settings-provider-row");
      await expect(providerRows.first()).toBeVisible();
      expect(await providerRows.count()).toBeGreaterThanOrEqual(1);
      const settingsMetrics = await dialog.evaluate((node) => {
        const panel = node.querySelector<HTMLElement>("[data-testid='settings-preferences-panel']");
        const rows = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='settings-provider-row']"));
        const status = node.querySelector<HTMLElement>("[data-testid='settings-provider-status']");
        const center = node.querySelector<HTMLElement>(".forge-settings-center");
        const sidebar = node.querySelector<HTMLElement>(".forge-settings-sidebar");
        const content = node.querySelector<HTMLElement>(".forge-settings-content");
        const panelStyle = panel ? getComputedStyle(panel) : null;
        const rowStyle = rows[0] ? getComputedStyle(rows[0]) : null;
        const secondRowStyle = rows[1] ? getComputedStyle(rows[1]) : null;
        const centerRect = center?.getBoundingClientRect();
        const sidebarRect = sidebar?.getBoundingClientRect();
        const contentRect = content?.getBoundingClientRect();
        return {
          panelRadius: panelStyle ? Number.parseFloat(panelStyle.borderTopLeftRadius) : 0,
          panelDisplay: panelStyle?.display ?? "",
          panelGap: panelStyle?.rowGap ?? "",
          panelOverflow: panelStyle?.overflow ?? "",
          centerBottom: centerRect ? Math.round(centerRect.bottom) : 0,
          sidebarBottom: sidebarRect ? Math.round(sidebarRect.bottom) : 0,
          contentBottom: contentRect ? Math.round(contentRect.bottom) : 0,
          contentCanScroll: content ? content.scrollHeight > content.clientHeight : false,
          contentOverflowY: content ? getComputedStyle(content).overflowY : "",
          firstRowHeight: rows[0] ? Math.round(rows[0].getBoundingClientRect().height) : 0,
          firstRowDisplay: rowStyle?.display ?? "",
          firstRowBackground: rowStyle?.backgroundColor ?? "",
          firstRowBorderTop: rowStyle?.borderTopColor ?? "",
          secondRowBorderTop: secondRowStyle?.borderTopColor ?? "",
          firstRowTransition: rowStyle?.transitionProperty ?? "",
          statusRadius: status ? Number.parseFloat(getComputedStyle(status).borderTopLeftRadius) : 0,
          statusBorder: status ? getComputedStyle(status).borderColor : "",
        };
      });
      expect(settingsMetrics.panelRadius).toBeLessThanOrEqual(8);
      expect(settingsMetrics.panelDisplay).toBe("grid");
      expect(settingsMetrics.panelGap).toBe("8px");
      expect(settingsMetrics.panelOverflow).toBe("visible");
      expect(settingsMetrics.sidebarBottom).toBeLessThanOrEqual(settingsMetrics.centerBottom);
      expect(settingsMetrics.contentBottom).toBeLessThanOrEqual(settingsMetrics.centerBottom);
      expect(settingsMetrics.contentCanScroll).toBe(true);
      expect(settingsMetrics.contentOverflowY).toBe("auto");
      expect(settingsMetrics.firstRowHeight).toBeGreaterThanOrEqual(64);
      expect(settingsMetrics.firstRowDisplay).toBe("grid");
      expect(settingsMetrics.firstRowBackground).not.toBe("rgba(0, 0, 0, 0)");
      expect(settingsMetrics.firstRowBorderTop).toBe("rgba(0, 0, 0, 0)");
      expect(settingsMetrics.secondRowBorderTop).toBe("rgba(0, 0, 0, 0)");
      expect(settingsMetrics.firstRowTransition.includes("background-color")).toBe(true);
      expect(settingsMetrics.statusRadius).toBeLessThanOrEqual(8);
      expect(settingsMetrics.statusBorder).not.toBe("rgba(0, 0, 0, 0)");
      await providerRows.first().hover();
      await expect(providerRows.first()).not.toHaveCSS("border-color", "rgba(0, 0, 0, 0)");
      const deepseek = dialog
        .getByTestId("settings-preferences-panel")
        .getByTestId("settings-provider-row")
        .filter({ hasText: "DeepSeek" });
      await expect(deepseek.getByText("DeepSeek V4 Flash 1M")).toBeVisible();
      await expect(deepseek.getByText("默认模型 · 上下文 1M")).toBeVisible();
      await expect(deepseek.getByText("deepseek-v4-flash[1m]")).toHaveCount(0);
      const contentScroll = dialog.locator(".forge-settings-content");
      const scrollBefore = await contentScroll.evaluate((node) => node.scrollTop);
      const scrollAfter = await contentScroll.evaluate((node) => {
        node.scrollTop = 500;
        return node.scrollTop;
      });
      expect(scrollAfter).toBeGreaterThan(scrollBefore);
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
      const metrics = await card.evaluate((node) => {
        const grid = node.querySelector<HTMLElement>("[data-testid='delivery-summary-grid']");
        const items = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='delivery-summary-item']"));
        const actionBar = node.querySelector<HTMLElement>("[data-testid='delivery-action-bar']");
        const action = node.querySelector<HTMLElement>("[data-testid='delivery-primary-action']");
        const gridStyle = grid ? getComputedStyle(grid) : null;
        const actionStyle = action ? getComputedStyle(action) : null;
        return {
          width: Math.round(node.getBoundingClientRect().width),
          gridDisplay: gridStyle?.display ?? "",
          gridWrap: gridStyle?.flexWrap ?? "",
          gridGap: gridStyle?.gap ?? "",
          itemCount: items.length,
          maxItemHeight: items.length ? Math.max(...items.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
          actionBarHeight: actionBar ? Math.round(actionBar.getBoundingClientRect().height) : 0,
          actionHeight: action ? Math.round(action.getBoundingClientRect().height) : 0,
          actionRadius: actionStyle ? Number.parseFloat(actionStyle.borderTopLeftRadius) : 0,
          actionBackground: actionStyle?.backgroundColor ?? "",
        };
      });
      expect(metrics.width).toBeLessThanOrEqual(720);
      expect(metrics.gridDisplay).toBe("flex");
      expect(metrics.gridWrap).toBe("wrap");
      expect(metrics.gridGap).not.toBe("normal");
      expect(metrics.maxItemHeight).toBeLessThanOrEqual(72);
      expect(metrics.actionBarHeight).toBeLessThanOrEqual(42);
      expect(metrics.actionHeight).toBe(28);
      expect(metrics.actionRadius).toBeLessThanOrEqual(8);
      expect(metrics.actionBackground).not.toBe("rgba(0, 0, 0, 0)");
      await card.getByRole("button", { name: "继续修复" }).click();
  
      await expect(page.locator("textarea")).toHaveValue(/npm run build/);
      await expect(page.locator("textarea")).toHaveValue(/继续修复/);
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
  
    test("project archive summary gives quick project, context, and record state", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
      await page.getByTitle("打开项目档案").click();
  
      const archive = page.getByRole("complementary", { name: "项目档案" });
      await expect(archive).toBeVisible();
      const summary = archive.getByTestId("project-archive-summary-strip");
      await expect(summary).toBeVisible();
      await expect(summary.getByText("项目", { exact: true })).toBeVisible();
      await expect(summary.getByText("上下文", { exact: true })).toBeVisible();
      await expect(summary.getByText("记录", { exact: true })).toBeVisible();
  
      const metrics = await summary.evaluate((node) => {
        const items = Array.from(node.querySelectorAll<HTMLElement>(".forge-archive-summary-item"));
        const firstIcon = node.querySelector<HTMLElement>(".forge-archive-summary-icon");
        const firstValue = node.querySelector<HTMLElement>(".forge-archive-summary-value");
        return {
          display: getComputedStyle(node).display,
          itemCount: items.length,
          maxItemHeight: items.length ? Math.max(...items.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
          iconWidth: firstIcon ? Math.round(firstIcon.getBoundingClientRect().width) : 0,
          valueOverflow: firstValue ? getComputedStyle(firstValue).textOverflow : "",
        };
      });
  
      expect(metrics.display).toBe("grid");
      expect(metrics.itemCount).toBe(3);
      expect(metrics.maxItemHeight).toBeLessThanOrEqual(50);
      expect(metrics.iconWidth).toBe(24);
      expect(metrics.valueOverflow).toBe("ellipsis");
    });
  
});
