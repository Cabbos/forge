import { useEffect, useState } from "react";
import { ChevronRight, Terminal } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ShellCard({ block }: { block: BlockState }) {
  const [expanded, setExpanded] = useState(false);
  const exitCode = block.metadata.exit_code as number | undefined;
  const isError = exitCode !== undefined && exitCode !== 0;

  useEffect(() => {
    if (block.isComplete && isError) setExpanded(true);
  }, [block.isComplete, isError]);

  return (
    <div className="mb-3">
      <Collapsible open={expanded} onOpenChange={setExpanded}>
        <div className="rounded-md overflow-hidden border" style={{ borderColor: "#181818" }}>
          <CollapsibleTrigger className="w-full flex items-center gap-2.5 px-3 py-2 text-xs transition-colors"
            style={{ background: "#0a0a0a", color: "#888" }}>
            <ChevronRight className={cn("size-3 transition-transform", expanded && "rotate-90")} />
            <Terminal className="size-3" />
            <span className="font-mono truncate">{(block.metadata.command as string) || "shell"}</span>
            {block.isComplete && (
              <span className="ml-auto" style={{ color: isError ? "#D47777" : "#4A9E6B", fontSize: "10px" }}>
                {isError ? `exit ${exitCode}` : "ok"}
              </span>
            )}
          </CollapsibleTrigger>
          <CollapsibleContent>
            <pre className="p-3 text-xs font-mono whitespace-pre-wrap break-all overflow-auto" style={{ color: "#999", maxHeight: "300px", background: "#060606" }}>
              {block.content}
            </pre>
          </CollapsibleContent>
        </div>
      </Collapsible>
    </div>
  );
}
