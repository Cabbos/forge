import { useState } from "react";
import { ArrowLeft, FileDiff, FolderOpen, Globe2, ListTree, TerminalSquare, type LucideIcon } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import type { WorkPanelLauncherAction, WorkPanelTab } from "./workPanelTypes";

const launcherActions: Array<{
  id: WorkPanelLauncherAction;
  label: string;
  shortcut?: string;
  icon: LucideIcon;
}> = [
  { id: "review", label: "审阅", shortcut: "⌃⇧G", icon: FileDiff },
  { id: "terminal", label: "终端", icon: TerminalSquare },
  { id: "preview", label: "预览", shortcut: "⌘T", icon: Globe2 },
  { id: "files", label: "文件", shortcut: "⌘P", icon: FolderOpen },
  { id: "subtasks", label: "子任务", shortcut: "⌥⌘S", icon: ListTree },
];

interface WorkPanelLauncherProps {
  taskKey: string;
  onOpenTab: (tab: WorkPanelTab) => void;
}

export function WorkPanelLauncher({ taskKey, onOpenTab }: WorkPanelLauncherProps) {
  const [selection, setSelection] = useState<Exclude<WorkPanelLauncherAction, "review" | "terminal"> | null>(null);

  const handleAction = (action: WorkPanelLauncherAction) => {
    if (action === "review") {
      onOpenTab({ kind: "review", id: `review:${taskKey}`, label: "审阅 · 当前改动", taskId: taskKey });
      return;
    }
    if (action === "terminal") {
      onOpenTab({ kind: "terminal", id: `terminal:${taskKey}`, label: "终端", taskId: taskKey });
      return;
    }
    setSelection(action);
  };

  if (selection) {
    return (
      <div className="forge-work-panel-picker" data-testid={`work-panel-${selection}-picker`}>
        <ButtonPrimitive type="button" className="forge-work-panel-picker-back" onClick={() => setSelection(null)}>
          <ArrowLeft className="size-4" />
          返回
        </ButtonPrimitive>
        <div className="forge-work-panel-picker-copy">
          <h3>{pickerTitle(selection)}</h3>
          <p>输入搜索或选择一个可用对象后再打开。</p>
        </div>
      </div>
    );
  }

  return (
    <div className="forge-work-panel-launcher" data-testid="work-panel-launcher">
      <div className="forge-work-panel-launcher-actions">
        {launcherActions.map((action) => (
          <ButtonPrimitive
            key={action.id}
            type="button"
            className="forge-work-panel-launcher-action"
            onClick={() => handleAction(action.id)}
          >
            <action.icon className="size-5" />
            <span>{action.label}</span>
            {action.shortcut ? <kbd>{action.shortcut}</kbd> : null}
          </ButtonPrimitive>
        ))}
      </div>
    </div>
  );
}

function pickerTitle(action: Exclude<WorkPanelLauncherAction, "review" | "terminal">) {
  switch (action) {
    case "preview":
      return "选择预览";
    case "files":
      return "选择文件";
    case "subtasks":
      return "选择子任务";
  }
}
