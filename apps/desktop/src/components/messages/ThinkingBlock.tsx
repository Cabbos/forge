import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ThinkingBlock({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(!block.isComplete);
  if (!block.content && block.isComplete) return null;

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="mb-3" style={{ paddingLeft: "40px" }}>
      <div>
        <CollapsibleTrigger className="flex items-center gap-2 text-[10px] uppercase tracking-wider cursor-pointer mb-1.5"
          style={{ color: "#777" }}>
          <ChevronRight className={cn("size-3 transition-transform duration-200", open && "rotate-90")} />
          Thinking
          {!block.isComplete ? (
            <span className="flex gap-1 ml-1">
              <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite]" style={{ background: "#D4A853" }} />
              <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite_0.2s]" style={{ background: "#D4A853", animationDelay: "0.2s" }} />
              <span className="inline-block w-1 h-1 rounded-full animate-[pulse-dot_1s_infinite_0.4s]" style={{ background: "#D4A853", animationDelay: "0.4s" }} />
            </span>
          ) : (
            <span className="ml-1 text-[9px] normal-case text-muted-foreground/40">done</span>
          )}
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div className="text-xs leading-relaxed whitespace-pre-wrap border-l-2 border-[#222] pl-3.5 py-1"
            style={{ color: "#888" }}>
            {block.content || "..."}
          </div>
          {!block.isComplete && (
            <div className="h-px mt-2 overflow-hidden rounded-full" style={{ background: "#181818" }}>
              <div className="h-full w-1/3 rounded-full animate-[shimmer_1.5s_ease-in-out_infinite]"
                style={{ background: "linear-gradient(90deg, transparent, rgba(212,168,83,0.2), transparent)" }} />
            </div>
          )}
        </CollapsibleContent>
      </div>
    </Collapsible>
  );
}
