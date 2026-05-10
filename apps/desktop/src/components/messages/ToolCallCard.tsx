import { useState } from "react";
import {
  ChevronRight,
  Wrench,
  CheckCircle2,
  XCircle,
  Loader2,
} from "lucide-react";
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from "@/components/ui/collapsible";
import { Badge } from "@/components/ui/badge";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

interface ToolCallCardProps {
  block: BlockState;
}

export function ToolCallCard({ block }: ToolCallCardProps) {
  const [open, setOpen] = useState(true);

  const isToolCall = block.event_type === "tool_call";
  const isError = Boolean(block.metadata.is_error ?? false);

  let toolName: string;
  let status: "running" | "success" | "error";

  if (isToolCall) {
    toolName = (block.metadata.tool_name as string) || "unknown";
    status = block.isComplete ? "success" : "running";
  } else {
    toolName = (block.metadata.tool_name as string) || "Tool";
    status = isError ? "error" : "success";
  }

  const statusStyles: Record<string, { border: string; bg: string; accent: string }> = {
    running: {
      border: "border-blue-500/20",
      bg: "bg-blue-500/[0.03]",
      accent: "text-blue-400",
    },
    success: {
      border: "border-emerald-500/20",
      bg: "bg-emerald-500/[0.03]",
      accent: "text-emerald-400",
    },
    error: {
      border: "border-destructive/20",
      bg: "bg-destructive/[0.03]",
      accent: "text-red-400",
    },
  };

  const style = statusStyles[status];

  const StatusIcon = {
    running: Loader2,
    success: CheckCircle2,
    error: XCircle,
  }[status];

  const statusBadgeVariant = {
    running: "secondary",
    success: "default",
    error: "destructive",
  } as const;

  return (
    <div className="mb-5">
      <Collapsible open={open} onOpenChange={setOpen}>
        <div
          className={cn(
            "border rounded-lg overflow-hidden transition-all duration-200",
            style.border,
            style.bg
          )}
        >
          {/* Header */}
          <CollapsibleTrigger className="w-full flex items-center gap-2.5 px-4 py-2.5 text-sm hover:bg-white/[0.03] transition-colors duration-150">
            <ChevronRight
              className={`size-4 shrink-0 transition-transform duration-200 ease-out text-muted-foreground ${
                open ? "rotate-90" : ""
              }`}
            />
            <Wrench className="size-4 shrink-0 text-muted-foreground/70" />
            <span className="font-medium text-foreground/90 text-xs tracking-wide uppercase truncate">
              {toolName}
            </span>
            <Badge
              variant={statusBadgeVariant[status]}
              className="shrink-0 ml-auto"
            >
              <StatusIcon
                className={cn(
                  "size-3",
                  status === "running" && "animate-spin"
                )}
              />
              <span className="ml-1 text-[11px]">
                {status === "running"
                  ? "Running"
                  : status === "error"
                    ? "Error"
                    : "Done"}
              </span>
            </Badge>
          </CollapsibleTrigger>

          {/* Content */}
          <CollapsibleContent className="overflow-hidden data-[panel-open]:animate-[collapsible-down_200ms_ease-out] data-[panel-closed]:animate-[collapsible-up_200ms_ease-out]">
            <div className="px-4 pb-4 border-t border-border/20">
              {/* Input (for tool_call) */}
              {isToolCall && Boolean(block.metadata.tool_input) && (
                <div className="mt-3">
                  <p className="text-[11px] text-muted-foreground/70 font-semibold uppercase tracking-widest mb-1.5">
                    Input
                  </p>
                  <pre className="bg-black/10 dark:bg-black/20 border border-border/20 rounded-lg p-3 text-xs font-mono whitespace-pre-wrap overflow-x-auto leading-relaxed">
                    {formatJSONValue(block.metadata.tool_input)}
                  </pre>
                </div>
              )}

              {/* Result / Output */}
              {block.content && (
                <div className="mt-3">
                  <p className="text-[11px] text-muted-foreground/70 font-semibold uppercase tracking-widest mb-1.5">
                    {isToolCall ? "Output" : "Result"}
                  </p>
                  <pre
                    className={cn(
                      "bg-black/10 dark:bg-black/20 border rounded-lg p-3 text-xs font-mono whitespace-pre-wrap overflow-x-auto leading-relaxed",
                      isError
                        ? "border-red-500/20"
                        : "border-border/20"
                    )}
                  >
                    {block.content}
                  </pre>
                </div>
              )}

              {/* Empty state when tool is still running */}
              {!block.content && !block.isComplete && (
                <div className="mt-3 text-xs text-muted-foreground/60 flex items-center gap-2">
                  <Loader2 className="size-3 animate-spin" />
                  Waiting for result...
                </div>
              )}
            </div>
          </CollapsibleContent>
        </div>
      </Collapsible>
    </div>
  );
}

/** Format a tool_input value (string or object) for display. */
function formatJSONValue(value: unknown): string {
  if (typeof value === "string") {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
