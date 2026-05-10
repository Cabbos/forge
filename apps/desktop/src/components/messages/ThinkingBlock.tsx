import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";

export function ThinkingBlock({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(!block.isComplete); // auto-expand while streaming
  if (!block.content && block.isComplete) return null;

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="flex gap-3">
        {/* Avatar */}
        <div className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5"
          style={{ background: "rgba(212,168,83,0.12)", color: "#D4A853", fontSize: "0.65rem", fontWeight: 700 }}>
          A
        </div>

        <div className="flex-1 min-w-0">
          {/* Header */}
          <CollapsibleTrigger className="flex items-center gap-2 text-[10px] uppercase tracking-wider cursor-pointer" style={{ color: "#777" }}>
            <ChevronRight className={`size-3 transition-transform duration-200 ${open ? "rotate-90" : ""}`} />
            <span>Thinking</span>
            {!block.isComplete ? (
              <span className="flex gap-1 ml-1">
                <span className="inline-block w-1.5 h-1.5 rounded-full animate-pulse" style={{ background: "#D4A853" }} />
                <span className="inline-block w-1.5 h-1.5 rounded-full animate-pulse" style={{ background: "#D4A853", animationDelay: "200ms" }} />
                <span className="inline-block w-1.5 h-1.5 rounded-full animate-pulse" style={{ background: "#D4A853", animationDelay: "400ms" }} />
              </span>
            ) : (
              <span className="ml-1 text-[9px] normal-case" style={{ color: "#555" }}>done</span>
            )}
          </CollapsibleTrigger>

          {/* Content */}
          <CollapsibleContent>
            <div className="mt-2 pl-1">
              {block.isComplete ? (
                <div className="text-sm leading-relaxed whitespace-pre-wrap" style={{ color: "#888" }}>
                  {block.content}
                </div>
              ) : (
                <div className="relative">
                  <div className="text-sm leading-relaxed whitespace-pre-wrap" style={{ color: "#888" }}>
                    {block.content || "..."}
                  </div>
                  {/* Shimmer bar at bottom while streaming */}
                  <div className="h-px mt-2 overflow-hidden rounded-full" style={{ background: "#1c1c1c" }}>
                    <div className="h-full w-1/3 rounded-full animate-[shimmer_1.5s_ease-in-out_infinite]"
                      style={{ background: "linear-gradient(90deg, transparent, rgba(212,168,83,0.4), transparent)" }} />
                  </div>
                </div>
              )}
            </div>
          </CollapsibleContent>
        </div>
      </div>
    </Collapsible>
  );
}
