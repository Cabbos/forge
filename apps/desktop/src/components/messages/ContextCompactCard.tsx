import { useState } from "react";
import { Archive, ChevronRight } from "lucide-react";
import { ForgeCollapsible, ForgeCollapsibleContent, ForgeCollapsibleTrigger } from "@/components/primitives/collapsible";
import { ForgeIcon } from "@/components/primitives/icon";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ContextCompactCard({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(false);
  const skipped = block.event_type === "context_compact_skipped";
  const compacted = numberMeta(block, "compacted_messages");
  const retained = numberMeta(block, "retained_messages");
  const before = numberMeta(block, "estimated_tokens_before");
  const after = numberMeta(block, "estimated_tokens_after");
  const reason = stringMeta(block, "reason");

  return (
    <div className="compact-spool">
      <ForgeCollapsible open={open} onOpenChange={setOpen}>
        <ForgeCollapsibleTrigger
          data-testid="context-compact-trigger"
          className="forge-log-line forge-context-compact-trigger"
        >
          <ChevronRight className={cn("size-3 shrink-0 transition-transform", open && "rotate-90")} />
          <ForgeIcon icon={Archive} tone="context" contained={false} className="size-3.5" />
          <span className="shrink-0 font-medium">{skipped ? "上下文无需压缩" : "上下文已自动整理"}</span>
          <span className="compact-spool-meta min-w-0 truncate">
            {skipped
              ? `保留 ${retained} 条 · ${compactReasonLabel(reason)}`
              : `${compacted} 条历史 · 保留 ${retained} 条 · ${formatTokens(before)} -> ${formatTokens(after)}`}
          </span>
        </ForgeCollapsibleTrigger>
        <ForgeCollapsibleContent>
          <div
            data-testid="log-detail-surface"
            className="forge-log-detail"
          >
            <div data-testid="log-detail-output" className="forge-log-output">
              {block.content}
            </div>
          </div>
        </ForgeCollapsibleContent>
      </ForgeCollapsible>
    </div>
  );
}

function numberMeta(block: BlockState, key: string) {
  const value = block.metadata[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function stringMeta(block: BlockState, key: string) {
  const value = block.metadata[key];
  return typeof value === "string" ? value : "";
}

function compactReasonLabel(reason: string) {
  switch (reason) {
    case "history_too_short":
    case "too_few_messages_to_compact":
      return "历史较短";
    case "no_safe_retention_boundary":
      return "等待安全边界";
    default:
      return "已保留原上下文";
  }
}

function formatTokens(value: number) {
  if (value >= 1_000_000) {
    const amount = value / 1_000_000;
    return `${Number.isInteger(amount) ? amount.toFixed(0) : amount.toFixed(1)}M`;
  }
  if (value >= 1_000) return `${Math.round(value / 1_000)}K`;
  return `${value}`;
}
