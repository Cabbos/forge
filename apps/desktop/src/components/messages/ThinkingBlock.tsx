import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";
import { ProcessStatusDots } from "./ProcessStatusDots";

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
        {isRunning ? <ProcessStatusDots testId="thinking-dots" /> : null}
      </button>

      {open && (
        <div className="forge-status-detail">
          {block.content || "..."}
        </div>
      )}
    </div>
  );
}
