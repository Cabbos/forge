import { useCallback, useEffect, useState, type ReactNode } from "react";
import { Group, Panel, Separator, usePanelRef } from "react-resizable-panels";
import { WorkPanelShell } from "./WorkPanelShell";
import { closeWorkPanelTab, focusWorkPanelTab, openWorkPanelLauncher, openWorkPanelTab, restoreTaskPanelState } from "./workPanelState";
import { getWorkbenchWidth, getWorkPanelBounds, getWorkPanelViewportMode, MAX_WORK_PANEL_WIDTH_PX, MIN_WORK_PANEL_WIDTH_PX, normalizeWorkPanelWidthPercent } from "./workPanelDimensions";
import { loadWorkPanelTasks, saveWorkPanelTask } from "./workPanelPersistence";
import type { WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes";

interface WorkPanelLayoutProps {
  children: ReactNode;
  taskKey: string;
  taskLabel: string;
}

export function WorkPanelLayout({ children, taskKey, taskLabel }: WorkPanelLayoutProps) {
  const panelRef = usePanelRef();
  const [open, setOpen] = useState(false);
  const [maximized, setMaximized] = useState(false);
  const [viewportWidth, setViewportWidth] = useState(() => typeof window === "undefined" ? 1200 : window.innerWidth);
  const [taskStates, setTaskStates] = useState<Record<string, WorkPanelTaskState>>(() => {
    if (typeof window === "undefined") return {};
    return loadWorkPanelTasks(window.localStorage);
  });
  const state = taskStates[taskKey] ?? restoreTaskPanelState(null);
  const mode = getWorkPanelViewportMode(viewportWidth);
  const bounds = getWorkPanelBounds(getWorkbenchWidth(viewportWidth));

  const updateState = useCallback((update: (current: WorkPanelTaskState) => WorkPanelTaskState) => {
    setTaskStates((current) => {
      const nextTaskState = update(current[taskKey] ?? restoreTaskPanelState(null));
      if (typeof window !== "undefined") saveWorkPanelTask(window.localStorage, taskKey, nextTaskState);
      return { ...current, [taskKey]: nextTaskState };
    });
  }, [taskKey]);

  const setWidthPercent = useCallback((widthPercent: number) => {
    updateState((current) => ({ ...current, widthPercent: normalizeWorkPanelWidthPercent(widthPercent) }));
  }, [updateState]);

  useEffect(() => {
    const toggle = () => setOpen((current) => !current);
    const show = () => setOpen(true);
    const resize = () => setViewportWidth(window.innerWidth);
    window.addEventListener("toggle-work-panel", toggle);
    window.addEventListener("open-work-panel", show);
    window.addEventListener("resize", resize);
    return () => {
      window.removeEventListener("toggle-work-panel", toggle);
      window.removeEventListener("open-work-panel", show);
      window.removeEventListener("resize", resize);
    };
  }, []);

  const toggleMaximize = () => {
    if (maximized) {
      panelRef.current?.resize(`${state.widthPercent}%`);
      setMaximized(false);
      return;
    }
    setMaximized(true);
    requestAnimationFrame(() => panelRef.current?.resize("100%"));
  };

  const isSplit = mode === "split";
  const panelSize = isSplit ? `${state.widthPercent}%` : mode === "fixed" ? `${MIN_WORK_PANEL_WIDTH_PX}px` : "100%";

  return (
    <Group orientation="horizontal" className="forge-work-panel-layout" data-viewport-mode={mode}>
      <Panel id="conversation" minSize={mode === "overlay" || maximized ? "0%" : "35%"} groupResizeBehavior="preserve-relative-size">
        {children}
      </Panel>
      {open && mode !== "overlay" ? (
        <Separator
          id="work-panel-separator"
          className="forge-work-panel-separator"
          aria-label="调整工作面板宽度"
          onDoubleClick={() => setWidthPercent(40)}
        />
      ) : null}
      {open ? (
        <Panel
          id="work-panel"
          panelRef={panelRef}
          defaultSize={panelSize}
          minSize={maximized ? "100%" : isSplit ? `${bounds.min}%` : mode === "fixed" ? `${MIN_WORK_PANEL_WIDTH_PX}px` : "100%"}
          maxSize={maximized ? "100%" : isSplit ? `${bounds.max}%` : mode === "fixed" ? `${MAX_WORK_PANEL_WIDTH_PX}px` : "100%"}
          groupResizeBehavior="preserve-relative-size"
          onResize={(size) => {
            if (!maximized && isSplit) setWidthPercent(size.asPercentage);
          }}
        >
          <WorkPanelShell
            maximized={maximized}
            state={state}
            taskKey={taskKey}
            taskLabel={taskLabel}
            viewportMode={mode}
            onClose={() => setOpen(false)}
            onCloseTab={(tabId) => updateState((current) => closeWorkPanelTab(current, tabId))}
            onFocusTab={(tabId) => updateState((current) => focusWorkPanelTab(current, tabId))}
            onOpenLauncher={() => updateState(openWorkPanelLauncher)}
            onOpenTab={(tab: WorkPanelTab) => updateState((current) => openWorkPanelTab(current, tab))}
            onToggleMaximize={toggleMaximize}
            onDecreaseWidth={() => setWidthPercent(state.widthPercent - 2)}
            onIncreaseWidth={() => setWidthPercent(state.widthPercent + 2)}
          />
        </Panel>
      ) : null}
    </Group>
  );
}
