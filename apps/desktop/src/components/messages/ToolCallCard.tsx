import { useState, useEffect } from "react";
import { ChevronRight, Loader2, CheckCircle2, XCircle, Wrench } from "lucide-react";
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
  // Keep normal tool chatter compact; only surface errors automatically.
  useEffect(() => {
    if (block.isComplete && isError) setOpen(true);
  }, [block.isComplete, isError]);
  const toolName = (block.metadata.tool_name as string) || "tool";
  const toolInput = block.metadata.tool_input;
  const status = block.isComplete ? (isError ? "error" : "done") : "running";
  const copy = TOOL_COPY[toolName] ?? { label: toolName, running: `正在执行 ${toolName}`, done: `${toolName} 已完成` };
  const actionText = status === "running" ? copy.running : status === "error" ? `${copy.label}遇到问题` : copy.done;
  const inputSummary = summarizeToolInput(toolName, toolInput);

  const StatusIcon = { running: Loader2, done: CheckCircle2, error: XCircle }[status];
  const statusColor = { running: "#D4A853", done: "#4A9E6B", error: "#D47777" }[status];
  const statusText = { running: "进行中", done: "完成", error: "异常" }[status];

  return (
    <div className="mb-3">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="inline-flex max-w-full items-center gap-2 rounded-md border px-3 py-2 text-xs transition-colors"
          style={{ background: "var(--card)", borderColor: isError ? "rgba(212,119,119,0.4)" : "var(--border)", color: "#E4E7EC" }}>
          <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
          <Wrench className="size-3.5 shrink-0" style={{ color: statusColor }} />
          <span className="shrink-0 font-medium">{actionText}</span>
          {inputSummary && (
            <span className="min-w-0 truncate font-mono text-[11px]" style={{ color: "var(--muted-foreground)" }}>
              {inputSummary}
            </span>
          )}
          <span className="ml-auto flex shrink-0 items-center gap-1" style={{ color: statusColor, fontSize: "10px" }}>
            <StatusIcon className={cn("size-3", status === "running" && "animate-spin")} />
            {statusText}
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
                <span className="font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>{toolName}</span>
              </div>
              <div className="max-h-[220px] overflow-auto p-3 font-mono text-xs whitespace-pre-wrap break-all"
                style={{ color: "#D0D5DD" }}>
                {block.content || (status === "running" ? "等待工具返回结果..." : "")}
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
