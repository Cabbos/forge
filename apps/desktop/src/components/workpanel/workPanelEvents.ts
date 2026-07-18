import type { WorkPanelTab } from "./workPanelTypes.ts";

export const OPEN_WORK_PANEL_TAB_EVENT = "open-work-panel-tab";

export function openWorkPanelTabInLayout(tab: WorkPanelTab) {
  window.dispatchEvent(new CustomEvent<WorkPanelTab>(OPEN_WORK_PANEL_TAB_EVENT, { detail: tab }));
}

export function workPanelTabFromEvent(event: Event): WorkPanelTab | null {
  if (!(event instanceof CustomEvent)) return null;
  return isWorkPanelTab(event.detail) ? event.detail : null;
}

function isWorkPanelTab(value: unknown): value is WorkPanelTab {
  if (!isRecord(value) || !nonEmptyString(value.id) || !nonEmptyString(value.label)) return false;

  switch (value.kind) {
    case "review":
    case "terminal":
      return nonEmptyString(value.taskId);
    case "preview":
      return isRecord(value.target)
        && ((value.target.type === "url" && nonEmptyString(value.target.url))
          || (value.target.type === "file" && nonEmptyString(value.target.path)));
    case "file":
      return nonEmptyString(value.path);
    case "subtask":
      return nonEmptyString(value.taskId) && nonEmptyString(value.subtaskId);
    default:
      return false;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}
