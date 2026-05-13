import { Compass, ShieldAlert } from "lucide-react";
import type { WorkflowState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function WorkflowStatusPill({ workflow }: { workflow: WorkflowState | null }) {
  if (!workflow) return null;

  const strict = workflow.gate === "approval_required";

  return (
    <span
      className={cn(
        "inline-flex max-w-[220px] shrink-0 items-center gap-1 rounded-md border px-2 py-0.5 text-[10px]",
        strict ? "border-amber-500/30 text-amber-300" : "border-border text-muted-foreground",
      )}
      title={`${workflow.developer_label}: ${workflow.reason}`}
    >
      {strict ? <ShieldAlert className="size-3" /> : <Compass className="size-3" />}
      <span className="truncate">{workflow.beginner_label}</span>
    </span>
  );
}
