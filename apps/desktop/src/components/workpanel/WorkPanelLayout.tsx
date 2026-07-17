import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Group, Panel, Separator, usePanelRef } from "react-resizable-panels";
import { WorkPanelShell } from "./WorkPanelShell";
import { closeWorkPanelTab, focusWorkPanelTab, openWorkPanelLauncher, openWorkPanelTab, restoreTaskPanelState } from "./workPanelState";
import { loadWorkPanelTasks, saveWorkPanelTask } from "./workPanelPersistence";
import type { WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes";

const WIDTH_STORAGE_KEY = "forge-work-panel-width-v1";
const DEFAULT_PANEL_WIDTH = 45;

interface WorkPanelLayoutProps {
  children: ReactNode;
  taskKey: string;
  taskLabel: string;
}

export function WorkPanelLayout({ children, taskKey, taskLabel }: WorkPanelLayoutProps) {
  const panelRef = usePanelRef();
  const lastWidthRef = useRef(readStoredWidth());
  const [open, setOpen] = useState(false);
  const [maximized, setMaximized] = useState(false);
  const [taskStates, setTaskStates] = useState<Record<string, WorkPanelTaskState>>(() => {
    if (typeof window === "undefined") return {};
    return loadWorkPanelTasks(window.localStorage);
  });
  const state = taskStates[taskKey] ?? restoreTaskPanelState(null);

  const updateState = useCallback((update: (current: WorkPanelTaskState) => WorkPanelTaskState) => {
    setTaskStates((current) => {
      const nextTaskState = update(current[taskKey] ?? restoreTaskPanelState(null));
      if (typeof window !== "undefined") saveWorkPanelTask(window.localStorage, taskKey, nextTaskState);
      return { ...current, [taskKey]: nextTaskState };
    });
  }, [taskKey]);

  useEffect(() => {
    const toggle = () => setOpen((current) => !current);
    const show = () => setOpen(true);
    window.addEventListener("toggle-work-panel", toggle);
    window.addEventListener("open-work-panel", show);
    return () => {
      window.removeEventListener("toggle-work-panel", toggle);
      window.removeEventListener("open-work-panel", show);
    };
  }, []);

  const toggleMaximize = () => {
    if (maximized) {
      panelRef.current?.resize(`${lastWidthRef.current}%`);
      setMaximized(false);
      return;
    }
    const currentWidth = panelRef.current?.getSize().asPercentage;
    if (currentWidth) lastWidthRef.current = currentWidth;
    setMaximized(true);
    requestAnimationFrame(() => panelRef.current?.resize("100%"));
  };

  return (
    <Group orientation="horizontal" className="forge-work-panel-layout">
      <Panel id="conversation" minSize={maximized ? "0%" : "35%"} groupResizeBehavior="preserve-relative-size">
        {children}
      </Panel>
      {open ? (
        <Separator
          id="work-panel-separator"
          className="forge-work-panel-separator"
          aria-label="调整工作面板宽度"
        />
      ) : null}
      {open ? (
        <Panel
          id="work-panel"
          panelRef={panelRef}
          defaultSize={`${lastWidthRef.current}%`}
          minSize={maximized ? "100%" : "30%"}
          maxSize={maximized ? "100%" : "70%"}
          groupResizeBehavior="preserve-relative-size"
          onResize={(size) => {
            if (maximized) return;
            lastWidthRef.current = size.asPercentage;
            window.localStorage.setItem(WIDTH_STORAGE_KEY, String(size.asPercentage));
          }}
        >
          <WorkPanelShell
            maximized={maximized}
            state={state}
            taskKey={taskKey}
            taskLabel={taskLabel}
            onClose={() => setOpen(false)}
            onCloseTab={(tabId) => updateState((current) => closeWorkPanelTab(current, tabId))}
            onFocusTab={(tabId) => updateState((current) => focusWorkPanelTab(current, tabId))}
            onOpenLauncher={() => updateState(openWorkPanelLauncher)}
            onOpenTab={(tab: WorkPanelTab) => updateState((current) => openWorkPanelTab(current, tab))}
            onToggleMaximize={toggleMaximize}
          />
        </Panel>
      ) : null}
    </Group>
  );
}

function readStoredWidth() {
  if (typeof window === "undefined") return DEFAULT_PANEL_WIDTH;
  const width = Number.parseFloat(window.localStorage.getItem(WIDTH_STORAGE_KEY) ?? "");
  return Number.isFinite(width) && width >= 30 && width <= 70 ? width : DEFAULT_PANEL_WIDTH;
}
