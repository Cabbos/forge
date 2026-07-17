import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Group, Panel, Separator, usePanelRef } from "react-resizable-panels";
import { WorkPanelShell } from "./WorkPanelShell";
import { closeWorkPanelTab, focusWorkPanelTab, openWorkPanelLauncher, openWorkPanelTab, restoreTaskPanelState } from "./workPanelState";
import { clampWorkPanelWidthPercent, getWorkbenchWidth, getWorkPanelBounds, getWorkPanelViewportMode, MIN_WORK_PANEL_WIDTH_PX, normalizeWorkPanelWidthPercent } from "./workPanelDimensions";
import { loadWorkPanelTasks, saveWorkPanelTask } from "./workPanelPersistence";
import type { WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes";

interface WorkPanelLayoutProps {
  children: ReactNode;
  taskKey: string;
}

export function WorkPanelLayout({ children, taskKey }: WorkPanelLayoutProps) {
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
  const modeRef = useRef(mode);
  const suppressLayoutPersistenceRef = useRef(false);
  const restoreWidthAfterOverlayRef = useRef(false);

  const updateState = useCallback((update: (current: WorkPanelTaskState) => WorkPanelTaskState, persist = true) => {
    setTaskStates((current) => {
      const nextTaskState = update(current[taskKey] ?? restoreTaskPanelState(null));
      if (persist && typeof window !== "undefined") saveWorkPanelTask(window.localStorage, taskKey, nextTaskState);
      return { ...current, [taskKey]: nextTaskState };
    });
  }, [taskKey]);

  const setWidthPercent = useCallback((widthPercent: number) => {
    const target = mode === "split"
      ? clampWorkPanelWidthPercent(widthPercent, bounds)
      : normalizeWorkPanelWidthPercent(widthPercent);
    updateState((current) => ({ ...current, widthPercent: target }), false);
    if (mode === "split") panelRef.current?.resize(`${target}%`);
  }, [bounds, mode, panelRef, updateState]);

  useEffect(() => {
    const toggle = () => setOpen((current) => !current);
    const show = () => setOpen(true);
    const resize = () => {
      const nextMode = getWorkPanelViewportMode(window.innerWidth);
      if (modeRef.current === "overlay" && nextMode === "split") {
        suppressLayoutPersistenceRef.current = true;
        restoreWidthAfterOverlayRef.current = true;
      }
      setViewportWidth(window.innerWidth);
    };
    window.addEventListener("toggle-work-panel", toggle);
    window.addEventListener("open-work-panel", show);
    window.addEventListener("resize", resize);
    return () => {
      window.removeEventListener("toggle-work-panel", toggle);
      window.removeEventListener("open-work-panel", show);
      window.removeEventListener("resize", resize);
    };
  }, []);

  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);

  useEffect(() => {
    if (!restoreWidthAfterOverlayRef.current || mode !== "split" || !open || maximized) return;
    const target = clampWorkPanelWidthPercent(state.widthPercent, bounds);
    const restore = requestAnimationFrame(() => {
      panelRef.current?.resize(`${target}%`);
      requestAnimationFrame(() => {
        restoreWidthAfterOverlayRef.current = false;
        suppressLayoutPersistenceRef.current = false;
      });
    });
    return () => cancelAnimationFrame(restore);
  }, [bounds, maximized, mode, open, panelRef, state.widthPercent]);

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
  // The overlay itself is fixed, so it must not consume PanelGroup width and hide the workbench.
  const panelSize = isSplit ? `${state.widthPercent}%` : mode === "fixed" ? `${MIN_WORK_PANEL_WIDTH_PX}px` : "0%";

  return (
    <Group
      orientation="horizontal"
      className="forge-work-panel-layout"
      data-viewport-mode={mode}
      onLayoutChanged={(layout) => {
        const widthPercent = layout["work-panel"];
        if (!isSplit || maximized || suppressLayoutPersistenceRef.current || typeof widthPercent !== "number") return;
        updateState((current) => ({ ...current, widthPercent: clampWorkPanelWidthPercent(widthPercent, bounds) }));
      }}
    >
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
          minSize={maximized ? "100%" : isSplit ? `${bounds.min}%` : mode === "fixed" ? `${MIN_WORK_PANEL_WIDTH_PX}px` : "0%"}
          maxSize={maximized ? "100%" : isSplit ? `${bounds.max}%` : mode === "fixed" ? `${MIN_WORK_PANEL_WIDTH_PX}px` : "0%"}
          groupResizeBehavior="preserve-relative-size"
        >
          <WorkPanelShell
            maximized={maximized}
            state={state}
            taskKey={taskKey}
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
