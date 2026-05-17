import { useEffect, useState } from "react";
import { Check, CheckCircle2, ChevronRight, Copy, Terminal } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ShellCard({ block }: { block: BlockState }) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const exitCode = block.metadata.exit_code as number | undefined;
  const isError = exitCode !== undefined && exitCode !== 0;
  const command = (block.metadata.command as string) || "命令";
  const output = block.content || "";

  useEffect(() => {
    if (block.isComplete && isError) setExpanded(true);
  }, [block.isComplete, isError]);

  const copyOutput = async () => {
    await navigator.clipboard?.writeText(output);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div className="mb-2">
      <Collapsible open={expanded} onOpenChange={setExpanded}>
        <CollapsibleTrigger
          data-testid="shell-card-trigger"
          className="inline-flex max-w-full items-center gap-2 rounded-md border px-2 py-1.5 text-xs transition-colors hover:border-border hover:bg-secondary/20"
          style={{ background: "transparent", borderColor: isError ? "rgba(212,119,119,0.34)" : "rgba(148,163,184,0.18)", color: "var(--muted-foreground)" }}
        >
          <ChevronRight className={cn("size-3 shrink-0 transition-transform", expanded && "rotate-90")} />
          <Terminal className="size-3 shrink-0" />
          <span className="min-w-0 truncate font-mono">{command}</span>
          {block.isComplete && (
            <span className="shrink-0" style={{ color: isError ? "#D47777" : "#4A9E6B", fontSize: "10px" }} title={isError ? `退出码 ${exitCode}` : "完成"}>
              {isError ? `退出码 ${exitCode}` : <CheckCircle2 className="size-3" />}
            </span>
          )}
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div className="mt-1.5 overflow-hidden rounded-md border" style={{ borderColor: "var(--border)" }}>
            <div className="flex items-center justify-between border-b px-3 py-2" style={{ borderColor: "var(--border)" }}>
              <span className="min-w-0 truncate font-mono text-[10px] text-muted-foreground/75">{command}</span>
              <button
                type="button"
                aria-label={copied ? "已复制命令输出" : "复制命令输出"}
                title={copied ? "已复制" : "复制命令输出"}
                onClick={copyOutput}
                disabled={!output}
                className="inline-flex size-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:cursor-default disabled:opacity-45"
              >
                {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
              </button>
            </div>
            <pre className="p-3 text-xs font-mono whitespace-pre-wrap break-all overflow-auto" style={{ color: "#D0D5DD", maxHeight: "300px", background: "var(--background)" }}>
              {output}
            </pre>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
