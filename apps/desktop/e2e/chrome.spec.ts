import { test, expect } from "@playwright/test";
import { setup, holdSendInput, expectHeldSendInput, releaseHeldSendInput } from "./fixtures/app";
import { simulateStream } from "./mock-ipc";

test.describe("Desktop Empty Workspace Layout", () => {
  test("no-project empty workbench stays centered inside wide desktop windows", async ({ page }) => {
    await setup(page, { workingDir: null });
    await page.setViewportSize({ width: 1600, height: 1000 });
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    const main = page.getByRole("main");
    await expect(main.getByTestId("empty-workbench")).toBeVisible();
    await expect(main.getByTestId("empty-entry-new-tool")).toBeVisible();
    await expect(main.getByTestId("empty-entry-existing-project")).toBeVisible();
    await expect(main.getByTestId("empty-start-composer")).toHaveCount(0);

    const metrics = await main.evaluate((node) => {
      const mainEl = node as HTMLElement;
      const shell = mainEl.querySelector<HTMLElement>(".forge-empty-shell-centered");
      const workbench = mainEl.querySelector<HTMLElement>("[data-testid='empty-workbench']");
      const grid = mainEl.querySelector<HTMLElement>(".forge-empty-entry-grid");
      const notice = mainEl.querySelector<HTMLElement>("[data-testid='empty-workspace-notice']");
      const cards = Array.from(mainEl.querySelectorAll<HTMLElement>("[data-testid^='empty-entry-']"));
      if (!shell || !workbench || !grid || !notice || cards.length < 2) return null;

      const mainRect = mainEl.getBoundingClientRect();
      const workbenchRect = workbench.getBoundingClientRect();
      const gridRect = grid.getBoundingClientRect();
      const noticeRect = notice.getBoundingClientRect();
      const gridCenter = gridRect.left + gridRect.width / 2;
      const mainCenter = mainRect.left + mainRect.width / 2;

      return {
        shellOverflowX: getComputedStyle(shell).overflowX,
        workbenchWidth: Math.round(workbenchRect.width),
        gridWidth: Math.round(gridRect.width),
        mainWidth: Math.round(mainRect.width),
        gridLeft: Math.round(gridRect.left - mainRect.left),
        gridRightGap: Math.round(mainRect.right - gridRect.right),
        centerDelta: Math.round(Math.abs(gridCenter - mainCenter)),
        cardWidths: cards.map((card) => Math.round(card.getBoundingClientRect().width)),
        noticeLeftGap: Math.round(noticeRect.left - mainRect.left),
        noticeRightGap: Math.round(mainRect.right - noticeRect.right),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.shellOverflowX).toBe("hidden");
    expect(metrics!.workbenchWidth).toBeLessThanOrEqual(640);
    expect(metrics!.gridWidth).toBeLessThanOrEqual(metrics!.mainWidth - 48);
    expect(metrics!.gridLeft).toBeGreaterThanOrEqual(24);
    expect(metrics!.gridRightGap).toBeGreaterThanOrEqual(24);
    expect(metrics!.centerDelta).toBeLessThanOrEqual(2);
    expect(Math.abs(metrics!.cardWidths[0] - metrics!.cardWidths[1])).toBeLessThanOrEqual(1);
    expect(metrics!.noticeLeftGap).toBeGreaterThanOrEqual(24);
    expect(metrics!.noticeRightGap).toBeGreaterThanOrEqual(24);
  });

  test("work panel launcher keeps empty start choices readable on narrow desktop", async ({ page }) => {
    await setup(page, { workingDir: null });
    await page.setViewportSize({ width: 900, height: 720 });
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "打开工作面板" }).click();
    await expect(page.getByTestId("work-panel-launcher")).toBeVisible();

    const metrics = await page.getByRole("main").evaluate((node) => {
      const main = node as HTMLElement;
      const panel = document.querySelector<HTMLElement>("aside.forge-work-panel");
      const grid = main.querySelector<HTMLElement>(".forge-empty-entry-grid");
      const cards = Array.from(main.querySelectorAll<HTMLElement>(".forge-empty-entry-card"));
      const actions = Array.from(document.querySelectorAll<HTMLElement>(".forge-work-panel-launcher-action"));
      if (!panel || !grid || cards.length < 2 || actions.length < 5) return null;
      const panelRect = panel.getBoundingClientRect();
      const gridRect = grid.getBoundingClientRect();
      const cardRects = cards.map((card) => card.getBoundingClientRect());
      const actionRects = actions.map((action) => action.getBoundingClientRect());
      return {
        columnCount: getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length,
        gridRight: Math.round(gridRect.right),
        panelLeft: Math.round(panelRect.left),
        panelWidth: Math.round(panelRect.width),
        cardWidths: cardRects.map((rect) => Math.round(rect.width)),
        cardTops: cardRects.map((rect) => Math.round(rect.top)),
        actionWidths: actionRects.map((rect) => Math.round(rect.width)),
        actionHeights: actionRects.map((rect) => Math.round(rect.height)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.columnCount).toBeGreaterThanOrEqual(1);
    expect(metrics!.columnCount).toBeLessThanOrEqual(2);
    expect(metrics!.gridRight).toBeLessThanOrEqual(metrics!.panelLeft);
    expect(metrics!.panelWidth).toBeGreaterThanOrEqual(270);
    expect(metrics!.actionWidths.every((width) => width >= 190)).toBe(true);
    expect(metrics!.actionHeights.every((height) => height >= 58)).toBe(true);
    expect(metrics!.cardWidths.every((width) => width >= 200)).toBe(true);
    expect(metrics!.cardTops[1]).toBeGreaterThan(metrics!.cardTops[0]);
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

test.describe("Timeline Chrome", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });


    test("empty workbench start controls read as compact desktop rails", async ({ page }) => {
      const main = page.getByRole("main");
      const entries = main.locator("[data-forge-motion='empty-entry']");
      await expect(entries).toHaveCount(2);
      await expect(main.locator("[data-forge-motion='empty-composer']")).toBeVisible();
      await expect(main.locator("[data-forge-motion='empty-context']")).toBeVisible();
  
      const metrics = await main.evaluate((node) => {
        const cards = Array.from(node.querySelectorAll<HTMLElement>("[data-forge-motion='empty-entry']"));
        const composer = node.querySelector<HTMLElement>("[data-forge-motion='empty-composer']");
        if (!composer || cards.length < 2) return null;
        return {
          cardHeights: cards.map((card) => Math.round(card.getBoundingClientRect().height)),
          cardDisplays: cards.map((card) => getComputedStyle(card).display),
          cardAlignments: cards.map((card) => getComputedStyle(card).alignItems),
          cardShadows: cards.map((card) => getComputedStyle(card).boxShadow),
          composerWidth: Math.round(composer.getBoundingClientRect().width),
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(Math.max(...metrics!.cardHeights)).toBeLessThanOrEqual(82);
      expect(metrics!.cardDisplays.every((display) => display === "flex")).toBe(true);
      expect(metrics!.cardAlignments.every((alignment) => alignment === "center")).toBe(true);
      expect(metrics!.cardShadows.every((shadow) => shadow === "none")).toBe(true);
      expect(metrics!.composerWidth).toBeGreaterThanOrEqual(520);
    });

    test("empty workbench stays grounded in short desktop windows", async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 520 });
      await page.goto("http://localhost:1420");
  
      const main = page.getByRole("main");
      await expect(main.getByTestId("empty-start-composer")).toBeVisible();
      await expect(main.getByTestId("empty-middle-hints")).toBeHidden();
  
      const metrics = await main.evaluate((node) => {
        const mainRect = (node as HTMLElement).getBoundingClientRect();
        const frame = node.querySelector<HTMLElement>(".forge-empty-composer-frame");
        const composer = node.querySelector<HTMLElement>("[data-testid='empty-start-composer']");
        const input = node.querySelector<HTMLElement>(".forge-empty-composer-input");
        const frameRect = frame?.getBoundingClientRect();
        const composerRect = composer?.getBoundingClientRect();
        return {
          composerBottomGap: frameRect ? Math.round(mainRect.bottom - frameRect.bottom) : -1,
          composerTop: composerRect ? Math.round(composerRect.top - mainRect.top) : 0,
          mainHeight: Math.round(mainRect.height),
          inputMinHeight: input ? Math.round(Number.parseFloat(getComputedStyle(input).minHeight)) : 0,
        };
      });
  
      expect(metrics.composerBottomGap).toBeLessThanOrEqual(1);
      expect(metrics.composerTop).toBeGreaterThan(metrics.mainHeight * 0.5);
      expect(metrics.inputMinHeight).toBeLessThanOrEqual(64);
    });

    test("titlebar presents session and project state as a compact desktop status bar", async ({ page }) => {
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
      await holdSendInput(page);
      await page.locator("textarea").fill("Titlebar polish status");
      await page.locator("textarea").press("Enter");
      await expectHeldSendInput(page, "Titlebar polish status");
  
      const titlebar = page.getByTestId("app-titlebar");
      await expect(titlebar.getByTestId("titlebar-title")).toContainText("Titlebar polish status");
      await expect(titlebar.getByTestId("titlebar-project-boundary")).toContainText("forge");
      await expect(titlebar.getByTestId("titlebar-status-pill")).toContainText("响应中");
      await expect(titlebar.getByTestId("titlebar-status-pill")).toHaveAttribute("data-state", "running");
      await expect(titlebar.getByTestId("titlebar-actions")).toBeVisible();
  
      const metrics = await titlebar.evaluate((node) => {
        const title = node.querySelector<HTMLElement>("[data-testid='titlebar-title']");
        const project = node.querySelector<HTMLElement>("[data-testid='titlebar-project-boundary']");
        const status = node.querySelector<HTMLElement>("[data-testid='titlebar-status-pill']");
        const actions = node.querySelector<HTMLElement>("[data-testid='titlebar-actions']");
        const buttons = Array.from(node.querySelectorAll<HTMLElement>(".forge-titlebar-button"));
        if (!title || !project || !status || !actions) return null;
        const statusStyle = getComputedStyle(status);
        const projectStyle = getComputedStyle(project);
        const actionsStyle = getComputedStyle(actions);
        return {
          titlebarHeight: Math.round(node.getBoundingClientRect().height),
          contextLeft: Math.round(title.getBoundingClientRect().left - node.getBoundingClientRect().left),
          actionsRightGap: Math.round(node.getBoundingClientRect().right - actions.getBoundingClientRect().right),
          titleLineHeight: Math.round(Number.parseFloat(getComputedStyle(title).lineHeight)),
          projectHeight: Math.round(project.getBoundingClientRect().height),
          projectTopGap: Math.round(project.getBoundingClientRect().top - title.getBoundingClientRect().bottom),
          projectBackground: projectStyle.backgroundColor,
          projectBorder: projectStyle.borderTopColor,
          statusHeight: Math.round(status.getBoundingClientRect().height),
          statusRadius: Number.parseFloat(statusStyle.borderTopLeftRadius),
          statusBackground: statusStyle.backgroundColor,
          actionsGap: Math.round(Number.parseFloat(actionsStyle.columnGap)),
          buttonSizes: buttons.map((button) => ({
            width: Math.round(button.getBoundingClientRect().width),
            height: Math.round(button.getBoundingClientRect().height),
          })),
          buttonTransitions: buttons.map((button) => getComputedStyle(button).transitionProperty),
        };
      });
  
      expect(metrics).not.toBeNull();
      expect(metrics!.titlebarHeight).toBe(64);
      expect(metrics!.contextLeft).toBeGreaterThanOrEqual(24);
      expect(metrics!.actionsRightGap).toBeGreaterThanOrEqual(18);
      expect(metrics!.titleLineHeight).toBeLessThanOrEqual(20);
      expect(metrics!.projectHeight).toBe(24);
      expect(metrics!.projectTopGap).toBeGreaterThanOrEqual(4);
      expect(metrics!.projectBackground).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics!.projectBorder).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics!.statusHeight).toBeLessThanOrEqual(20);
      expect(metrics!.statusRadius).toBeLessThanOrEqual(8);
      expect(metrics!.statusBackground).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics!.actionsGap).toBe(4);
      expect(metrics!.buttonSizes).toEqual([{ width: 32, height: 32 }, { width: 32, height: 32 }]);
      expect(metrics!.buttonTransitions.every((value) => value.includes("background-color"))).toBe(true);
  
      const searchButton = titlebar.getByRole("button", { name: "搜索" });
      await searchButton.hover();
      await expect(searchButton).not.toHaveCSS("border-color", "rgba(0, 0, 0, 0)");
      await releaseHeldSendInput(page);
    });

    test("search command palette keeps portal surfaces in the light workbench theme", async ({ page }) => {
      const searchButton = page.locator('button[aria-label="搜索"]');
      await expect(searchButton).toHaveCount(1);
      await searchButton.click();

      await expect(page.getByRole("dialog")).toBeVisible();
      await expect(page.getByPlaceholder("搜索或输入命令...")).toBeVisible();

      const paletteMetrics = await page.evaluate(() => {
        const overlay = document.querySelector<HTMLElement>("[data-slot='dialog-overlay']");
        const dialog = document.querySelector<HTMLElement>("[data-slot='dialog-content']");
        const surface = document.querySelector<HTMLElement>("[data-testid='command-palette-surface']");
        return {
          bodyBackground: getComputedStyle(document.body).backgroundColor,
          overlayBackground: overlay ? getComputedStyle(overlay).backgroundColor : "",
          dialogBackground: dialog ? getComputedStyle(dialog).backgroundColor : "",
          surfaceBackground: surface ? getComputedStyle(surface).backgroundColor : "",
          hasViteOverlay: Boolean(document.querySelector("vite-error-overlay")),
        };
      });

      expect(paletteMetrics.bodyBackground).toBe("rgb(252, 253, 254)");
      expect(paletteMetrics.overlayBackground).toBe("rgba(244, 246, 248, 0.72)");
      expect(paletteMetrics.dialogBackground).toBe("rgb(255, 255, 255)");
      expect(paletteMetrics.surfaceBackground).toBe("rgb(255, 255, 255)");
      expect(paletteMetrics.hasViteOverlay).toBe(false);
    });

    test("reduced motion keeps running chrome steady", async ({ page }) => {
      await page.emulateMedia({ reducedMotion: "reduce" });
      const sessionId = crypto.randomUUID();
      await page.addInitScript((sessionId) => {
        // @ts-expect-error mock
        window.__mockSessionId = sessionId;
      }, sessionId);
  
      await page.goto("http://localhost:1420");
      await page.getByRole("button", { name: "新对话", exact: true }).click();
      await expect(page.locator("textarea")).toBeVisible();
      await holdSendInput(page);
      await page.locator("textarea").fill("Reduced motion status");
      await page.locator("textarea").press("Enter");
      await expectHeldSendInput(page, "Reduced motion status");
  
      const metrics = await page.evaluate(() => {
        const statusDot = document.querySelector<HTMLElement>(".forge-titlebar-status-dot");
        const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
        const dotStyle = statusDot ? getComputedStyle(statusDot) : null;
        const composerStyle = composer ? getComputedStyle(composer) : null;
        return {
          dotAnimationName: dotStyle?.animationName ?? "",
          dotAnimationDuration: dotStyle?.animationDuration ?? "",
          composerTransitionDuration: composerStyle?.transitionDuration ?? "",
        };
      });
  
      expect(metrics.dotAnimationName).toBe("none");
      expect(metrics.dotAnimationDuration).toBe("0s");
      expect(metrics.composerTransitionDuration.split(", ").every((duration) => duration === "0s")).toBe(true);
      await releaseHeldSendInput(page);
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
  
      const events = Array.from({ length: 40 }, (_, index) => ([
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
        scroller.dispatchEvent(new WheelEvent("wheel", { deltaY: -160, bubbles: true }));
        scroller.scrollTop = 0;
        scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
      });
  
      const control = page.getByTestId("scroll-to-bottom");
      await expect(control).toBeVisible();
      const metrics = await control.evaluate((node) => {
        const style = getComputedStyle(node);
        return {
          background: style.backgroundColor,
          backdrop: style.backdropFilter || style.getPropertyValue("-webkit-backdrop-filter"),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          shadow: style.boxShadow,
          width: Math.round(node.getBoundingClientRect().width),
          height: Math.round(node.getBoundingClientRect().height),
        };
      });
      expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics.backdrop).not.toBe("none");
      expect(metrics.radius).toBeLessThanOrEqual(8);
      expect(metrics.shadow).not.toBe("none");
      expect(metrics.width).toBe(28);
      expect(metrics.height).toBe(28);
    });

    test("sidebar shows persistent navigation", async ({ page }) => {
      const sidebar = page.locator("aside").first();
  
      const width = (await sidebar.boundingBox())?.width ?? 0;
      expect(width).toBeGreaterThanOrEqual(212);
      expect(width).toBeLessThanOrEqual(240);
      await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeVisible();
      await expect(sidebar.getByRole("button", { name: "插件" })).toBeVisible();
      await expect(sidebar.getByRole("button", { name: "自动化" })).toBeVisible();
      await expect(sidebar.getByRole("button", { name: "设置" })).toBeVisible();
      await expect(sidebar.getByText("当前工作空间", { exact: true })).toHaveCount(0);
      await expect(sidebar.getByText("插件", { exact: true })).toHaveCount(0);
      await expect(sidebar.getByText("自动化", { exact: true })).toHaveCount(0);
      await expect(sidebar.getByText("设置", { exact: true })).toHaveCount(0);
      await expect(sidebar.getByTestId("sidebar-primary-nav")).toBeVisible();
      const railMetrics = await sidebar.evaluate((node) => {
        const workspace = node.querySelector<HTMLElement>("[data-testid='workspace-trigger']");
        const primaryNav = node.querySelector<HTMLElement>("[data-testid='sidebar-primary-nav']");
        const primaryActions = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='sidebar-primary-action']"));
        const brand = node.querySelector<HTMLElement>(".forge-sidebar-brand");
        const sidebarStyle = getComputedStyle(node);
        const style = workspace ? getComputedStyle(workspace) : null;
        return {
          sidebarPaddingLeft: Math.round(Number.parseFloat(sidebarStyle.paddingLeft)),
          sidebarPaddingRight: Math.round(Number.parseFloat(sidebarStyle.paddingRight)),
          sidebarPaddingBottom: Math.round(Number.parseFloat(sidebarStyle.paddingBottom)),
          workspaceHeight: workspace ? Math.round(workspace.getBoundingClientRect().height) : 0,
          workspaceRadius: style ? Number.parseFloat(style.borderTopLeftRadius) : 0,
          workspaceBorder: style?.borderTopColor ?? "",
          workspaceBackground: style?.backgroundColor ?? "",
          primaryGap: primaryNav ? Math.round(Number.parseFloat(getComputedStyle(primaryNav).rowGap)) : -1,
          primaryActions: primaryActions.map((button) => Math.round(button.getBoundingClientRect().height)),
          primaryTransitions: primaryActions.map((button) => getComputedStyle(button).transitionProperty),
          brandHeight: brand ? Math.round(brand.getBoundingClientRect().height) : 0,
        };
      });
      expect(railMetrics.sidebarPaddingLeft).toBeGreaterThanOrEqual(8);
      expect(railMetrics.sidebarPaddingRight).toBeGreaterThanOrEqual(8);
      expect(railMetrics.sidebarPaddingBottom).toBeGreaterThanOrEqual(8);
      expect(railMetrics.workspaceHeight).toBe(30);
      expect(railMetrics.workspaceRadius).toBeLessThanOrEqual(8);
      expect(railMetrics.workspaceBorder).toBe("rgba(0, 0, 0, 0)");
      expect(railMetrics.workspaceBackground).toBe("rgba(34, 32, 28, 0.74)");
      expect(railMetrics.primaryGap).toBeLessThanOrEqual(3);
      expect(railMetrics.primaryActions).toEqual([28, 28]);
      expect(railMetrics.brandHeight).toBeGreaterThanOrEqual(40);
      expect(railMetrics.brandHeight).toBeLessThanOrEqual(46);
      const utilityMetrics = await sidebar.locator("[data-testid='sidebar-utility-nav']").evaluate((node) => {
        const rect = node.getBoundingClientRect();
        const sidebarRect = node.closest("aside")?.getBoundingClientRect();
        const style = getComputedStyle(node);
        const buttons = Array.from(node.querySelectorAll("button")).map((button) => {
          const item = button.getBoundingClientRect();
          return {
            width: Math.round(item.width),
            left: sidebarRect ? Math.round(item.left - sidebarRect.left) : 0,
          };
        });
        const transitions = Array.from(node.querySelectorAll("button")).map((button) => getComputedStyle(button).transitionProperty);
        return {
          height: Math.round(rect.height),
          borderTop: style.borderTopColor,
          paddingTop: Math.round(Number.parseFloat(style.paddingTop)),
          bottomGap: sidebarRect ? Math.round(sidebarRect.bottom - rect.bottom) : 0,
          buttons,
          transitions,
        };
      });
      expect(utilityMetrics.height).toBeLessThanOrEqual(42);
      expect(utilityMetrics.borderTop).toBe("rgb(58, 53, 44)");
      expect(utilityMetrics.paddingTop).toBeGreaterThanOrEqual(6);
      expect(utilityMetrics.bottomGap).toBeGreaterThanOrEqual(8);
      expect(utilityMetrics.buttons.map((button) => button.width)).toEqual([28, 28, 28]);
      expect(utilityMetrics.buttons[0].left).toBeGreaterThanOrEqual(8);
      expect(railMetrics.primaryTransitions.every((value) => value === "all" || value.includes("background-color"))).toBe(true);
      expect(utilityMetrics.transitions.every((value) => value === "all" || value.includes("background-color"))).toBe(true);
  
      const searchAction = sidebar.getByRole("button", { name: "搜索" });
      await searchAction.hover();
      await expect(searchAction).not.toHaveCSS("border-color", "rgba(0, 0, 0, 0)");
      const pluginsAction = sidebar.getByRole("button", { name: "插件" });
      await pluginsAction.hover();
      await expect(pluginsAction).not.toHaveCSS("border-color", "rgba(0, 0, 0, 0)");
  
      await sidebar.getByRole("button", { name: "插件" }).click();
      const drawer = page.getByRole("complementary", { name: "插件" });
      await expect(drawer.getByText("插件", { exact: true }).first()).toBeVisible();
      await expect(drawer.getByRole("tab", { name: /插件/ })).toHaveAttribute("aria-selected", "true");
      await expect(drawer.getByRole("textbox", { name: "搜索插件" })).toBeVisible();
      await expect(drawer.getByTestId("capability-drawer-header")).toHaveCSS("height", "42px");
      await expect(page.getByTestId("capability-drawer-surface")).toHaveCSS("width", "320px");
      await expect.poll(async () => {
        const box = await drawer.boundingBox();
        return box ? { x: Math.round(box.x), width: Math.round(box.width) } : null;
      }).toEqual({ x: Math.round(width), width: 320 });
      const drawerMaterial = await page.getByTestId("capability-drawer-surface").evaluate((node) => {
        const root = document.documentElement;
        const style = getComputedStyle(node);
        return {
          token: getComputedStyle(root).getPropertyValue("--forge-sidebar-width").trim(),
          backdrop: style.backdropFilter || style.webkitBackdropFilter,
          background: style.backgroundColor,
        };
      });
      expect(drawerMaterial.token).toBe("216px");
      expect(drawerMaterial.backdrop).toBe("none");
      expect(drawerMaterial.background).toBe("rgba(42, 39, 33, 0.97)");
      await expect(drawer.getByTestId("forge-icon-action").first()).toBeVisible();
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
      const longTitle = "Build a compact scanner with an extremely long workspace path and confirmation command preview";
      await page.locator("textarea").fill(longTitle);
      await page.locator("textarea").press("Enter");
  
      const sidebar = page.locator("aside").first();
      const row = sidebar.locator(".forge-sidebar-history-row").first();
      await expect(row).toBeVisible();
  
      const metrics = await row.evaluate((node) => {
        const root = document.documentElement;
        const style = getComputedStyle(node);
        const indicatorStyle = getComputedStyle(node, "::before");
        const label = node.querySelector("span");
        const deleteButton = node.querySelector("button");
        const list = node.closest(".forge-sidebar-history-list");
        const group = node.closest(".forge-sidebar-history-group");
        const groupLabel = document.querySelector(".forge-sidebar-history-group-label");
        return {
          token: getComputedStyle(root).getPropertyValue("--forge-sidebar-row-height").trim(),
          height: Math.round(node.getBoundingClientRect().height),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          labelInset: label ? Math.round(label.getBoundingClientRect().left - node.getBoundingClientRect().left) : 0,
          indicatorContent: indicatorStyle.content,
          indicatorWidth: indicatorStyle.width,
          borderColor: style.borderTopColor,
          background: style.backgroundColor,
          deleteOpacity: deleteButton ? getComputedStyle(deleteButton).opacity : null,
          listDisplay: list ? getComputedStyle(list).display : "",
          rowClientWidth: node.clientWidth,
          rowScrollWidth: node.scrollWidth,
          groupClientWidth: group?.clientWidth ?? 0,
          groupScrollWidth: group?.scrollWidth ?? 0,
          groupOverflowX: group ? getComputedStyle(group).overflowX : "",
          groupLabelHeight: groupLabel ? Math.round(groupLabel.getBoundingClientRect().height) : 0,
        };
      });
  
      expect(metrics.token).toBe("28px");
      expect(metrics.height).toBe(28);
      expect(metrics.radius).toBeLessThanOrEqual(8);
      expect(metrics.labelInset).toBeGreaterThanOrEqual(10);
      expect(metrics.indicatorContent).toBe("none");
      expect(metrics.indicatorWidth).toBe("auto");
      expect(metrics.borderColor).toBe("rgb(216, 201, 184)");
      expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
      expect(metrics.deleteOpacity).toBe("0");
      expect(metrics.listDisplay).toBe("flex");
      expect(metrics.rowScrollWidth).toBeLessThanOrEqual(metrics.rowClientWidth + 1);
      expect(metrics.groupScrollWidth).toBeLessThanOrEqual(metrics.groupClientWidth + 1);
      expect(metrics.groupOverflowX).toBe("hidden");
      expect(metrics.groupLabelHeight).toBeGreaterThanOrEqual(20);
    });

    test("command palette shows compact desktop shortcuts", async ({ page }) => {
      await page.keyboard.down("Control");
      await page.keyboard.press("k");
      await page.keyboard.up("Control");
  
      const palette = page.getByRole("dialog");
      await expect(palette.getByTestId("command-palette-surface")).toBeVisible();
      const paletteMetrics = await palette.evaluate((node) => {
        const motionRoot = node.querySelector<HTMLElement>(".forge-command-motion-root");
        const surface = node.querySelector<HTMLElement>("[data-testid='command-palette-surface']");
        const input = node.querySelector<HTMLElement>("[data-slot='command-input-wrapper']");
        const inputControl = node.querySelector<HTMLElement>("[data-slot='command-input']");
        const item = node.querySelector<HTMLElement>("[data-slot='command-item']");
        const shortcut = node.querySelector<HTMLElement>("[data-testid='command-shortcut']");
        const style = surface ? getComputedStyle(surface) : null;
        const inputStyle = inputControl ? getComputedStyle(inputControl) : null;
        const motionStyle = motionRoot ? getComputedStyle(motionRoot) : null;
        return {
          width: Math.round(node.getBoundingClientRect().width),
          motionRootWidth: motionRoot ? Math.round(motionRoot.getBoundingClientRect().width) : 0,
          motionWillChange: motionStyle?.willChange ?? "",
          motionEntryCount: node.querySelectorAll("[data-forge-motion='command-entry']").length,
          radius: style ? Number.parseFloat(style.borderTopLeftRadius) : 0,
          inputHeight: input ? Math.round(input.getBoundingClientRect().height) : 0,
          inputBackground: inputStyle?.backgroundColor ?? "",
          inputOutline: inputStyle?.outlineStyle ?? "",
          itemHeight: item ? Math.round(item.getBoundingClientRect().height) : 0,
          shortcutRadius: shortcut ? Number.parseFloat(getComputedStyle(shortcut).borderTopLeftRadius) : 0,
        };
      });
      expect(paletteMetrics.width).toBeGreaterThanOrEqual(540);
      expect(paletteMetrics.width).toBeLessThanOrEqual(600);
      expect(paletteMetrics.motionRootWidth).toBeGreaterThanOrEqual(540);
      expect(paletteMetrics.motionWillChange).toContain("transform");
      expect(paletteMetrics.motionEntryCount).toBeGreaterThanOrEqual(3);
      expect(paletteMetrics.radius).toBeLessThanOrEqual(8);
      expect(paletteMetrics.inputHeight).toBeLessThanOrEqual(42);
      expect(paletteMetrics.inputBackground).toBe("rgba(0, 0, 0, 0)");
      expect(paletteMetrics.inputOutline).toBe("none");
      expect(paletteMetrics.itemHeight).toBeLessThanOrEqual(34);
      expect(paletteMetrics.shortcutRadius).toBeLessThanOrEqual(6);
      await expect(palette.getByRole("option", { name: /新建对话/ })).toContainText("⌘N");
      await expect(palette.getByRole("option", { name: /设置/ })).toContainText("⌘,");
  
      await page.keyboard.press("Escape");
      await page.keyboard.down("Control");
      await page.keyboard.press(",");
      await page.keyboard.up("Control");
      await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    });
});
