import { test, expect } from "@playwright/test";
import { resolve } from "node:path";
import { setup } from "./fixtures/app";

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

    await expect(page.getByRole("main").getByTestId("empty-entry-new-tool")).toBeVisible();
    await expect(page.getByRole("main").getByTestId("empty-entry-existing-project")).toBeVisible();
    await expect(page.getByRole("main").getByRole("button", { name: /做个新工具/ })).toBeVisible();
    await expect(page.getByRole("main").getByRole("button", { name: /打开已有项目/ })).toBeVisible();
    await expect(page.getByRole("button", { name: "新对话", exact: true })).toBeDisabled();
  });

  test("conversation list follows the active workspace", async ({ page }) => {
    const workspaceA = "/Users/cabbos/project/app-one";
    const workspaceB = "/Users/cabbos/project/app-two";
    const sessionA = crypto.randomUUID();
    const sessionB = crypto.randomUUID();

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[data-testid='workspace-trigger']", { timeout: 10000 });
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
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    const sessions = [
      { id: "recent-today", title: "Today build", updatedAt: Date.now() },
      { id: "recent-yesterday", title: "Yesterday build", updatedAt: todayStart.getTime() - 12 * 60 * 60 * 1000 },
      { id: "recent-older", title: "Older build", updatedAt: todayStart.getTime() - 8 * 24 * 60 * 60 * 1000 },
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

  test("empty entry opens the folder picker before the first conversation", async ({ page }) => {
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

    const main = page.getByRole("main");
    await main.getByTestId("empty-entry-new-tool").click();

    await expect(page.locator("aside").first().getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByTestId("empty-start-composer").getByRole("textbox")).toBeFocused();
  });

  test("workspace menu can remove the current project from the recent list", async ({ page }) => {
    const workspaceA = "/Users/cabbos/project/remove-one";
    const workspaceB = "/Users/cabbos/project/remove-two";
    await setup(page);
    await page.goto("http://localhost:1420");
    const initialSidebar = page.locator("aside").first();
    await expect(initialSidebar.getByRole("button", { name: /forge/ })).toBeVisible();
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
