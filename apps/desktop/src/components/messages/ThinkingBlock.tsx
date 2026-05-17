import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ThinkingBlock({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(false);
  if (!block.content && block.isComplete) return null;

  const isRunning = !block.isComplete;

  return (
    <div>
      {/* Inline header */}
      <button
        data-testid="thinking-trigger"
        onClick={() => setOpen(!open)}
        className="inline-flex items-center gap-1.5 py-1 text-xs cursor-pointer select-none text-left"
        style={{ color: "var(--muted-foreground)" }}>
        <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
        <span className="text-[11px]" style={{ color: "var(--muted-foreground)" }}>
          {isRunning ? "正在梳理思路" : "思考记录"}
        </span>
        {isRunning ? (
          <span data-testid="thinking-dots" className="flex gap-1">
            <span className="inline-block h-1 w-1 rounded-full animate-[pulse-dot_1.15s_infinite]" style={{ background: "currentColor" }} />
            <span className="inline-block h-1 w-1 rounded-full animate-[pulse-dot_1.15s_infinite]" style={{ background: "currentColor", animationDelay: "0.18s" }} />
            <span className="inline-block h-1 w-1 rounded-full animate-[pulse-dot_1.15s_infinite]" style={{ background: "currentColor", animationDelay: "0.36s" }} />
          </span>
        ) : null}
      </button>

      {/* Content */}
      {open && (
        <div className="pl-3.5 py-1 border-l-2 mt-1 text-xs leading-relaxed whitespace-pre-wrap"
          style={{ borderColor: "var(--border)", color: "#D0D5DD" }}>
          {block.content || "..."}
        </div>
      )}
    </div>
  );
}
