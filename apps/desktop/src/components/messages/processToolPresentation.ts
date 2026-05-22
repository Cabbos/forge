import type { BlockState } from "@/lib/protocol";

type ToolCallStatus = "running" | "done" | "error";

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

export function deriveToolCallView(block: BlockState) {
  const isError = Boolean(block.metadata.is_error ?? false);
  const status: ToolCallStatus = block.isComplete ? (isError ? "error" : "done") : "running";
  const toolName = (block.metadata.tool_name as string) || "tool";
  const toolInput = block.metadata.tool_input;
  const toolCopy = TOOL_COPY[toolName] ?? { label: toolName, running: `正在执行 ${toolName}`, done: `${toolName} 已完成` };
  const detailText = block.content || (status === "running" ? "等待工具返回结果..." : "");
  const durationMs = typeof block.metadata.duration_ms === "number" ? block.metadata.duration_ms : null;

  return {
    actionText: status === "running" ? toolCopy.running : status === "error" ? `${toolCopy.label}遇到问题` : toolCopy.done,
    detailText,
    durationLabel: block.isComplete && durationMs !== null ? formatDuration(durationMs) : "",
    inputSummary: summarizeToolInput(toolName, toolInput),
    isError,
    resultSummary: summarizeToolResult(detailText, isError),
    status,
    toolName,
  };
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

function summarizeToolResult(text: string, isError: boolean) {
  const firstUsefulLine = text
    .split("\n")
    .map((line) => line.trim())
    .find(Boolean);
  if (!firstUsefulLine) return "";
  return truncateMiddle(firstUsefulLine, isError ? 96 : 72);
}
