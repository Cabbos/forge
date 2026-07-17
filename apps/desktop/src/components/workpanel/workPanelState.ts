import type { WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes.ts";
import { normalizeWorkPanelWidthPercent } from "./workPanelDimensions.ts";

const EMPTY_WORK_PANEL_STATE: WorkPanelTaskState = {
  tabs: [],
  activeTabId: null,
  launcherOpen: true,
  widthPercent: 40,
};

export function restoreTaskPanelState(state: WorkPanelTaskState | null | undefined): WorkPanelTaskState {
  if (!state || state.tabs.length === 0) return {
    ...EMPTY_WORK_PANEL_STATE,
    widthPercent: normalizeWorkPanelWidthPercent(state?.widthPercent),
  };

  const activeTabId = state.tabs.some((tab) => tab.id === state.activeTabId)
    ? state.activeTabId
    : state.tabs[0]?.id ?? null;

  return {
    tabs: [...state.tabs],
    activeTabId: state.launcherOpen ? null : activeTabId,
    launcherOpen: state.launcherOpen,
    widthPercent: normalizeWorkPanelWidthPercent(state.widthPercent),
  };
}

export function openWorkPanelTab(state: WorkPanelTaskState, tab: WorkPanelTab): WorkPanelTaskState {
  const existing = state.tabs.find((candidate) => candidate.id === tab.id);
  return {
    tabs: existing ? state.tabs : [...state.tabs, tab],
    activeTabId: tab.id,
    launcherOpen: false,
    widthPercent: state.widthPercent,
  };
}

export function focusWorkPanelTab(state: WorkPanelTaskState, tabId: string): WorkPanelTaskState {
  if (!state.tabs.some((tab) => tab.id === tabId)) return state;
  return {
    ...state,
    activeTabId: tabId,
    launcherOpen: false,
  };
}

export function openWorkPanelLauncher(state: WorkPanelTaskState): WorkPanelTaskState {
  return {
    ...state,
    activeTabId: null,
    launcherOpen: true,
  };
}

export function closeWorkPanelTab(state: WorkPanelTaskState, tabId: string): WorkPanelTaskState {
  const closingIndex = state.tabs.findIndex((tab) => tab.id === tabId);
  if (closingIndex < 0) return state;

  const tabs = state.tabs.filter((tab) => tab.id !== tabId);
  if (tabs.length === 0) return { ...EMPTY_WORK_PANEL_STATE, widthPercent: normalizeWorkPanelWidthPercent(state.widthPercent) };
  if (state.activeTabId !== tabId) return { ...state, tabs };

  const nextIndex = Math.min(closingIndex, tabs.length - 1);
  return {
    tabs,
    activeTabId: tabs[nextIndex]?.id ?? null,
    launcherOpen: false,
    widthPercent: state.widthPercent,
  };
}
