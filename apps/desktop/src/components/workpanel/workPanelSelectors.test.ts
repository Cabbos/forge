import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  WORK_PANEL_LAUNCHER_ACTIONS,
  createFileTab,
  createPreviewUrlTab,
  createSubtaskTab,
  isAllowedPreviewUrl,
} from "./workPanelSelectors.ts";

describe("work panel launcher selectors", () => {
  it("keeps the agreed launcher order and labels", () => {
    assert.deepEqual(
      WORK_PANEL_LAUNCHER_ACTIONS.map(({ id, label }) => [id, label]),
      [
        ["review", "审阅"],
        ["terminal", "终端"],
        ["preview", "预览网页"],
        ["files", "打开文件"],
        ["subtasks", "侧边任务"],
      ],
    );
  });

  it("creates stable object IDs instead of category tabs", () => {
    assert.equal(createFileTab("README.md").id, "file:README.md");
    assert.equal(createPreviewUrlTab("http://localhost:1420/")?.id, "preview:http://localhost:1420");
    assert.equal(createSubtaskTab("session-1", "task-7", "Runtime UI implementer").id, "subtask:task-7");
  });

  it("only embeds HTTP loopback URLs", () => {
    for (const value of [
      "http://localhost:1420",
      "https://127.0.0.1:3000/result",
      "http://[::1]:4173",
      "http://preview.localhost:8000",
    ]) {
      assert.equal(isAllowedPreviewUrl(value), true, value);
    }

    for (const value of [
      "file:///tmp/result.html",
      "javascript:alert(1)",
      "https://example.com",
      "http://localhost.example.com",
      "not a url",
    ]) {
      assert.equal(isAllowedPreviewUrl(value), false, value);
      assert.equal(createPreviewUrlTab(value), null, value);
    }
  });
});
