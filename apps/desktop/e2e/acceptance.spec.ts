import { test, expect } from "@playwright/test";
import { setup } from "./fixtures/app";

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
    await expect(task).toContainText("完成");

    await task.getByRole("button", { name: "禁用" }).click();
    await expect(task).toContainText("已禁用");

    await task.getByRole("button", { name: "删除" }).click();
    await expect(dialog.locator(".forge-scheduler-task-card", { hasText: "Daily acceptance check" })).toHaveCount(0);
  });
});
