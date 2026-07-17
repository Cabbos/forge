import { useRef } from "react";
import { Maximize2, Minimize2, Minus, Plus, X } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";
import { WorkPanelContent } from "./WorkPanelContent";
import { WorkPanelLauncher } from "./WorkPanelLauncher";
import type { WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes";
import type { WorkPanelViewportMode } from "./workPanelDimensions";

interface WorkPanelShellProps {
  maximized: boolean;
  state: WorkPanelTaskState;
  taskKey: string;
  taskLabel: string;
  viewportMode: WorkPanelViewportMode;
  onClose: () => void;
  onCloseTab: (tabId: string) => void;
  onFocusTab: (tabId: string) => void;
  onOpenLauncher: () => void;
  onOpenTab: (tab: WorkPanelTab) => void;
  onToggleMaximize: () => void;
  onDecreaseWidth: () => void;
  onIncreaseWidth: () => void;
}

export function WorkPanelShell({
  maximized,
  state,
  taskKey,
  taskLabel,
  viewportMode,
  onClose,
  onCloseTab,
  onFocusTab,
  onOpenLauncher,
  onOpenTab,
  onToggleMaximize,
  onDecreaseWidth,
  onIncreaseWidth,
}: WorkPanelShellProps) {
  const panelRef = useRef<HTMLElement>(null);
  const selectedValue = state.launcherOpen ? null : state.activeTabId;

  useGSAP(() => {
    const panel = panelRef.current;
    if (!panel || prefersReducedMotion()) return;
    gsap.fromTo(
      panel,
      { autoAlpha: 0, x: 12 },
      {
        autoAlpha: 1,
        x: 0,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: panelRef });

  useGSAP(() => {
    const panel = panelRef.current;
    if (!panel || prefersReducedMotion()) return;
    const content = panel.querySelector<HTMLElement>(
      ".forge-work-panel-launcher, .forge-work-panel-tab-content:not([hidden])",
    );
    if (!content) return;
    gsap.fromTo(
      content,
      { autoAlpha: 0, y: 3 },
      {
        autoAlpha: 1,
        y: 0,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: panelRef, dependencies: [selectedValue] });

  return (
    <aside ref={panelRef} className="forge-work-panel" role="complementary" aria-label="工作面板" data-testid="work-panel" data-viewport-mode={viewportMode} data-width-percent={state.widthPercent}>
      <header className="forge-work-panel-header">
        <div className="forge-work-panel-heading">
          <span className="forge-work-panel-title">工作面板</span>
          <span className="forge-work-panel-task" title={taskLabel}>{taskLabel}</span>
        </div>
        <div className="forge-work-panel-header-actions">
          <ForgeIconButton aria-label="缩小工作面板" title="缩小工作面板" onClick={onDecreaseWidth}>
            <Minus className="size-4" />
          </ForgeIconButton>
          <ForgeIconButton aria-label="扩大工作面板" title="扩大工作面板" onClick={onIncreaseWidth}>
            <Plus className="size-4" />
          </ForgeIconButton>
          <ForgeIconButton
            aria-label={maximized ? "恢复工作面板宽度" : "最大化工作面板"}
            title={maximized ? "恢复工作面板宽度" : "最大化工作面板"}
            onClick={onToggleMaximize}
          >
            {maximized ? <Minimize2 className="size-4" /> : <Maximize2 className="size-4" />}
          </ForgeIconButton>
          <ForgeIconButton aria-label="关闭工作面板" title="关闭工作面板" onClick={onClose}>
            <X className="size-4" />
          </ForgeIconButton>
        </div>
      </header>

      <Tabs
        value={selectedValue}
        onValueChange={(value) => { if (typeof value === "string") onFocusTab(value); }}
        className="forge-work-panel-tabs"
      >
        <div className="forge-work-panel-tab-rail">
          <TabsList variant="line" aria-label="已打开的工作内容" className="forge-work-panel-tab-list">
            {state.tabs.map((tab) => (
              <div key={tab.id} className="forge-work-panel-tab-wrap">
                <TabsTrigger value={tab.id} className="forge-work-panel-tab" title={tab.label}>
                  <span>{tab.label}</span>
                </TabsTrigger>
                <ButtonPrimitive
                  type="button"
                  className="forge-work-panel-tab-close"
                  aria-label={`关闭 ${tab.label}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    onCloseTab(tab.id);
                  }}
                >
                  <X className="size-3" />
                </ButtonPrimitive>
              </div>
            ))}
          </TabsList>
          <ButtonPrimitive type="button" className="forge-work-panel-new-tab" aria-label="新建工作面板标签" onClick={onOpenLauncher}>
            <Plus className="size-4" />
          </ButtonPrimitive>
        </div>

        {state.launcherOpen ? (
          <WorkPanelLauncher taskKey={taskKey} onOpenTab={onOpenTab} />
        ) : (
          state.tabs.map((tab) => (
            <TabsContent key={tab.id} value={tab.id} className="forge-work-panel-tab-content">
              <WorkPanelContent tab={tab} onOpenTab={onOpenTab} />
            </TabsContent>
          ))
        )}
      </Tabs>
    </aside>
  );
}
