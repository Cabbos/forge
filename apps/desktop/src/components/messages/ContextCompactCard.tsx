import { useState } from "react";
import { Archive, ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ContextCompactCard({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(false);
  const compacted = numberMeta(block, "compacted_messages");
  const retained = numberMeta(block, "retained_messages");
  const before = numberMeta(block, "estimated_tokens_before");
  const after = numberMeta(block, "estimated_tokens_after");

  return (
    <div className="mb-3">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          className="inline-flex max-w-full items-center gap-2 rounded-md border px-3 py-2 text-xs transition-colors"
          style={{ background: "var(--card)", borderColor: "var(--border)", color: "#E4E7EC" }}
        >
          <ChevronRight className={cn("size-3 shrink-0 transition-transform", open && "rotate-90")} />
          <Archive className="size-3.5 shrink-0" style={{ color: "#7BA7D8" }} />
          <span className="shrink-0 font-medium">上下文已自动压缩</span>
          <span className="min-w-0 truncate" style={{ color: "var(--muted-foreground)" }}>
            {compacted} 条历史 · 保留 {retained} 条 · {formatTokens(before)} {"->"} {formatTokens(after)}
          </span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div
            className="mt-1.5 max-h-[220px] max-w-full overflow-auto rounded-md border p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap break-words"
            style={{ background: "var(--background)", borderColor: "var(--border)", color: "#D0D5DD" }}
          >
            {block.content}
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
