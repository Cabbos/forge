import { CheckCircle2, CircleDashed, PanelRightOpen } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import type { AgentA2AProjection } from "@/lib/protocol";

export function AgentA2AInlineSummary({ state }: { state: AgentA2AProjection | null }) {
  if (!state || state.tasks.length === 0) return null;

  const openWorkPanel = () => {
    window.dispatchEvent(new Event("open-work-panel"));
  };
  const statusText = state.running_count > 0
    ? `${state.running_count} 个子任务运行中`
    : `${state.completed_count} 个子任务已完成`;

  return (
    <ButtonPrimitive
      type="button"
      className="forge-a2a-inline-summary"
      data-running={state.running_count > 0}
      onClick={openWorkPanel}
      title="打开工作面板查看子任务过程"
    >
      <span className="forge-a2a-inline-icon">
        {state.running_count > 0 ? <CircleDashed className="size-3.5" /> : <CheckCircle2 className="size-3.5" />}
      </span>
      <span className="forge-a2a-inline-copy">
        <span className="forge-a2a-inline-title">子任务</span>
        <span className="forge-a2a-inline-detail">{statusText}，查看过程与审阅材料</span>
      </span>
      <PanelRightOpen className="size-3.5" />
    </ButtonPrimitive>
  );
}
