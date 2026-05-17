import type { WorkflowState } from "@/lib/protocol";
import {
  deriveTaskModeView,
  taskGateCopy,
  taskGateLabel,
} from "@/lib/task-mode";

export function CurrentTaskCard({ workflow }: { workflow: WorkflowState | null }) {
  const mode = deriveTaskModeView(workflow);
  const gateCopy = workflow ? taskGateCopy(workflow.gate) : null;

  if (!workflow) {
    return (
      <section>
        <div className="forge-section-head">
          <h3 className="forge-section-title">当前任务</h3>
          <span className="forge-section-meta">自动判断</span>
        </div>
        <div className="forge-empty">
          发送消息后会显示当前任务判断。
        </div>
      </section>
    );
  }

  return (
    <section>
      <div className="forge-section-head">
        <h3 className="forge-section-title">当前任务</h3>
        <span className="forge-section-meta">自动判断</span>
      </div>
      <div className="forge-surface px-3 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-xs font-medium text-foreground">{mode.label}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{mode.title}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground/80">{workflow.reason || mode.description}</div>
          </div>
          <span className="forge-pill">
            {taskGateLabel(workflow.gate)}
          </span>
        </div>

        {gateCopy && (
          <div className="mt-2 rounded border border-amber-500/20 bg-amber-500/5 px-2 py-1.5 text-[11px] text-amber-200/90">
            {gateCopy}
          </div>
        )}

      </div>
    </section>
  );
}
