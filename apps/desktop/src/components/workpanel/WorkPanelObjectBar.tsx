import { Maximize2, Minimize2, MoreHorizontal, Plus, X } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { WorkPanelTab } from "./workPanelTypes";

interface WorkPanelObjectBarProps {
  activeTabId: string | null;
  maximized: boolean;
  tabs: WorkPanelTab[];
  onClose: () => void;
  onCloseTab: (tabId: string) => void;
  onFocusTab: (tabId: string) => void;
  onOpenLauncher: () => void;
  onToggleMaximize: () => void;
}

export function WorkPanelObjectBar({
  activeTabId,
  maximized,
  tabs,
  onClose,
  onCloseTab,
  onFocusTab,
  onOpenLauncher,
  onToggleMaximize,
}: WorkPanelObjectBarProps) {
  return (
    <div className="forge-work-panel-object-bar">
      <TabsList variant="line" aria-label="已打开的工作内容" className="forge-work-panel-tab-list">
        {tabs.map((tab) => (
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

      <DropdownMenu>
        <DropdownMenuTrigger className="forge-work-panel-object-action" aria-label="更多已打开内容" title="更多已打开内容">
          <MoreHorizontal className="size-4" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="forge-work-panel-overflow-menu">
          {tabs.map((tab) => (
            <DropdownMenuCheckboxItem key={tab.id} checked={tab.id === activeTabId} onClick={() => onFocusTab(tab.id)}>
              {tab.label}
            </DropdownMenuCheckboxItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <ForgeIconButton aria-label="新建工作面板标签" title="新建工作面板标签" onClick={onOpenLauncher}>
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
  );
}
