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
          {isRunning ? "思考中" : "思考记录"}
        </span>
        {isRunning ? (
          <span className="flex gap-1">
            <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#4B9CD3" }} />
            <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#4B9CD3", animationDelay: "0.2s" }} />
            <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#4B9CD3", animationDelay: "0.4s" }} />
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
