import { CheckCircle2, ChevronRight, Loader2, XCircle } from "lucide-react";
import { ForgeCollapsibleTrigger } from "@/components/primitives/collapsible";
import { cn } from "@/lib/utils";
import type { ProcessActivityState } from "@/components/messages/processActivity";

interface ToolActivitySummaryProps {
  state: ProcessActivityState;
  isRunning: boolean;
  label: string;
  summaryItems: string[];
  open: boolean;
}

export function ToolActivitySummary({
  state,
  isRunning,
  label,
  summaryItems,
  open,
}: ToolActivitySummaryProps) {
  const StatusIcon = state === "error" ? XCircle : state === "running" ? Loader2 : CheckCircle2;

  return (
    <ForgeCollapsibleTrigger
      data-testid="tool-activity-summary"
      data-state={state}
      className="forge-tool-activity-summary"
    >
      <ChevronRight className={cn("size-3 shrink-0 transition-transform", open && "rotate-90")} />
      <StatusIcon
        data-running-icon={isRunning ? "true" : undefined}
        className={cn("size-3.5 shrink-0", isRunning && "animate-spin")}
      />
      <span className="forge-tool-activity-summary-label">{label}</span>
      {summaryItems.length > 0 && (
        <span className="forge-tool-activity-summary-items">
          {summaryItems.map((item) => (
            <span key={item} className="forge-tool-activity-summary-item">{item}</span>
          ))}
        </span>
      )}
    </ForgeCollapsibleTrigger>
  );
}
