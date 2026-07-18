import { useId, useState } from "react";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import { ConversationProcessItem } from "@/components/chat/ConversationProcessItem";
import type {
  ProcessDigest,
  TurnTerminalSummary,
} from "@/components/chat/conversationTurnView";
import {
  ForgeCollapsible,
  ForgeCollapsibleContent,
  ForgeCollapsibleTrigger,
} from "@/components/primitives/collapsible";

export function ConversationProcessDisclosure({
  digest,
  terminal,
  sessionId,
}: {
  digest: ProcessDigest;
  terminal: TurnTerminalSummary;
  sessionId?: string;
}) {
  const [open, setOpen] = useState(false);
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const contentId = `forge-process-${useId().replace(/:/g, "")}`;
  const hasEvidence = digest.items.length > 0 || digest.usage.length > 0 || Boolean(digest.delivery);
  const hasRuntimeEvidence = digest.usage.length > 0 || Boolean(digest.delivery);
  const summary = terminalSummaryLabel(terminal);

  if (!hasEvidence) {
    return (
      <div
        data-testid="conversation-process-status"
        data-terminal-outcome={terminal.outcome}
        className="forge-process-status"
      >
        {summary}
      </div>
    );
  }

  return (
    <ForgeCollapsible open={open} onOpenChange={setOpen}>
      <div
        data-testid="conversation-process-disclosure"
        data-terminal-outcome={terminal.outcome}
        className="forge-process-disclosure"
      >
        <ForgeCollapsibleTrigger
          type="button"
          data-testid="conversation-process-trigger"
          aria-label={`${summary}，${open ? "收起运行过程" : "查看运行过程"}`}
          aria-expanded={open}
          aria-controls={contentId}
          className="forge-process-disclosure-trigger"
        >
          <span className="forge-process-disclosure-status">{summary}</span>
          <span
            aria-hidden="true"
            data-open={open ? "true" : "false"}
            className="forge-process-disclosure-chevron"
          >
            ›
          </span>
        </ForgeCollapsibleTrigger>

        {open && (
          <ForgeCollapsibleContent id={contentId}>
            <div className="forge-process-disclosure-content">
              {digest.items.length > 0 && (
                <ol data-testid="conversation-process-timeline" className="forge-process-digest-list">
                  {digest.items.map((item) => (
                    <ConversationProcessItem key={item.id} item={item} sessionId={sessionId} />
                  ))}
                </ol>
              )}

              {hasRuntimeEvidence && (
                <ForgeCollapsible open={evidenceOpen} onOpenChange={setEvidenceOpen}>
                  <ForgeCollapsibleTrigger
                    type="button"
                    data-testid="conversation-evidence-trigger"
                    aria-label={evidenceOpen ? "收起运行证据" : "查看运行证据"}
                    className="forge-process-evidence-trigger"
                  >
                    <span>运行证据</span>
                    <span
                      aria-hidden="true"
                      data-open={evidenceOpen ? "true" : "false"}
                      className="forge-process-disclosure-chevron"
                    >
                      ›
                    </span>
                  </ForgeCollapsibleTrigger>
                  {evidenceOpen && (
                    <ForgeCollapsibleContent>
                      <DigestMetadata digest={digest} sessionId={sessionId} />
                    </ForgeCollapsibleContent>
                  )}
                </ForgeCollapsible>
              )}
            </div>
          </ForgeCollapsibleContent>
        )}
      </div>
    </ForgeCollapsible>
  );
}

function DigestMetadata({
  digest,
  sessionId,
}: {
  digest: ProcessDigest;
  sessionId?: string;
}) {
  const deliverySummary = digest.delivery?.metadata.summary;
  const delivery = isRecord(deliverySummary) ? deliverySummary : null;
  const verification = stringValue(delivery?.verification_label);
  const checkpoint = stringValue(delivery?.checkpoint_label);
  const usage = digest.usage[digest.usage.length - 1] ?? null;
  if (!usage && !verification && !checkpoint) return null;

  return (
    <div className="forge-process-digest-metadata">
      {(verification || checkpoint) && (
        <div data-testid="conversation-delivery-metadata" className="forge-process-metadata-row">
          <span>交付</span>
          <span>{[verification, checkpoint].filter(Boolean).join(" · ")}</span>
        </div>
      )}

      {usage && (
        <div className="forge-process-runtime-evidence">
          <div className="forge-process-metadata-row">
            <span>模型用量</span>
            <span>{usageSummary(usage.metadata)}</span>
          </div>
          <div className="forge-process-detail-content forge-process-usage-detail">
            <MemoizedBlockRenderer block={usage} sessionId={sessionId} />
          </div>
        </div>
      )}
    </div>
  );
}

function usageSummary(metadata: Record<string, unknown>) {
  const model = stringValue(metadata.model) ?? "未知模型";
  const input = numberValue(metadata.input_tokens);
  const output = numberValue(metadata.output_tokens);
  const tokens = input === null || output === null ? "用量未知" : `${input} / ${output}`;
  return `${model} · ${tokens}`;
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function terminalSummaryLabel(terminal: TurnTerminalSummary) {
  const parts = [outcomeLabel(terminal.outcome)];
  const duration = durationLabel(terminal.durationMs);
  if (duration) parts.push(duration);
  if (terminal.operationCount > 0) parts.push(`${terminal.operationCount} 项操作`);
  return parts.join(" · ");
}

function outcomeLabel(outcome: TurnTerminalSummary["outcome"]) {
  if (outcome === "stopped") return "已停止";
  if (outcome === "failed") return "未完成";
  return "已完成";
}

function durationLabel(durationMs: number | null) {
  if (durationMs === null) return null;
  if (durationMs < 1_000) return "<1 秒";

  const totalSeconds = Math.floor(durationMs / 1_000);
  if (totalSeconds < 60) return `${totalSeconds} 秒`;

  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return seconds === 0 ? `${minutes} 分钟` : `${minutes} 分 ${seconds} 秒`;
}
