import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { WorkflowState } from "@/lib/protocol";

export function CurrentTaskCard({ workflow }: { workflow: WorkflowState | null }) {
  const [expanded, setExpanded] = useState(false);

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
            <div className="truncate text-xs font-medium text-foreground">{workflow.beginner_label}</div>
            <div className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{workflow.reason}</div>
          </div>
          <span className="shrink-0 rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {gateLabel(workflow.gate)}
          </span>
        </div>

        {workflow.gate !== "none" && (
          <div className="mt-2 rounded border border-amber-500/20 bg-amber-500/5 px-2 py-1.5 text-[11px] text-amber-200/90">
            {workflow.gate === "soft" ? "建议先梳理方案，也可以直接处理。" : "建议先确认方案，再进入实现。"}
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

function gateLabel(gate: WorkflowState["gate"]) {
  switch (gate) {
    case "none":
      return "直接";
    case "soft":
      return "建议";
    case "approval_required":
      return "需确认";
  }
}
