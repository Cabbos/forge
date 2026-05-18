import { useEffect, useState } from "react";
import { Check, CheckCircle2, ChevronRight, Copy, Loader2, Terminal } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { cn } from "@/lib/utils";

export function ShellCard({ block }: { block: BlockState }) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const exitCode = block.metadata.exit_code as number | undefined;
  const isError = exitCode !== undefined && exitCode !== 0;
  const isRunning = !block.isComplete;
  const command = (block.metadata.command as string) || "命令";
  const output = block.content || "";
  const outputSections = parseShellOutput(output, isError);

  useEffect(() => {
    if (block.isComplete && isError) setExpanded(true);
  }, [block.isComplete, isError]);

  const copyOutput = async () => {
    await navigator.clipboard?.writeText(output);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div>
      <Collapsible open={expanded} onOpenChange={setExpanded}>
        <CollapsibleTrigger
          data-testid="shell-card-trigger"
          data-state={isRunning ? "running" : isError ? "error" : "done"}
          className="forge-log-line"
          data-tone={isError ? "error" : "default"}
        >
          <ChevronRight className={cn("size-3 shrink-0 transition-transform", expanded && "rotate-90")} />
          <Terminal className="size-3 shrink-0" />
          <span className="min-w-0 truncate font-mono">{command}</span>
          {isRunning ? (
            <span
              className="forge-log-status"
              data-tone="running"
              title="运行中"
            >
              <Loader2 className="size-3 animate-spin" />
            </span>
          ) : (
            <span
              data-testid={isError ? "shell-exit-code" : undefined}
              className="forge-log-status"
              data-tone={isError ? "error" : "success"}
              title={isError ? `退出码 ${exitCode}` : "完成"}
            >
              {isError ? `exit ${exitCode}` : <CheckCircle2 className="size-3" />}
            </span>
          )}
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div data-testid="log-detail-surface" className="forge-log-detail">
            <div data-testid="log-detail-header" className="forge-log-detail-header">
              <span className="min-w-0 truncate font-mono text-[10px] text-muted-foreground/75">{command}</span>
              <button
                type="button"
                aria-label={copied ? "已复制命令输出" : "复制命令输出"}
                title={copied ? "已复制" : "复制命令输出"}
                onClick={copyOutput}
                disabled={!output}
                className="forge-log-action disabled:cursor-default disabled:opacity-45"
              >
                {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
              </button>
            </div>
            <div data-testid="log-detail-output" className="forge-log-output">
              {outputSections.map((section, index) => (
                <div key={`${section.label}-${index}`} data-testid="shell-output-section" className="forge-shell-output-section" data-tone={section.label === "stderr" ? "error" : "default"}>
                  <div className="forge-shell-output-label">{section.label}</div>
                  <pre>{section.content || " "}</pre>
                </div>
              ))}
            </div>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

function parseShellOutput(output: string, isError: boolean) {
  const lines = output.split("\n");
  const sections: Array<{ label: string; content: string }> = [];
  let currentLabel: string | null = null;
  let currentLines: string[] = [];

  const flush = () => {
    if (!currentLabel) return;
    sections.push({ label: currentLabel, content: currentLines.join("\n").trimEnd() });
    currentLines = [];
  };

  for (const line of lines) {
    const match = line.match(/^(stdout|stderr):\s*$/i);
    if (match) {
      flush();
      currentLabel = match[1].toLowerCase();
      continue;
    }
    if (!currentLabel) currentLabel = isError ? "output" : "stdout";
    currentLines.push(line);
  }
  flush();

  return sections.length ? sections : [{ label: isError ? "output" : "stdout", content: output }];
}
