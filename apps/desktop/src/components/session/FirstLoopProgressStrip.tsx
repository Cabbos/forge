import type { BlockState } from "@/lib/protocol";
import type { FirstLoopDraft } from "@/lib/first-loop";
import { deriveFirstLoopProgress } from "@/lib/first-loop-progress";
import { cn } from "@/lib/utils";

interface FirstLoopProgressStripProps {
  blocks: BlockState[];
  draft: FirstLoopDraft | null;
}

export function FirstLoopProgressStrip({ blocks, draft }: FirstLoopProgressStripProps) {
  const phases = deriveFirstLoopProgress(blocks, draft);

  return (
    <div className="flex min-w-0 items-center justify-center gap-2 overflow-hidden border-b border-border px-3 py-1.5">
      {phases.map((phase, index) => (
        <div key={phase.id} className="flex min-w-0 items-center gap-2">
          <div
            className={cn(
              "flex min-w-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] transition-colors",
              phase.state === "active" && "bg-primary/10 text-primary",
              phase.state === "done" && "text-foreground/80",
              phase.state === "upcoming" && "text-muted-foreground/55",
            )}
          >
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                phase.state === "active" && "bg-primary",
                phase.state === "done" && "bg-emerald-400",
                phase.state === "upcoming" && "bg-muted-foreground/35",
              )}
            />
            <span className="truncate">{phase.label}</span>
          </div>
          {index < phases.length - 1 && (
            <span className="hidden h-px w-4 bg-border sm:block" />
          )}
        </div>
      ))}
    </div>
  );
}
