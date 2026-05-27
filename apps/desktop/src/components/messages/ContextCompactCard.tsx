import { useState } from "react";
import { Archive, ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ContextCompactCard({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(false);
  const compacted = numberMeta(block, "compacted_messages");
  const retained = numberMeta(block, "retained_messages");
  const before = numberMeta(block, "estimated_tokens_before");
  const after = numberMeta(block, "estimated_tokens_after");

  return (
    <div className="compact-spool">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          data-testid="context-compact-trigger"
          className="forge-log-line forge-context-compact-trigger"
        >
          <ChevronRight className={cn("size-3 shrink-0 transition-transform", open && "rotate-90")} />
          <ForgeIcon icon={Archive} tone="context" contained={false} className="size-3.5" />
          <span className="shrink-0 font-medium">上下文已自动整理</span>
          <span className="min-w-0 truncate" style={{ color: "var(--muted-foreground)" }}>
            {compacted} 条历史 · 保留 {retained} 条 · {formatTokens(before)} {"->"} {formatTokens(after)}
          </span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div
            data-testid="log-detail-surface"
            className="forge-log-detail"
          >
            <div data-testid="log-detail-output" className="forge-log-output">
              {block.content}
            </div>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

function numberMeta(block: BlockState, key: string) {
  const value = block.metadata[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function formatTokens(value: number) {
  if (value >= 1_000_000) {
    const amount = value / 1_000_000;
    return `${Number.isInteger(amount) ? amount.toFixed(0) : amount.toFixed(1)}M`;
  }
  if (value >= 1_000) return `${Math.round(value / 1_000)}K`;
  return `${value}`;
}
