import { restoreTaskPanelState } from "./workPanelState.ts";
import type { PreviewTarget, WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes.ts";

export const WORK_PANEL_STORAGE_KEY = "forge-work-panel-v1";

export interface WorkPanelStorage {
  version: 2;
  tasks: Record<string, WorkPanelTaskState>;
}

type StorageLike = Pick<Storage, "getItem" | "setItem">;

export function loadWorkPanelTasks(storage: StorageLike): Record<string, WorkPanelTaskState> {
  const raw = storage.getItem(WORK_PANEL_STORAGE_KEY);
  if (!raw) return {};

  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed) || (parsed.version !== 1 && parsed.version !== 2) || !isRecord(parsed.tasks)) return {};

    return Object.entries(parsed.tasks).reduce<Record<string, WorkPanelTaskState>>((tasks, [key, value]) => {
      if (!key.trim() || !isRecord(value) || !Array.isArray(value.tabs)) return tasks;
      const tabs = value.tabs.filter(isWorkPanelTab);
      const requestedActiveId = typeof value.activeTabId === "string" ? value.activeTabId : null;
      const launcherOpen = value.launcherOpen === true;
      const widthPercent = parsed.version === 1 ? 40 : typeof value.widthPercent === "number" ? value.widthPercent : 40;
      tasks[key] = restoreTaskPanelState({ tabs, activeTabId: requestedActiveId, launcherOpen, widthPercent: widthPercent ?? 40 });
      return tasks;
    }, {});
  } catch {
    return {};
  }
}

export function saveWorkPanelTask(storage: StorageLike, taskKey: string, state: WorkPanelTaskState): void {
  if (!taskKey.trim()) return;
  const tasks = loadWorkPanelTasks(storage);
  tasks[taskKey] = restoreTaskPanelState(state);
  storage.setItem(WORK_PANEL_STORAGE_KEY, JSON.stringify({ version: 2, tasks } satisfies WorkPanelStorage));
}

function isWorkPanelTab(value: unknown): value is WorkPanelTab {
  if (!isRecord(value) || !nonEmptyString(value.id) || !nonEmptyString(value.label)) return false;

  switch (value.kind) {
    case "review":
    case "terminal":
      return nonEmptyString(value.taskId);
    case "preview":
      return isPreviewTarget(value.target);
    case "file":
      return nonEmptyString(value.path);
    case "subtask":
      return nonEmptyString(value.taskId) && nonEmptyString(value.subtaskId);
    default:
      return false;
  }
}

function isPreviewTarget(value: unknown): value is PreviewTarget {
  if (!isRecord(value)) return false;
  if (value.type === "url") return nonEmptyString(value.url);
  if (value.type === "file") return nonEmptyString(value.path);
  return false;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}
