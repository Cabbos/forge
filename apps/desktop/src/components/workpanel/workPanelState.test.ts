import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  closeWorkPanelTab,
  focusWorkPanelTab,
  openWorkPanelLauncher,
  openWorkPanelTab,
  restoreTaskPanelState,
} from "./workPanelState.ts";
import {
  loadWorkPanelTasks,
  saveWorkPanelTask,
  WORK_PANEL_STORAGE_KEY,
} from "./workPanelPersistence.ts";
import type { WorkPanelTaskState } from "./workPanelTypes.ts";

describe("work panel tab state", () => {
  it("starts on the launcher without fabricating a tab", () => {
    assert.deepEqual(restoreTaskPanelState(null), {
      tabs: [],
      activeTabId: null,
      launcherOpen: true,
      widthPercent: 40,
    });
  });

  it("focuses an existing object instead of opening a duplicate", () => {
    const initial = restoreTaskPanelState(null);
    const first = openWorkPanelTab(initial, {
      kind: "file",
      id: "file:README.md",
      label: "README.md",
      path: "README.md",
    });
    const second = openWorkPanelTab(first, {
      kind: "file",
      id: "file:README.md",
      label: "README.md",
      path: "README.md",
    });

    assert.equal(second.tabs.length, 1);
    assert.equal(second.activeTabId, "file:README.md");
    assert.equal(second.launcherOpen, false);
  });

  it("focuses tabs and opens a transient launcher without deleting tabs", () => {
    const withFile = openWorkPanelTab(restoreTaskPanelState(null), {
      kind: "file",
      id: "file:README.md",
      label: "README.md",
      path: "README.md",
    });
    const withReview = openWorkPanelTab(withFile, {
      kind: "review",
      id: "review:task-1",
      label: "审阅 · 当前改动",
      taskId: "task-1",
    });

    assert.equal(focusWorkPanelTab(withReview, "file:README.md").activeTabId, "file:README.md");
    assert.deepEqual(openWorkPanelLauncher(withReview), {
      ...withReview,
      activeTabId: null,
      launcherOpen: true,
    });
  });

  it("selects the nearest tab after close and returns to launcher after final close", () => {
    const first = openWorkPanelTab(restoreTaskPanelState(null), {
      kind: "file",
      id: "file:a.ts",
      label: "a.ts",
      path: "a.ts",
    });
    const second = openWorkPanelTab(first, {
      kind: "file",
      id: "file:b.ts",
      label: "b.ts",
      path: "b.ts",
    });
    const afterSecondClose = closeWorkPanelTab(second, "file:b.ts");

    assert.equal(afterSecondClose.activeTabId, "file:a.ts");
    assert.equal(afterSecondClose.launcherOpen, false);
    assert.deepEqual(closeWorkPanelTab({ ...afterSecondClose, widthPercent: 52 }, "file:a.ts"), {
      tabs: [],
      activeTabId: null,
      launcherOpen: true,
      widthPercent: 52,
    });
  });
});

describe("work panel persistence", () => {
  it("loads only valid task and tab records", () => {
    const storage = memoryStorage({
      [WORK_PANEL_STORAGE_KEY]: JSON.stringify({
        version: 1,
        tasks: {
          "task-1": {
            tabs: [
              { kind: "file", id: "file:README.md", label: "README.md", path: "README.md" },
              { kind: "unknown", id: "bad", label: "Bad" },
            ],
            activeTabId: "bad",
            launcherOpen: false,
          },
          "": { tabs: [], activeTabId: null, launcherOpen: true },
        },
      }),
    });

    assert.deepEqual(loadWorkPanelTasks(storage), {
      "task-1": {
        tabs: [{ kind: "file", id: "file:README.md", label: "README.md", path: "README.md" }],
        activeTabId: "file:README.md",
        launcherOpen: false,
        widthPercent: 40,
      },
    });
  });

  it("migrates v1 task records with the default width", () => {
    const storage = memoryStorage({
      [WORK_PANEL_STORAGE_KEY]: JSON.stringify({
        version: 1,
        tasks: {
          "task-1": { tabs: [], activeTabId: null, launcherOpen: true },
        },
      }),
    });

    assert.equal(loadWorkPanelTasks(storage)["task-1"]?.widthPercent, 40);
  });

  it("falls back safely for malformed JSON and saves one task without losing siblings", () => {
    const storage = memoryStorage({ [WORK_PANEL_STORAGE_KEY]: "not-json" });
    assert.deepEqual(loadWorkPanelTasks(storage), {});

    const task: WorkPanelTaskState = {
      tabs: [{ kind: "terminal", id: "terminal:task-1", label: "终端", taskId: "task-1" }],
      activeTabId: "terminal:task-1",
      launcherOpen: false,
      widthPercent: 40,
    };
    saveWorkPanelTask(storage, "task-1", task);
    saveWorkPanelTask(storage, "task-2", restoreTaskPanelState(null));

    assert.deepEqual(Object.keys(loadWorkPanelTasks(storage)), ["task-1", "task-2"]);
  });
});

function memoryStorage(initial: Record<string, string> = {}): Storage {
  const values = new Map(Object.entries(initial));
  return {
    get length() {
      return values.size;
    },
    clear() {
      values.clear();
    },
    getItem(key) {
      return values.get(key) ?? null;
    },
    key(index) {
      return [...values.keys()][index] ?? null;
    },
    removeItem(key) {
      values.delete(key);
    },
    setItem(key, value) {
      values.set(key, value);
    },
  };
}
