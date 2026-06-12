import type { RefObject } from "react";
import { X } from "lucide-react";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { HubPanelContent, type HubPanelContentProps } from "./HubPanelContent";

interface HubPanelShellProps extends HubPanelContentProps {
  panelRef: RefObject<HTMLElement>;
  onClose: () => void;
}

export function HubPanelShell({
  panelRef,
  onClose,
  ...contentProps
}: HubPanelShellProps) {
  return (
    <aside
      ref={panelRef}
      data-testid="project-archive-panel"
      aria-label="项目档案"
      data-forge-motion="archive-panel"
      className="forge-inspector fixed right-0 top-0 z-50 flex h-full flex-col overflow-hidden"
    >
      <div className="forge-inspector-header">
        <div className="forge-inspector-title-block">
          <span className="forge-inspector-title">项目档案</span>
          <span className="forge-inspector-subtitle">状态、子任务、上下文与交付</span>
        </div>
        <ForgeIconButton
          aria-label="关闭项目档案"
          onClick={onClose}
          title="关闭项目档案"
        >
          <X className="size-4" />
        </ForgeIconButton>
      </div>

      <HubPanelContent {...contentProps} />
    </aside>
  );
}
