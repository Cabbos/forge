import { useState } from "react";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import { deriveConversationProcessTarget } from "@/components/chat/conversationProcessTarget";
import type { ProcessDigestItem } from "@/components/chat/conversationTurnView";
import {
  ForgeCollapsible,
  ForgeCollapsibleContent,
  ForgeCollapsibleTrigger,
} from "@/components/primitives/collapsible";
import { openWorkPanelTabInLayout } from "@/components/workpanel/workPanelEvents";

export function ConversationProcessItem({
  item,
  sessionId,
}: {
  item: ProcessDigestItem;
  sessionId?: string;
}) {
  const [detailsOpen, setDetailsOpen] = useState(false);
  const canShowDetails = item.kind !== "understanding" && item.evidence.length > 0;
  const target = deriveConversationProcessTarget(item);

  return (
    <li data-testid="conversation-process-item" data-process-kind={item.kind} className="forge-process-digest-item">
      <div className="forge-process-digest-row">
        <span aria-hidden="true" className="forge-process-digest-node" data-outcome={item.outcome} />
        <span className="forge-process-digest-label">{item.label}</span>
        <span className="forge-process-digest-outcome">{outcomeLabel(item.outcome)}</span>
        {item.durationMs !== null && (
          <span className="forge-process-digest-duration">{formatDuration(item.durationMs)}</span>
        )}
        {target && (
          <button
            type="button"
            aria-label={target.accessibleLabel}
            className="forge-process-target-trigger"
            onClick={() => openWorkPanelTabInLayout(target.tab)}
          >
            打开
          </button>
        )}
      </div>

      {canShowDetails && (
        <ForgeCollapsible open={detailsOpen} onOpenChange={setDetailsOpen}>
          <ForgeCollapsibleTrigger
            type="button"
            aria-label={`${detailsOpen ? "收起" : "查看"} ${item.label} 详情`}
            className="forge-process-detail-trigger"
          >
            <span aria-hidden="true" data-open={detailsOpen ? "true" : "false"}>›</span>
            {detailsOpen ? "收起详情" : "查看详情"}
          </ForgeCollapsibleTrigger>
          {detailsOpen && (
            <ForgeCollapsibleContent>
              <div data-testid="conversation-process-details" className="forge-process-detail-content">
                {item.evidence.map((block, index) => (
                  <MemoizedBlockRenderer
                    key={`${block.block_id}-${block.event_type}-${index}`}
                    block={block}
                    sessionId={sessionId}
                  />
                ))}
              </div>
            </ForgeCollapsibleContent>
          )}
        </ForgeCollapsible>
      )}
    </li>
  );
}

function outcomeLabel(outcome: ProcessDigestItem["outcome"]) {
  if (outcome === "failed") return "失败";
  if (outcome === "running") return "进行中";
  return "完成";
}

function formatDuration(durationMs: number) {
  if (durationMs < 1_000) return `${Math.round(durationMs)}ms`;
  return `${(durationMs / 1_000).toFixed(durationMs < 10_000 ? 1 : 0)}s`;
}
