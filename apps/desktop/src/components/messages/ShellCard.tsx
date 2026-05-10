import { useState } from "react";
import { Terminal, ChevronDown, ChevronRight, Check, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { BlockState } from "@/lib/protocol";

interface ShellCardProps {
  block: BlockState;
}

export function ShellCard({ block }: ShellCardProps) {
  const [expanded, setExpanded] = useState(true);
  const command = (block.metadata?.command as string) || "";
  const exitCode = (block.metadata?.exit_code as number) ?? 0;
  const isComplete = block.isComplete;

  return (
    <div className="my-4 border border-border rounded-xl overflow-hidden bg-black/5 dark:bg-black/30">
      {/* Command header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-4 py-2.5 hover:bg-white/5 transition-colors text-left"
      >
        {expanded ? (
          <ChevronDown className="size-3.5 text-muted-foreground" />
        ) : (
          <ChevronRight className="size-3.5 text-muted-foreground" />
        )}
        <Terminal className="size-3.5 text-emerald-400" />
        <span className="text-xs font-mono text-emerald-400 flex-1 truncate">
          $ {command}
        </span>
        {isComplete && (
          <span
            className={cn(
              "flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full",
              exitCode === 0
                ? "bg-emerald-950/50 text-emerald-400"
                : "bg-red-950/50 text-red-400"
            )}
          >
            {exitCode === 0 ? (
              <Check className="size-3" />
            ) : (
              <X className="size-3" />
            )}
            exit {exitCode}
          </span>
        )}
        {!isComplete && (
          <span className="text-[10px] text-muted-foreground animate-pulse">
            running...
          </span>
        )}
      </button>

      {expanded && block.content && (
        <div className="border-t border-border/30">
          <pre className="p-4 text-xs font-mono text-[#c9d1d9] overflow-auto max-h-80 whitespace-pre-wrap break-all leading-relaxed">
            {block.content}
          </pre>
        </div>
      )}
    </div>
  );
}
