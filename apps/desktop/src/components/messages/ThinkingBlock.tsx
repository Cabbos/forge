import { useState } from "react";
import { ChevronRight } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ThinkingBlock({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(false);
  if (!block.content && block.isComplete) return null;

  const isRunning = !block.isComplete;

  return (
    <div className="mb-1">
      {/* Inline header */}
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 text-xs cursor-pointer select-none text-left"
        style={{ color: "#888" }}>
        <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
        <span className="uppercase tracking-wider text-[10px]" style={{ color: "#777" }}>
          {isRunning ? "Thinking" : "Thought"}
        </span>
        {isRunning ? (
          <span className="flex gap-1">
            <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#4B9CD3" }} />
            <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#4B9CD3", animationDelay: "0.2s" }} />
            <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#4B9CD3", animationDelay: "0.4s" }} />
          </span>
        ) : (
          <span style={{ color: "#4A9E6B", fontSize: "10px" }}>✓</span>
        )}
      </button>

      {/* Content */}
      {open && (
        <div className="pl-3.5 py-1 border-l-2 mt-1 text-xs leading-relaxed whitespace-pre-wrap"
          style={{ borderColor: "#1c1c1c", color: "#777" }}>
          {block.content || "..."}
        </div>
      )}
    </div>
  );
}
