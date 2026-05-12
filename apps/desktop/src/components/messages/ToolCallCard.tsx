import { useState, useEffect } from "react";
import { ChevronRight, Loader2, CheckCircle2, XCircle } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { SubAgentTrace } from "@/components/messages/SubAgentTrace";
import { cn } from "@/lib/utils";

export function ToolCallCard({ block }: { block: BlockState }) {
  const isError = Boolean(block.metadata.is_error ?? false);
  const [open, setOpen] = useState(false);
  // Keep normal tool chatter compact; only surface errors automatically.
  useEffect(() => {
    if (block.isComplete && isError) setOpen(true);
  }, [block.isComplete, isError]);
  const toolName = (block.metadata.tool_name as string) || "tool";
  const status = block.isComplete ? (isError ? "error" : "done") : "running";

  const StatusIcon = { running: Loader2, done: CheckCircle2, error: XCircle }[status];
  const statusColor = { running: "#D4A853", done: "#4A9E6B", error: "#D47777" }[status];
  const statusText = { running: "running", done: "done", error: "error" }[status];

  return (
    <div className="mb-3">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="inline-flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer transition-colors border font-mono text-xs"
          style={{ background: "#0a0a0a", borderColor: "#181818", color: "#D4A853" }}>
          <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
          <span>{toolName}</span>
          <span style={{ color: "#555", fontSize: "11px" }}>
            {block.metadata.tool_input ? JSON.stringify(block.metadata.tool_input).slice(0, 60) : ""}
          </span>
          <span className="flex items-center gap-1" style={{ color: statusColor, fontSize: "10px" }}>
            <StatusIcon className={cn("size-3", status === "running" && "animate-spin")} />
            {statusText}
          </span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          {toolName === "delegate_task" ? (
            <SubAgentTrace content={block.content} />
          ) : (
            <div className="mt-1.5 p-3 rounded-md border font-mono text-xs whitespace-pre-wrap break-all"
              style={{ background: "#060606", borderColor: "#181818", color: "#999", maxHeight: "200px", overflow: "auto", maxWidth: "100%" }}>
              {block.content || (status === "running" ? "Waiting..." : "")}
            </div>
          )}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
