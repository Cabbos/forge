import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  Clock3,
  FileDiff,
  GitBranch,
  GitPullRequest,
  Pin,
  PinOff,
  ShieldCheck,
  Sparkles,
  Wrench,
} from "lucide-react";
import type { BlockState, SessionState } from "@/lib/protocol";
import { cn } from "@/lib/utils";
import { getProviderModelLabel } from "@/lib/session-display";

interface TaskProgressPopoverProps {
  session?: SessionState;
}

interface ProgressItem {
  label: string;
  status: "done" | "active" | "idle";
}

interface ProgressSummary {
  title: string;
  subtitle: string;
  items: ProgressItem[];
  added: number;
  removed: number;
  changedFiles: number;
  toolCount: number;
  errorCount: number;
  confirmCount: number;
  latestGoal: string;
  sourceLabel: string;
}

const TOOL_LABELS: Record<string, string> = {
  read_file: "读取文件",
  read: "读取文件",
  write_file: "修改文件",
  edit: "修改文件",
  run_shell: "运行命令",
  bash: "运行命令",
  execute_command: "运行命令",
  shell: "运行命令",
  search_content: "搜索内容",
  grep: "搜索内容",
  search_files: "搜索文件",
  glob: "搜索文件",
  git_diff: "查看变更",
  web_fetch: "读取网页",
  web_search: "搜索网页",
};

export function TaskProgressPopover({ session }: TaskProgressPopoverProps) {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const summary = useMemo(() => buildProgressSummary(session), [session]);

  useEffect(() => {
    if (!open || pinned) return;

    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };

    window.addEventListener("pointerdown", handlePointerDown);
    return () => window.removeEventListener("pointerdown", handlePointerDown);
  }, [open, pinned]);

  useEffect(() => {
    if (pinned && session?.streaming) {
      setOpen(true);
    }
  }, [pinned, session?.streaming]);

  const statusColor = session?.streaming ? "#D4A853" : summary.errorCount > 0 ? "#D47777" : "#4A9E6B";

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="group inline-flex min-h-7 items-center gap-2 rounded-md px-3 text-[11px] transition-colors"
        style={{
          background: open ? "var(--card)" : "transparent",
          border: open ? "1px solid var(--border)" : "1px solid transparent",
          color: "var(--muted-foreground)",
        }}
        aria-expanded={open}
      >
        <span className="h-1.5 w-1.5 rounded-full" style={{ background: statusColor }} />
        <span className="max-w-[260px] truncate text-left group-hover:text-foreground">
          {summary.title}
        </span>
        {summary.toolCount > 0 && (
          <span className="rounded px-1.5 py-0.5 font-mono text-[10px]" style={{ background: "var(--secondary)", color: "var(--muted-foreground)" }}>
            {summary.toolCount}
          </span>
        )}
      </button>

      {open && (
        <div
          className="absolute left-1/2 top-9 z-50 w-[300px] -translate-x-1/2 rounded-lg border p-4 text-left shadow-2xl"
          style={{
            background: "var(--popover)",
            borderColor: "var(--border)",
            boxShadow: "0 18px 60px rgba(0,0,0,0.45)",
          }}
        >
          <div className="mb-3 flex items-center justify-between">
            <div>
              <div className="text-[12px] font-medium text-foreground/85">进度</div>
              <div className="mt-1 max-w-[230px] truncate text-[11px]" style={{ color: "var(--muted-foreground)" }}>
                {summary.subtitle}
              </div>
            </div>
            <button
              type="button"
              onClick={() => setPinned((value) => !value)}
              className="rounded-md p-1 transition-colors hover:bg-secondary"
              title={pinned ? "取消固定" : "固定浮层"}
            >
              {pinned ? (
                <PinOff className="size-3.5" style={{ color: "var(--muted-foreground)" }} />
              ) : (
                <Pin className="size-3.5" style={{ color: "var(--muted-foreground)" }} />
              )}
            </button>
          </div>

          <div className="space-y-2">
            {summary.items.map((item) => (
              <ProgressRow key={item.label} item={item} />
            ))}
          </div>

          <Divider />

          <SectionTitle>分支详情</SectionTitle>
          <div className="space-y-2">
            <MetricRow
              icon={<FileDiff className="size-3.5" />}
              label={summary.changedFiles > 0 ? `${summary.changedFiles} 个文件变更` : "变更"}
              value={
                summary.added > 0 || summary.removed > 0 ? (
                  <span>
                    <span style={{ color: "#4AD17A" }}>+{summary.added.toLocaleString()}</span>{" "}
                    <span style={{ color: "#ff6b6b" }}>-{summary.removed.toLocaleString()}</span>
                  </span>
                ) : (
                  <span style={{ color: "var(--muted-foreground)" }}>暂无 diff</span>
                )
              }
            />
            <MetricRow
              icon={<GitBranch className="size-3.5" />}
              label="Git 操作"
              value={<span style={{ color: summary.changedFiles > 0 ? "#E4E7EC" : "var(--muted-foreground)" }}>{summary.changedFiles > 0 ? "有改动待验收" : "未检测到改动"}</span>}
            />
            <MetricRow
              icon={<GitPullRequest className="size-3.5" />}
              label="PR"
              value={<span style={{ color: "var(--muted-foreground)" }}>未创建</span>}
            />
            <MetricRow
              icon={<Wrench className="size-3.5" />}
              label="工具调用"
              value={<span style={{ color: summary.toolCount > 0 ? "#E4E7EC" : "var(--muted-foreground)" }}>{summary.toolCount}</span>}
            />
            <MetricRow
              icon={<ShieldCheck className="size-3.5" />}
              label="权限确认"
              value={<span style={{ color: summary.confirmCount > 0 ? "#D4A853" : "var(--muted-foreground)" }}>{summary.confirmCount}</span>}
            />
            {summary.errorCount > 0 && (
              <MetricRow
                icon={<AlertTriangle className="size-3.5" />}
                label="需要关注"
                value={<span style={{ color: "#D47777" }}>{summary.errorCount}</span>}
              />
            )}
          </div>

          <Divider />

          <SectionTitle>来源</SectionTitle>
          <div className="space-y-2">
            <MetricRow
              icon={<Sparkles className="size-3.5" />}
              label={summary.sourceLabel}
              value={null}
            />
            <div className="rounded-md border px-2.5 py-2 text-[11px] leading-relaxed" style={{ borderColor: "var(--border)", color: "var(--muted-foreground)", background: "var(--card)" }}>
              {summary.latestGoal}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function ProgressRow({ item }: { item: ProgressItem }) {
  const Icon = item.status === "done" ? CheckCircle2 : item.status === "active" ? Clock3 : Circle;
  const color = item.status === "done" ? "#C8CDD6" : item.status === "active" ? "#D4A853" : "#8C93A0";

  return (
    <div className="flex items-center gap-2 text-[12px]" style={{ color }}>
      <Icon className={cn("size-3.5 shrink-0", item.status === "active" && "animate-pulse")} />
      <span className="truncate">{item.label}</span>
    </div>
  );
}

function MetricRow({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: string;
  value: ReactNode;
}) {
  return (
    <div className="flex items-center gap-2 text-[12px]" style={{ color: "#E4E7EC" }}>
      <span className="shrink-0" style={{ color: "var(--muted-foreground)" }}>{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {value && <span className="shrink-0 font-mono text-[12px]">{value}</span>}
    </div>
  );
}

function SectionTitle({ children }: { children: ReactNode }) {
  return (
    <div className="mb-2 text-[12px] font-medium" style={{ color: "var(--muted-foreground)" }}>
      {children}
    </div>
  );
}

function Divider() {
  return <div className="my-4 h-px" style={{ background: "var(--border)" }} />;
}

function buildProgressSummary(session?: SessionState): ProgressSummary {
  const blocks = session?.blocks ?? [];
  const latestGoal = getLatestGoal(blocks);
  const toolBlocks = blocks.filter((block) => block.event_type === "tool_call" || block.event_type === "shell");
  const diffBlocks = blocks.filter((block) => block.event_type === "diff_view");
  const confirmCount = blocks.filter((block) => block.event_type === "confirm_ask").length;
  const errorCount = blocks.filter((block) =>
    block.event_type === "error" || Boolean(block.metadata?.is_error)
  ).length;
  const diffStats = getDiffStats(diffBlocks);
  const hasUserGoal = blocks.some((block) => block.event_type === "user_message");
  const hasAssistantOutput = blocks.some((block) => block.event_type === "text" && block.content.trim());
  const hasToolActivity = toolBlocks.length > 0 || diffBlocks.length > 0;
  const hasWriteOrDiff = diffStats.changedFiles > 0 || toolBlocks.some((block) => isWriteTool(block));
  const isStreaming = Boolean(session?.streaming);
  const isDone = Boolean(session && !isStreaming && hasAssistantOutput);

  const items: ProgressItem[] = [
    {
      label: hasUserGoal ? "理解你的目标" : "等待你描述目标",
      status: hasUserGoal ? "done" : "active",
    },
    {
      label: hasToolActivity ? "查找文件、工具和风险点" : "准备分析项目档案",
      status: hasToolActivity ? "done" : isStreaming ? "active" : "idle",
    },
    {
      label: hasWriteOrDiff ? "整理改动和验证线索" : "等待产生可验收结果",
      status: hasWriteOrDiff ? "done" : hasToolActivity || isStreaming ? "active" : "idle",
    },
    {
      label: isDone ? "等待你验收结果" : "继续生成回答",
      status: isDone ? "active" : hasAssistantOutput ? "done" : "idle",
    },
  ];

  const currentTool = getCurrentToolLabel(toolBlocks);
  const title = session
    ? isStreaming
      ? currentTool
        ? `正在${currentTool}`
        : "正在处理你的请求"
      : errorCount > 0
        ? "需要你关注一个异常"
        : "可以继续描述任务"
    : "创建任务后开始";

  return {
    title,
    subtitle: isStreaming ? "实时整理任务状态" : "最近一次任务摘要",
    items,
    added: diffStats.added,
    removed: diffStats.removed,
    changedFiles: diffStats.changedFiles,
    toolCount: toolBlocks.length,
    errorCount,
    confirmCount,
    latestGoal,
    sourceLabel: formatSourceLabel(session),
  };
}

function getLatestGoal(blocks: BlockState[]) {
  const latest = [...blocks].reverse().find((block) => block.event_type === "user_message");
  const text = latest?.content.trim() || "还没有目标。你可以直接描述想做什么，应用会把过程整理成可验收的步骤。";
  return text.length > 96 ? `${text.slice(0, 96)}...` : text;
}

function getCurrentToolLabel(blocks: BlockState[]) {
  const running = [...blocks].reverse().find((block) => !block.isComplete);
  const toolName = running?.metadata?.tool_name as string | undefined;
  if (!toolName && running?.event_type === "shell") return "运行命令";
  if (!toolName) return "";
  return TOOL_LABELS[toolName] ?? toolName;
}

function isWriteTool(block: BlockState) {
  const toolName = String(block.metadata?.tool_name ?? "");
  return ["write_file", "edit", "run_shell", "bash", "execute_command", "shell"].includes(toolName);
}

function getDiffStats(blocks: BlockState[]) {
  const files = new Set<string>();
  let added = 0;
  let removed = 0;

  for (const block of blocks) {
    const filePath = block.metadata?.file_path;
    if (typeof filePath === "string" && filePath && filePath !== "all files") {
      files.add(filePath);
    }

    for (const line of block.content.split("\n")) {
      if (line.startsWith("diff --git ")) {
        const match = line.match(/^diff --git a\/(.+?) b\//);
        if (match?.[1]) files.add(match[1]);
      } else if (line.startsWith("+") && !line.startsWith("+++")) {
        added += 1;
      } else if (line.startsWith("-") && !line.startsWith("---")) {
        removed += 1;
      }
    }
  }

  return { added, removed, changedFiles: files.size };
}

function formatSourceLabel(session?: SessionState) {
  if (!session) return "当前任务";
  return getProviderModelLabel(session.agentType, session.model);
}
