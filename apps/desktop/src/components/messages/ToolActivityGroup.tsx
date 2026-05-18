import { useEffect, useState } from "react";
import { CheckCircle2, ChevronRight, Loader2, XCircle } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ShellCard } from "@/components/messages/ShellCard";
import { ToolCallCard } from "@/components/messages/ToolCallCard";
import { cn } from "@/lib/utils";

export function ToolActivityGroup({ blocks }: { blocks: BlockState[] }) {
  const hasError = blocks.some((block) => {
    if (block.event_type === "shell") {
      const exitCode = block.metadata.exit_code as number | undefined;
      return exitCode !== undefined && exitCode !== 0;
    }
    return Boolean(block.metadata.is_error ?? false);
  });
  const [open, setOpen] = useState(hasError);
  const activitySummary = summarizeActivity(blocks);
  const isRunning = blocks.some((block) => !block.isComplete);
  const label = hasError
    ? `处理遇到问题 · ${blocks.length} 步`
    : isRunning
      ? `正在处理 · ${blocks.length} 步`
      : `过程已收起 · ${blocks.length} 步`;
  const StatusIcon = hasError ? XCircle : isRunning ? Loader2 : CheckCircle2;

  useEffect(() => {
    if (hasError) setOpen(true);
  }, [hasError]);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div data-testid="tool-activity-group" className="forge-tool-activity-group" data-tone={hasError ? "error" : "default"}>
        <CollapsibleTrigger
          data-testid="tool-activity-summary"
          data-state={hasError ? "error" : isRunning ? "running" : "done"}
          className="forge-tool-activity-summary"
        >
          <ChevronRight className={cn("size-3 shrink-0 transition-transform", open && "rotate-90")} />
          <StatusIcon
            data-running-icon={isRunning ? "true" : undefined}
            className={cn("size-3.5 shrink-0", isRunning && "animate-spin")}
          />
          <span className="shrink-0 font-medium">{label}</span>
          {activitySummary.map((item) => (
            <span key={item} className="forge-tool-activity-summary-item">{item}</span>
          ))}
        </CollapsibleTrigger>
        {open && (
          <CollapsibleContent>
            <div className="forge-tool-activity-list">
              {blocks.map((block) => {
                if (block.event_type === "shell") {
                  return <ShellCard key={block.block_id} block={block} />;
                }
                return <ToolCallCard key={block.block_id} block={block} />;
              })}
            </div>
          </CollapsibleContent>
        )}
      </div>
    </Collapsible>
  );
}

function summarizeActivity(blocks: BlockState[]) {
  const counts = blocks.reduce(
    (summary, block) => {
      if (block.event_type === "shell") {
        const command = String(block.metadata.command ?? "");
        if (/(build|test|check|lint)/i.test(command)) summary.checks += 1;
        else summary.commands += 1;
        return summary;
      }

      const toolName = String(block.metadata.tool_name ?? "");
      if (["read_file", "read"].includes(toolName)) summary.reads += 1;
      else if (["write_file", "edit"].includes(toolName)) summary.writes += 1;
      else if (["search_content", "grep", "search_files", "glob"].includes(toolName)) summary.searches += 1;
      else summary.tools += 1;
      return summary;
    },
    { reads: 0, writes: 0, searches: 0, checks: 0, commands: 0, tools: 0 },
  );

  return [
    counts.reads ? `查看 ${counts.reads} 个文件` : "",
    counts.writes ? `修改 ${counts.writes} 个文件` : "",
    counts.searches ? `搜索 ${counts.searches} 次` : "",
    counts.checks ? `运行 ${counts.checks} 次检查` : "",
    counts.commands ? `运行 ${counts.commands} 个命令` : "",
    counts.tools ? `调用 ${counts.tools} 个工具` : "",
  ].filter(Boolean);
}
