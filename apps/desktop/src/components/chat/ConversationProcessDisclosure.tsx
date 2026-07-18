import { useState } from "react";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import { ConversationProcessItem } from "@/components/chat/ConversationProcessItem";
import type { ProcessDigest } from "@/components/chat/conversationTurnView";
import {
  ForgeCollapsible,
  ForgeCollapsibleContent,
  ForgeCollapsibleTrigger,
} from "@/components/primitives/collapsible";
import { useStore } from "@/store";

export function ConversationProcessDisclosure({
  digest,
  sessionId,
}: {
  digest: ProcessDigest;
  sessionId?: string;
}) {
  const [open, setOpen] = useState(false);
  const [usageOpen, setUsageOpen] = useState(false);
  const setPendingInput = useStore((state) => state.setPendingInput);
  const hasEvidence = digest.items.length > 0 || digest.usage.length > 0 || Boolean(digest.delivery);
  const nextAction = deliveryNextAction(digest);
  if (!hasEvidence) return null;

  return (
    <ForgeCollapsible open={open} onOpenChange={setOpen}>
      <div data-testid="conversation-process-disclosure" className="forge-process-disclosure">
        <div className="forge-process-footer-row">
          <ForgeCollapsibleTrigger
            type="button"
            data-testid="conversation-process-trigger"
            aria-label={open ? "收起过程" : "查看过程"}
            className="forge-process-disclosure-trigger"
          >
            <span className="forge-process-disclosure-status">
              <span aria-hidden="true">✓</span>
              {" "}已完成{digest.operationCount > 0 ? ` · ${digest.operationCount} 项操作` : ""}
            </span>
            <span>{open ? "收起过程" : "查看过程"}</span>
            <span aria-hidden="true" data-open={open ? "true" : "false"} className="forge-process-disclosure-chevron">›</span>
          </ForgeCollapsibleTrigger>

          {nextAction && (
            <button
              type="button"
              data-testid="conversation-next-action"
              className="forge-process-next-action"
              onClick={() => setPendingInput(nextAction)}
            >
              {actionLabel(nextAction)}
            </button>
          )}
        </div>

        {open && (
          <ForgeCollapsibleContent>
            <div className="forge-process-disclosure-content">
              {digest.items.length > 0 && (
                <ol data-testid="conversation-process-timeline" className="forge-process-digest-list">
                  {digest.items.map((item) => (
                    <ConversationProcessItem key={item.id} item={item} sessionId={sessionId} />
                  ))}
                </ol>
              )}

              <DigestMetadata digest={digest} usageOpen={usageOpen} onUsageOpenChange={setUsageOpen} sessionId={sessionId} />
            </div>
          </ForgeCollapsibleContent>
        )}
      </div>
    </ForgeCollapsible>
  );
}

function DigestMetadata({
  digest,
  usageOpen,
  onUsageOpenChange,
  sessionId,
}: {
  digest: ProcessDigest;
  usageOpen: boolean;
  onUsageOpenChange: (open: boolean) => void;
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
        <ForgeCollapsible open={usageOpen} onOpenChange={onUsageOpenChange}>
          <ForgeCollapsibleTrigger
            type="button"
            aria-label={usageOpen ? "收起模型用量" : "查看模型用量"}
            className="forge-process-metadata-row forge-process-metadata-trigger"
          >
            <span>模型用量</span>
            <span>{usageSummary(usage.metadata)}</span>
          </ForgeCollapsibleTrigger>
          {usageOpen && (
            <ForgeCollapsibleContent>
              <div className="forge-process-detail-content forge-process-usage-detail">
                <MemoizedBlockRenderer block={usage} sessionId={sessionId} />
              </div>
            </ForgeCollapsibleContent>
          )}
        </ForgeCollapsible>
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

function deliveryNextAction(digest: ProcessDigest) {
  const summary = digest.delivery?.metadata.summary;
  return isRecord(summary) ? stringValue(summary.next_action) : null;
}

function actionLabel(action: string) {
  return action.length <= 18 ? action : "继续处理";
}
