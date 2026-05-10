import { useState } from "react";
import type { BlockState } from "../../lib/protocol";

interface ThinkingBlockProps {
  block: BlockState;
}

export function ThinkingBlock({ block }: ThinkingBlockProps) {
  const [expanded, setExpanded] = useState(false);

  if (!block.content && block.isComplete) return null;

  return (
    <div className="border border-brand/30 rounded-lg overflow-hidden bg-brand/[0.02]">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs text-text-secondary
                   hover:bg-surface-hover transition-colors"
        aria-expanded={expanded}
      >
        <span className="text-brand">{expanded ? "v" : ">"}</span>
        <span className="font-medium">Thinking</span>
        {!block.isComplete && (
          <span className="inline-block w-2 h-2 rounded-full bg-yellow-500 animate-pulse ml-auto" />
        )}
      </button>

      {expanded && (
        <div className="px-3 pb-3 border-t border-border">
          <p className="text-sm text-text-secondary mt-2 whitespace-pre-wrap leading-relaxed">
            {block.content || "Thinking..."}
          </p>
        </div>
      )}
    </div>
  );
}
