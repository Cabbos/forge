import { FileDiff, FileText, Globe2, ListTree, TerminalSquare } from "lucide-react";
import { WorkPanelFiles } from "./WorkPanelFiles";
import { WorkPanelPreview } from "./WorkPanelPreview";
import type { WorkPanelTab } from "./workPanelTypes";

export function WorkPanelContent({
  tab,
  onOpenTab,
}: {
  tab: WorkPanelTab;
  onOpenTab: (tab: WorkPanelTab) => void;
}) {
  if (tab.kind === "preview") return <WorkPanelPreview tab={tab} />;
  if (tab.kind === "file") return <WorkPanelFiles tab={tab} onOpenTab={onOpenTab} />;

  const content = placeholderForTab(tab);
  return (
    <div className="forge-work-panel-placeholder" data-testid={`work-panel-content-${tab.kind}`}>
      <content.icon className="size-5" />
      <strong>{content.title}</strong>
      <span>{content.detail}</span>
    </div>
  );
}

function placeholderForTab(tab: WorkPanelTab) {
  switch (tab.kind) {
    case "review":
      return { icon: FileDiff, title: "审阅当前改动", detail: "正在接入当前工作区的变更视图。" };
    case "terminal":
      return { icon: TerminalSquare, title: "临时终端", detail: "正在准备当前任务的验证终端。" };
    case "preview":
      return { icon: Globe2, title: tab.label, detail: "正在加载最新结果。" };
    case "file":
      return { icon: FileText, title: tab.label, detail: tab.path };
    case "subtask":
      return { icon: ListTree, title: tab.label, detail: "正在读取子任务状态。" };
  }
}
