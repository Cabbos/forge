import { useState, useEffect } from "react";
import { Check, CheckCircle2, Copy, Loader2, Wrench, XCircle } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeCollapsible, ForgeCollapsibleTrigger, ForgeCollapsibleContent } from "@/components/primitives/collapsible";
import type { BlockState, PermissionLedgerEvent } from "@/lib/protocol";
import { SubAgentTrace } from "@/components/messages/SubAgentTrace";
import { WriteFilePreview } from "@/components/messages/WriteFilePreview";
import { deriveToolCallView } from "./processToolPresentation";
import { deriveWritePreview } from "@/components/messages/writePreviewPresentation";

export function ToolCallCard({ block }: { block: BlockState }) {
  const toolView = deriveToolCallView(block);
  const writePreview = deriveWritePreview(toolView.toolName, block.metadata.tool_input);
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
              <WriteFilePreview preview={writePreview} />
              <ToolPermissionEvidence evidence={parsePermissionEvidence(block.metadata.permission_evidence)} />
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

function ToolPermissionEvidence({ evidence }: { evidence: PermissionLedgerEvent | null }) {
  if (!evidence) return null;
  return (
    <div data-testid="tool-permission-evidence" className="forge-log-summary" data-tone="default">
      <span>后端权限依据</span>
      <strong>{evidence.kind} · {evidence.permission_mode} · {evidence.reason}</strong>
    </div>
  );
}

function parsePermissionEvidence(value: unknown): PermissionLedgerEvent | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const record = value as Partial<PermissionLedgerEvent>;
  if (
    typeof record.kind !== "string" ||
    typeof record.workspace_path !== "string" ||
    typeof record.risk_tier !== "string" ||
    !Array.isArray(record.affected_files) ||
    typeof record.operation !== "string" ||
    typeof record.permission_mode !== "string" ||
    typeof record.reason !== "string"
  ) {
    return null;
  }
  return record as PermissionLedgerEvent;
}
