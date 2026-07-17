import { useRef } from "react";
import { Maximize2, Minimize2, X } from "lucide-react";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { Tabs, TabsContent } from "@/components/ui/tabs";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";
import { WorkPanelContent } from "./WorkPanelContent";
import { WorkPanelLauncher } from "./WorkPanelLauncher";
import { WorkPanelObjectBar } from "./WorkPanelObjectBar";
import type { WorkPanelTab, WorkPanelTaskState } from "./workPanelTypes";
import type { WorkPanelViewportMode } from "./workPanelDimensions";

interface WorkPanelShellProps {
  maximized: boolean;
  state: WorkPanelTaskState;
  taskKey: string;
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
  viewportMode,
  onClose,
  onCloseTab,
  onFocusTab,
  onOpenLauncher,
  onOpenTab,
  onToggleMaximize,
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
        duration: 0.18,
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
        duration: 0.16,
        ease: forgeMotion.evidence.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: panelRef, dependencies: [selectedValue] });

  return (
    <aside ref={panelRef} className="forge-work-panel" role="complementary" aria-label="工作面板" data-testid="work-panel" data-viewport-mode={viewportMode} data-width-percent={state.widthPercent}>
      <Tabs
        value={selectedValue}
        onValueChange={(value) => { if (typeof value === "string") onFocusTab(value); }}
        className="forge-work-panel-tabs"
      >
        {state.launcherOpen ? (
          <>
            <div className="forge-work-panel-launcher-utilities">
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
            <WorkPanelLauncher mode={state.tabs.length === 0 ? "empty" : "new"} taskKey={taskKey} onOpenTab={onOpenTab} />
          </>
        ) : (
          <>
            <WorkPanelObjectBar
              activeTabId={state.activeTabId}
              maximized={maximized}
              tabs={state.tabs}
              onClose={onClose}
              onCloseTab={onCloseTab}
              onFocusTab={onFocusTab}
              onOpenLauncher={onOpenLauncher}
              onToggleMaximize={onToggleMaximize}
            />
            {state.tabs.map((tab) => (
              <TabsContent key={tab.id} value={tab.id} className="forge-work-panel-tab-content">
                <WorkPanelContent tab={tab} onOpenTab={onOpenTab} />
              </TabsContent>
            ))}
          </>
        )}
      </Tabs>
    </aside>
  );
}
