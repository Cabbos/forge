import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { WorkflowOverrideAction, WorkflowState } from "@/lib/protocol";
import { overrideWorkflowRoute } from "@/lib/tauri";
import {
  deriveTaskModeView,
  taskGateCopy,
  taskGateLabel,
  workflowOverrideLabel,
} from "@/lib/task-mode";
import { useStore } from "@/store";

export function CurrentTaskCard({ workflow }: { workflow: WorkflowState | null }) {
  const [expanded, setExpanded] = useState(false);
  const [busyAction, setBusyAction] = useState<WorkflowOverrideAction | null>(null);
  const setWorkflowState = useStore((s) => s.setWorkflowState);
  const mode = deriveTaskModeView(workflow);
  const gateCopy = workflow ? taskGateCopy(workflow.gate) : null;

  const handleOverride = async (action: WorkflowOverrideAction) => {
    if (!workflow || busyAction) return;
    setBusyAction(action);
    try {
      const next = await overrideWorkflowRoute(workflow.session_id, action);
      setWorkflowState(workflow.session_id, next);
    } finally {
      setBusyAction(null);
    }
  };

  if (!workflow) {
    return (
      <section>
        <div className="mb-2 flex items-center justify-between">
          <h3 className="text-[11px] font-medium text-muted-foreground">当前任务</h3>
          <span className="text-[10px] text-muted-foreground/70">自动判断</span>
        </div>
        <div className="rounded-md border border-border bg-card px-3 py-3 text-xs text-muted-foreground">
          发送消息后会显示当前工作方式。
        </div>
      </section>
    );
  }

  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">当前任务</h3>
        <span className="text-[10px] text-muted-foreground/70">自动判断</span>
      </div>
      <div className="rounded-md border border-border bg-card px-3 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-xs font-medium text-foreground">{mode.label}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{mode.title}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground/80">{workflow.reason || mode.description}</div>
          </div>
          <span className="shrink-0 rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {taskGateLabel(workflow.gate)}
          </span>
        </div>

        {gateCopy && (
          <div className="mt-2 rounded border border-amber-500/20 bg-amber-500/5 px-2 py-1.5 text-[11px] text-amber-200/90">
            {gateCopy}
          </div>
        )}

        {workflow.override_actions.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {workflow.override_actions.map((action) => (
              <button
                key={action}
                type="button"
                disabled={busyAction !== null}
                onClick={() => handleOverride(action)}
                className="rounded border border-border px-2 py-1 text-[10px] text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:cursor-default disabled:opacity-60"
              >
                {busyAction === action ? "切换中" : workflowOverrideLabel(action)}
              </button>
            ))}
          </div>
        )}

        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="mt-2 inline-flex items-center gap-1 text-[10px] text-muted-foreground transition-colors hover:text-foreground"
        >
          {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
          开发者详情
        </button>

        {expanded && (
          <div className="mt-2 space-y-1 rounded border border-border bg-background/40 p-2 font-mono text-[10px] text-muted-foreground">
            <Row label="route" value={workflow.developer_label} />
            <Row label="phase" value={workflow.phase} />
            <Row label="gate" value={workflow.gate} />
            <Row label="signals" value={workflow.matched_signals.join(", ") || "none"} />
            {workflow.spec_path && <Row label="spec" value={workflow.spec_path} />}
            {workflow.plan_path && <Row label="plan" value={workflow.plan_path} />}
            {workflow.checkpoint_id && <Row label="checkpoint" value={workflow.checkpoint_id} />}
          </div>
        )}
      </div>
    </section>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[72px_minmax(0,1fr)] gap-2">
      <span className="text-muted-foreground/60">{label}</span>
      <span className="truncate text-muted-foreground">{value}</span>
    </div>
  );
}
