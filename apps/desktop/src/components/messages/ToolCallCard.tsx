import { useState, useEffect } from "react";
import { Check, CheckCircle2, Copy, Loader2, Wrench, XCircle } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeCollapsible, ForgeCollapsibleTrigger, ForgeCollapsibleContent } from "@/components/primitives/collapsible";
import type { BlockState } from "@/lib/protocol";
import { SubAgentTrace } from "@/components/messages/SubAgentTrace";
import { deriveToolCallView } from "./processToolPresentation";

export function ToolCallCard({ block }: { block: BlockState }) {
  const toolView = deriveToolCallView(block);
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  // Keep normal tool chatter compact; only surface errors automatically.
  useEffect(() => {
    if (block.isComplete && toolView.isError) setOpen(true);
  }, [block.isComplete, toolView.isError]);

  const copyDetails = async () => {
    await navigator.clipboard?.writeText(toolView.detailText);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  const statusLabel = toolView.status === "running" ? "运行中" : toolView.status === "error" ? "失败" : "完成";
  const StatusIcon = toolView.status === "running" ? Loader2 : toolView.status === "error" ? XCircle : CheckCircle2;

  return (
    <div className="tool-machine">
      <ForgeCollapsible open={open} onOpenChange={setOpen}>
        <ForgeCollapsibleTrigger
          data-testid="tool-card-trigger"
          data-state={toolView.status}
          className="forge-log-line forge-evidence-row tool-machine-plate"
          data-tone={toolView.isError ? "error" : "default"}
        >
          <Wrench className="forge-log-line-icon size-3" data-status={toolView.status} />
          <span className="forge-log-line-command tool-machine-name">{toolView.actionText}</span>
          {toolView.inputSummary && (
            <span className="forge-log-line-input tool-machine-input">{toolView.inputSummary}</span>
          )}
          {toolView.durationLabel && (
            <span className="forge-log-line-meta forge-log-line-duration tool-machine-duration">
              {toolView.durationLabel}
            </span>
          )}
          <span
            className="forge-log-status forge-log-line-status tool-machine-status"
            data-tone={toolView.status === "error" ? "error" : toolView.status === "running" ? "running" : "success"}
            data-status={toolView.status}
            title={statusLabel}
            aria-label={statusLabel}
          >
            <StatusIcon className={`size-3 ${toolView.status === "running" ? "animate-spin" : ""}`} />
          </span>
        </ForgeCollapsibleTrigger>
        <ForgeCollapsibleContent>
          {toolView.toolName === "delegate_task" ? (
            <SubAgentTrace content={block.content} />
          ) : (
            <div data-testid="log-detail-surface" className="forge-log-detail">
              <div data-testid="log-detail-header" className="forge-log-detail-header">
                <span className="text-[11px]" style={{ color: "var(--muted-foreground)" }}>技术细节</span>
                <div className="flex items-center gap-2">
                  <span className="font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>{toolView.toolName}</span>
                  <ButtonPrimitive
                    type="button"
                    aria-label={copied ? "已复制工具输出" : "复制工具输出"}
                    title={copied ? "已复制" : "复制工具输出"}
                    onClick={copyDetails}
                    disabled={!toolView.detailText}
                    className="forge-log-action disabled:cursor-default disabled:opacity-45"
                  >
                    {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
                  </ButtonPrimitive>
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
        </ForgeCollapsibleContent>
      </ForgeCollapsible>
    </div>
  );
}
