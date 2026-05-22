import { useState, useEffect } from "react";
import { Check, ChevronRight, Copy, Loader2, CheckCircle2, XCircle, Wrench } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { SubAgentTrace } from "@/components/messages/SubAgentTrace";
import { cn } from "@/lib/utils";
import { deriveToolCallView } from "./processToolPresentation";

export function ToolCallCard({ block }: { block: BlockState }) {
  const toolView = deriveToolCallView(block);
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  // Keep normal tool chatter compact; only surface errors automatically.
  useEffect(() => {
    if (block.isComplete && toolView.isError) setOpen(true);
  }, [block.isComplete, toolView.isError]);

  const StatusIcon = { running: Loader2, done: CheckCircle2, error: XCircle }[toolView.status];
  const statusColor = { running: "var(--forge-text-faint)", done: "var(--forge-icon-safety)", error: "var(--destructive)" }[toolView.status];
  const copyDetails = async () => {
    await navigator.clipboard?.writeText(toolView.detailText);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div>
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          data-testid="tool-card-trigger"
          data-state={toolView.status}
          className="forge-log-line"
          data-tone={toolView.isError ? "error" : "default"}
        >
          <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
          <Wrench className="size-3.5 shrink-0" style={{ color: statusColor }} />
          <span className="shrink-0 font-medium">{toolView.actionText}</span>
          {toolView.inputSummary && (
            <span className="min-w-0 truncate font-mono text-[11px]" style={{ color: "var(--muted-foreground)" }}>
              {toolView.inputSummary}
            </span>
          )}
          {toolView.durationLabel && (
            <span className="ml-auto shrink-0 font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>
              {toolView.durationLabel}
            </span>
          )}
          <span className={cn("flex shrink-0 items-center", !toolView.durationLabel && "ml-auto")} style={{ color: statusColor }} title={toolView.status === "running" ? "进行中" : toolView.status === "error" ? "异常" : "完成"}>
            <StatusIcon className={cn("size-3", toolView.status === "running" && "animate-spin")} />
          </span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          {toolView.toolName === "delegate_task" ? (
            <SubAgentTrace content={block.content} />
          ) : (
            <div data-testid="log-detail-surface" className="forge-log-detail">
              <div data-testid="log-detail-header" className="forge-log-detail-header">
                <span className="text-[11px]" style={{ color: "var(--muted-foreground)" }}>技术细节</span>
                <div className="flex items-center gap-2">
                  <span className="font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>{toolView.toolName}</span>
                  <button
                    type="button"
                    aria-label={copied ? "已复制工具输出" : "复制工具输出"}
                    title={copied ? "已复制" : "复制工具输出"}
                    onClick={copyDetails}
                    disabled={!toolView.detailText}
                    className="forge-log-action disabled:cursor-default disabled:opacity-45"
                  >
                    {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
                  </button>
                </div>
              </div>
              {toolView.resultSummary && (
                <div data-testid="tool-result-summary" className="forge-log-summary" data-tone={toolView.isError ? "error" : "default"}>
                  <span>{toolView.isError ? "失败原因" : "结果"}</span>
                  <strong>{toolView.resultSummary}</strong>
                </div>
              )}
              <div data-testid="log-detail-output" className="forge-log-output">
                {toolView.detailText}
              </div>
            </div>
          )}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
