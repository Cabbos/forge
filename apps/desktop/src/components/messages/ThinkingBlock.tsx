import { useState } from "react";
import { ChevronRight, BrainCircuit } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";

export function ThinkingBlock({ block }: { block: BlockState }) {
  const [open, setOpen] = useState(false);
  if (!block.content && block.isComplete) return null;

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="mb-5">
      <div className="bg-muted/20 border border-border/10 rounded-xl overflow-hidden transition-all duration-200">
        <CollapsibleTrigger className="w-full flex items-center gap-2.5 px-4 py-2.5 text-sm text-muted-foreground hover:bg-muted/30 transition-colors duration-200 group rounded-xl">
          <ChevronRight className={`size-3.5 shrink-0 transition-transform duration-200 ${open ? "rotate-90" : ""}`} />
          <BrainCircuit className="size-3.5 shrink-0 text-primary/50" />
          <span className="text-xs font-medium tracking-wide uppercase">Thinking</span>
          {!block.isComplete && (
            <span className="ml-auto flex gap-1">
              <span className="size-1.5 rounded-full bg-amber-400/60 animate-pulse" />
              <span className="size-1.5 rounded-full bg-amber-400/60 animate-pulse [animation-delay:200ms]" />
              <span className="size-1.5 rounded-full bg-amber-400/60 animate-pulse [animation-delay:400ms]" />
            </span>
          )}
          {block.isComplete && <span className="text-[10px] text-muted-foreground/40 ml-auto font-normal">done</span>}
        </CollapsibleTrigger>
        <CollapsibleContent className="overflow-hidden data-[panel-open]:animate-[collapsible-down_200ms_ease-out] data-[panel-closed]:animate-[collapsible-up_200ms_ease-out]">
          <div className="px-4 pb-4 border-t border-border/10">
            <div className="mt-3 text-sm text-muted-foreground/70 whitespace-pre-wrap leading-relaxed">{block.content || "Thinking..."}</div>
          </div>
        </CollapsibleContent>
      </div>
    </Collapsible>
  );
}
