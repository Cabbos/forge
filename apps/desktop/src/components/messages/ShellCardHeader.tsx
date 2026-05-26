import { CheckCircle2, ChevronRight, Loader2, Terminal } from "lucide-react";
import { CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";

interface ShellCardHeaderProps {
  command: string;
  expanded: boolean;
  exitCode?: number;
  isError: boolean;
  isRunning: boolean;
  state: string;
  tone: string;
}

export function ShellCardHeader({
  command,
  expanded,
  exitCode,
  isError,
  isRunning,
  state,
  tone,
}: ShellCardHeaderProps) {
  return (
    <CollapsibleTrigger
      data-testid="shell-card-trigger"
      data-state={state}
      className="forge-log-line"
      data-tone={tone}
    >
      <ChevronRight className={cn("size-3 shrink-0 transition-transform", expanded && "rotate-90")} />
      <Terminal className="size-3 shrink-0" />
      <span className="forge-log-line-command">{command}</span>
      {isRunning ? (
        <span
          className="forge-log-status forge-log-line-status"
          data-tone="running"
          data-status="running"
          title="运行中"
        >
          <Loader2 className="size-3 animate-spin" />
        </span>
      ) : (
        <span
          data-testid={isError ? "shell-exit-code" : undefined}
          className="forge-log-status forge-log-line-status"
          data-tone={isError ? "error" : "success"}
          data-status={isError ? "error" : "done"}
          title={isError ? `退出码 ${exitCode}` : "完成"}
        >
          {isError ? `exit ${exitCode}` : <CheckCircle2 className="size-3" />}
        </span>
      )}
    </CollapsibleTrigger>
  );
}
