import { WorkPanelFiles } from "./WorkPanelFiles";
import { WorkPanelPreview } from "./WorkPanelPreview";
import { WorkPanelReview } from "./WorkPanelReview";
import { WorkPanelSubtask } from "./WorkPanelSubtask";
import { WorkPanelTerminal } from "./WorkPanelTerminal";
import type { WorkPanelTab } from "./workPanelTypes";

export function WorkPanelContent({
  tab,
  onOpenTab,
}: {
  tab: WorkPanelTab;
  onOpenTab: (tab: WorkPanelTab) => void;
}) {
  if (tab.kind === "review") return <WorkPanelReview />;
  if (tab.kind === "preview") return <WorkPanelPreview tab={tab} />;
  if (tab.kind === "file") return <WorkPanelFiles tab={tab} onOpenTab={onOpenTab} />;
  if (tab.kind === "subtask") return <WorkPanelSubtask tab={tab} />;
  if (tab.kind === "terminal") return <WorkPanelTerminal tab={tab} />;
}
