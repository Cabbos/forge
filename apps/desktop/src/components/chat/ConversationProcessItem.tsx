import { useState } from "react";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import type { ProcessDigestItem } from "@/components/chat/conversationTurnView";
import {
  ForgeCollapsible,
  ForgeCollapsibleContent,
  ForgeCollapsibleTrigger,
} from "@/components/primitives/collapsible";

export function ConversationProcessItem({
  item,
  sessionId,
}: {
  item: ProcessDigestItem;
  sessionId?: string;
}) {
  const [detailsOpen, setDetailsOpen] = useState(false);
  const canShowDetails = item.evidence.length > 0;

  return (
    <li data-testid="conversation-process-item" data-process-kind={item.kind} className="forge-process-digest-item">
      <div className="forge-process-digest-row">
        <span aria-hidden="true" className="forge-process-digest-node" data-outcome={item.outcome} />
        <span className="forge-process-digest-label">{item.label}</span>
        <span className="forge-process-digest-outcome">{outcomeLabel(item.outcome)}</span>
      </div>

      {canShowDetails && (
        <ForgeCollapsible open={detailsOpen} onOpenChange={setDetailsOpen}>
          <ForgeCollapsibleTrigger
            type="button"
            aria-label={`${detailsOpen ? "收起" : "查看"}${item.label}运行证据`}
            className="forge-process-detail-trigger"
          >
            <span aria-hidden="true" data-open={detailsOpen ? "true" : "false"}>›</span>
            {detailsOpen ? "收起证据" : "查看证据"}
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
  if (outcome === "stopped") return "已停止";
  if (outcome === "running") return "进行中";
  return "完成";
}
