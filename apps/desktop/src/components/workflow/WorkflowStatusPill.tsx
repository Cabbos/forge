import { Compass, ShieldAlert } from "lucide-react";
import type { WorkflowState } from "@/lib/protocol";
import { deriveTaskModeView } from "@/lib/task-mode";
import { cn } from "@/lib/utils";

export function WorkflowStatusPill({
  workflow,
  activeContextCount = 0,
  onOpenContext,
}: {
  workflow: WorkflowState | null;
  activeContextCount?: number;
  onOpenContext?: () => void;
}) {
  if (!workflow) return null;

  const strict = workflow.gate === "approval_required";
  const mode = deriveTaskModeView(workflow);
  const label = activeContextCount > 0 ? `${mode.label} · 已参考 ${activeContextCount}` : mode.label;

  return (
    <button
      type="button"
      data-testid="workflow-status-pill"
      onClick={onOpenContext}
      className={cn(
        "inline-flex min-w-0 max-w-[220px] shrink items-center gap-1 rounded-md border px-2 py-0.5 text-[10px] transition-colors",
        strict ? "border-amber-500/30 text-amber-300" : "border-border text-muted-foreground",
        onOpenContext && "hover:bg-secondary hover:text-foreground",
      )}
      title={workflow.reason || mode.description}
    >
      {strict ? <ShieldAlert className="size-3" /> : <Compass className="size-3" />}
      <span className="truncate">{label}</span>
    </button>
  );
}
