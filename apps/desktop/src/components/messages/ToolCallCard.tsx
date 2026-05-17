import { useState, useEffect } from "react";
import { Check, ChevronRight, Copy, Loader2, CheckCircle2, XCircle, Wrench } from "lucide-react";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import type { BlockState } from "@/lib/protocol";
import { SubAgentTrace } from "@/components/messages/SubAgentTrace";
import { cn } from "@/lib/utils";

const TOOL_COPY: Record<string, { label: string; done: string; running: string }> = {
  read_file: { label: "读取文件", running: "正在读取文件", done: "已读取文件" },
  read: { label: "读取文件", running: "正在读取文件", done: "已读取文件" },
  write_file: { label: "修改文件", running: "正在修改文件", done: "已修改文件" },
  edit: { label: "修改文件", running: "正在修改文件", done: "已修改文件" },
  search_content: { label: "搜索内容", running: "正在搜索内容", done: "已搜索内容" },
  grep: { label: "搜索内容", running: "正在搜索内容", done: "已搜索内容" },
  search_files: { label: "查找文件", running: "正在查找文件", done: "已查找文件" },
  glob: { label: "查找文件", running: "正在查找文件", done: "已查找文件" },
  git_diff: { label: "查看改动", running: "正在查看改动", done: "已整理改动" },
  run_shell: { label: "运行命令", running: "正在运行命令", done: "命令已完成" },
  bash: { label: "运行命令", running: "正在运行命令", done: "命令已完成" },
  execute_command: { label: "运行命令", running: "正在运行命令", done: "命令已完成" },
  shell: { label: "运行命令", running: "正在运行命令", done: "命令已完成" },
  web_fetch: { label: "读取网页", running: "正在读取网页", done: "已读取网页" },
  web_search: { label: "搜索网页", running: "正在搜索网页", done: "已搜索网页" },
  delegate_task: { label: "分派任务", running: "正在分派任务", done: "任务已返回" },
};

export function ToolCallCard({ block }: { block: BlockState }) {
  const isError = Boolean(block.metadata.is_error ?? false);
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  // Keep normal tool chatter compact; only surface errors automatically.
  useEffect(() => {
    if (block.isComplete && isError) setOpen(true);
  }, [block.isComplete, isError]);
  const toolName = (block.metadata.tool_name as string) || "tool";
  const toolInput = block.metadata.tool_input;
  const status = block.isComplete ? (isError ? "error" : "done") : "running";
  const toolCopy = TOOL_COPY[toolName] ?? { label: toolName, running: `正在执行 ${toolName}`, done: `${toolName} 已完成` };
  const actionText = status === "running" ? toolCopy.running : status === "error" ? `${toolCopy.label}遇到问题` : toolCopy.done;
  const inputSummary = summarizeToolInput(toolName, toolInput);
  const detailText = block.content || (status === "running" ? "等待工具返回结果..." : "");
  const durationMs = typeof block.metadata.duration_ms === "number" ? block.metadata.duration_ms : null;
  const durationLabel = block.isComplete && durationMs !== null ? formatDuration(durationMs) : "";

  const StatusIcon = { running: Loader2, done: CheckCircle2, error: XCircle }[status];
  const statusColor = { running: "#D4A853", done: "#4A9E6B", error: "#D47777" }[status];
  const copyDetails = async () => {
    await navigator.clipboard?.writeText(detailText);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div className="mb-2">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger
          data-testid="tool-card-trigger"
          className="inline-flex max-w-full items-center gap-2 rounded-md border px-2 py-1.5 text-xs transition-colors hover:border-border hover:bg-secondary/20"
          style={{ background: "transparent", borderColor: isError ? "rgba(212,119,119,0.34)" : "rgba(148,163,184,0.18)", color: "#D8DBE2" }}>
          <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
          <Wrench className="size-3.5 shrink-0" style={{ color: statusColor }} />
          <span className="shrink-0 font-medium">{actionText}</span>
          {inputSummary && (
            <span className="min-w-0 truncate font-mono text-[11px]" style={{ color: "var(--muted-foreground)" }}>
              {inputSummary}
            </span>
          )}
          {durationLabel && (
            <span className="ml-auto shrink-0 font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>
              {durationLabel}
            </span>
          )}
          <span className={cn("flex shrink-0 items-center", !durationLabel && "ml-auto")} style={{ color: statusColor }} title={status === "running" ? "进行中" : status === "error" ? "异常" : "完成"}>
            <StatusIcon className={cn("size-3", status === "running" && "animate-spin")} />
          </span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          {toolName === "delegate_task" ? (
            <SubAgentTrace content={block.content} />
          ) : (
            <div className="mt-1.5 max-w-full overflow-hidden rounded-md border"
              style={{ background: "var(--background)", borderColor: "var(--border)" }}>
              <div className="flex items-center justify-between border-b px-3 py-2" style={{ borderColor: "var(--border)" }}>
                <span className="text-[11px]" style={{ color: "var(--muted-foreground)" }}>技术细节</span>
                <div className="flex items-center gap-2">
                  <span className="font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>{toolName}</span>
                  <button
                    type="button"
                    aria-label={copied ? "已复制工具输出" : "复制工具输出"}
                    title={copied ? "已复制" : "复制工具输出"}
                    onClick={copyDetails}
                    disabled={!detailText}
                    className="inline-flex size-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:cursor-default disabled:opacity-45"
                  >
                    {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
                  </button>
                </div>
              </div>
              <div className="max-h-[220px] overflow-auto p-3 font-mono text-xs whitespace-pre-wrap break-all"
                style={{ color: "#D0D5DD" }}>
                {detailText}
              </div>
            </div>
          )}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

function summarizeToolInput(toolName: string, input: unknown) {
  if (!input || typeof input !== "object") return "";
  const data = input as Record<string, unknown>;
  const pick = (...keys: string[]) => keys.map((key) => data[key]).find((value) => typeof value === "string" && value.trim()) as string | undefined;

  if (["read_file", "read", "write_file", "edit"].includes(toolName)) {
    return compactPath(pick("path", "file_path", "filename") ?? "");
  }

  if (["search_content", "grep"].includes(toolName)) {
    const pattern = pick("pattern", "query") ?? "";
    const path = pick("path") ?? "";
    return [pattern && `"${pattern}"`, path && compactPath(path)].filter(Boolean).join(" · ");
  }

  if (["search_files", "glob"].includes(toolName)) {
    return pick("pattern", "query") ?? compactPath(pick("path") ?? "");
  }

  if (["run_shell", "bash", "execute_command", "shell"].includes(toolName)) {
    return truncateMiddle(pick("command", "cmd") ?? "", 72);
  }

  if (toolName === "git_diff") {
    return compactPath(pick("path") ?? "当前改动");
  }

  if (["web_fetch", "web_search"].includes(toolName)) {
    return truncateMiddle(pick("url", "query") ?? "", 72);
  }

  return truncateMiddle(JSON.stringify(data), 72);
}

function compactPath(path: string) {
  const normalized = path.trim();
  if (!normalized) return "";
  const parts = normalized.split("/");
  if (parts.length <= 3) return normalized;
  return `${parts[0]}/.../${parts.slice(-2).join("/")}`;
}

function truncateMiddle(text: string, limit: number) {
  if (text.length <= limit) return text;
  const head = Math.ceil((limit - 3) * 0.6);
  const tail = Math.floor((limit - 3) * 0.4);
  return `${text.slice(0, head)}...${text.slice(text.length - tail)}`;
}

function formatDuration(ms: number) {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
}
