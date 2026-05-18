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
      <button
        data-testid="thinking-trigger"
        data-state={isRunning ? "running" : "complete"}
        onClick={() => setOpen(!open)}
        className="forge-status-row forge-status-trigger"
      >
        <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
        <span>{isRunning ? "正在梳理思路" : "思考已收起"}</span>
        {isRunning ? (
          <span data-testid="thinking-dots" className="forge-status-dots">
            <span className="forge-status-dot" />
            <span className="forge-status-dot" style={{ animationDelay: "0.18s" }} />
            <span className="forge-status-dot" style={{ animationDelay: "0.36s" }} />
          </span>
        ) : null}
      </button>

      {open && (
        <div className="forge-status-detail">
          {block.content || "..."}
        </div>
      )}
    </div>
  );
}
