import { useState, useEffect } from "react";
import { Check, Copy } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { SubAgentTrace } from "@/components/messages/SubAgentTrace";
import { deriveToolCallView } from "./processToolPresentation";

function meterSegments(status: string): number {
  switch (status) {
    case "running": return 3;
    case "done": return 5;
    case "error": return 2;
    default: return 0;
  }
}

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

  const segments = meterSegments(toolView.status);

  return (
    <div className="tool-machine">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          data-testid="tool-card-trigger"
          data-state={toolView.status}
          className="tool-machine-plate"
          data-tone={toolView.isError ? "error" : "default"}
        >
          <div className="tool-machine-led" data-status={toolView.status} />
          <span className="tool-machine-name">
            {toolView.actionText}
            {toolView.inputSummary ? `: ${toolView.inputSummary}` : ""}
          </span>
          {toolView.durationLabel && (
            <span style={{ fontSize: 10, color: "var(--forge-text-faint)", flexShrink: 0 }}>
              {toolView.durationLabel}
            </span>
          )}
          <span className="tool-machine-status" data-status={toolView.status}>
            {toolView.status === "running" ? "running" : toolView.status === "error" ? "error" : "done"}
          </span>
        </CollapsibleTrigger>
        <div className="tool-machine-meter">
          {Array.from({ length: 5 }, (_, i) => (
            <div key={i} className="tool-machine-segment" data-filled={i < segments ? "true" : "false"} />
          ))}
        </div>
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
