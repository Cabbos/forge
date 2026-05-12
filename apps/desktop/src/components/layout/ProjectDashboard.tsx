import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  ClipboardCheck,
  History,
  ExternalLink,
  FileDiff,
  Lightbulb,
  Loader2,
  Play,
  RefreshCw,
  Square,
  TerminalSquare,
  Wrench,
} from "lucide-react";
import type { BlockState, SessionState } from "@/lib/protocol";
import {
  getProjectRuntimeStatus,
  listCapabilities,
  openProjectPreview,
  createProjectCheckpoint,
  getProjectCheckpointStatus,
  restoreProjectCheckpoint,
  startProjectDevServer,
  stopProjectDevServer,
  type ProjectCheckpointStatus,
  type ProjectRuntimeStatus,
} from "@/lib/tauri";
import { useStore } from "@/store";
import { cn } from "@/lib/utils";

interface ProjectDashboardProps {
  sessionId: string | null;
  session?: SessionState | null;
}

interface DashboardSummary {
  goal: string;
  added: number;
  removed: number;
  changedFiles: number;
  toolCount: number;
  errorCount: number;
  confirmCount: number;
  checklist: { label: string; done: boolean }[];
  risks: string[];
}

const NEXT_PROMPTS = [
  "请根据当前项目状态，告诉我下一步最值得优化的一件事，并直接开始处理。",
  "请用小白能懂的话检查当前改动风险，并给我一份验收清单。",
  "请运行或指导我运行项目预览，然后根据结果继续修正体验问题。",
];

export function ProjectDashboard({ sessionId, session }: ProjectDashboardProps) {
  const [runtime, setRuntime] = useState<ProjectRuntimeStatus | null>(null);
  const [runtimeError, setRuntimeError] = useState("");
  const [checkpoint, setCheckpoint] = useState<ProjectCheckpointStatus | null>(null);
  const [checkpointError, setCheckpointError] = useState("");
  const [busyAction, setBusyAction] = useState<"refresh" | "start" | "stop" | "open" | null>(null);
  const [busyCheckpoint, setBusyCheckpoint] = useState<"create" | "restore" | null>(null);
  const [showLogs, setShowLogs] = useState(false);
  const [capabilityCount, setCapabilityCount] = useState(0);
  const [showOnboarding, setShowOnboarding] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem("tui-onboarding-complete") !== "1";
  });
  const setPendingInput = useStore((s) => s.setPendingInput);
  const summary = useMemo(() => buildDashboardSummary(session, runtime), [session, runtime]);

  const refreshRuntime = useCallback(async () => {
    setRuntimeError("");
    try {
      const status = await getProjectRuntimeStatus(sessionId ?? undefined);
      setRuntime(status);
    } catch (error) {
      setRuntimeError(error instanceof Error ? error.message : String(error));
    }
  }, [sessionId]);

  const refreshCheckpoint = useCallback(async () => {
    setCheckpointError("");
    try {
      const status = await getProjectCheckpointStatus(sessionId ?? undefined);
      setCheckpoint(status);
    } catch (error) {
      setCheckpointError(error instanceof Error ? error.message : String(error));
    }
  }, [sessionId]);

  useEffect(() => {
    refreshRuntime();
    const timer = window.setInterval(refreshRuntime, 4000);
    return () => window.clearInterval(timer);
  }, [refreshRuntime]);

  useEffect(() => {
    refreshCheckpoint();
  }, [refreshCheckpoint, session?.blocks.length]);

  useEffect(() => {
    listCapabilities()
      .then((all) => setCapabilityCount(all.filter((c) => c.enabled !== false).length))
      .catch(() => {});
  }, []);

  const runRuntimeAction = async (action: "refresh" | "start" | "stop" | "open") => {
    setBusyAction(action);
    setRuntimeError("");
    try {
      const next = action === "refresh"
        ? await getProjectRuntimeStatus(sessionId ?? undefined)
        : action === "start"
          ? await startProjectDevServer(sessionId ?? undefined)
          : action === "stop"
            ? await stopProjectDevServer(sessionId ?? undefined)
            : await openProjectPreview(sessionId ?? undefined);
      setRuntime(next);
    } catch (error) {
      setRuntimeError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusyAction(null);
    }
  };

  const runCheckpointAction = async (action: "create" | "restore") => {
    if (action === "restore") {
      const ok = window.confirm("确定要回到最近检查点吗？这会回退 tracked 文件，未跟踪文件不会自动删除。");
      if (!ok) return;
    }

    setBusyCheckpoint(action);
    setCheckpointError("");
    try {
      const next = action === "create"
        ? await createProjectCheckpoint(sessionId ?? undefined)
        : await restoreProjectCheckpoint(sessionId ?? undefined);
      setCheckpoint(next);
    } catch (error) {
      setCheckpointError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusyCheckpoint(null);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {showOnboarding && (
        <FirstRunGuide
          onDone={(prompt) => {
            window.localStorage.setItem("tui-onboarding-complete", "1");
            setShowOnboarding(false);
            setPendingInput(prompt);
          }}
          onSkip={() => {
            window.localStorage.setItem("tui-onboarding-complete", "1");
            setShowOnboarding(false);
          }}
        />
      )}

      <Section title="当前目标" icon={<Lightbulb className="size-3.5" />}>
        <div className="rounded-md border px-3 py-2 text-xs leading-relaxed" style={{ background: "var(--card)", borderColor: "var(--border)", color: "#D0D5DD" }}>
          {summary.goal}
        </div>
      </Section>

      <Section title="运行 / 预览" icon={<TerminalSquare className="size-3.5" />}>
        <RuntimePanel
          runtime={runtime}
          error={runtimeError}
          busyAction={busyAction}
          showLogs={showLogs}
          onToggleLogs={() => setShowLogs((value) => !value)}
          onAction={runRuntimeAction}
        />
      </Section>

      <Section title="检查点 / 回退" icon={<History className="size-3.5" />}>
        <CheckpointPanel
          checkpoint={checkpoint}
          error={checkpointError}
          busyAction={busyCheckpoint}
          onCreate={() => runCheckpointAction("create")}
          onRestore={() => runCheckpointAction("restore")}
        />
      </Section>

      <Section title="最近改动" icon={<FileDiff className="size-3.5" />}>
        <div className="grid grid-cols-3 gap-2">
          <Metric label="文件" value={String(summary.changedFiles)} />
          <Metric label="新增" value={`+${summary.added}`} valueColor="#4AD17A" />
          <Metric label="删除" value={`-${summary.removed}`} valueColor="#ff6b6b" />
        </div>
      </Section>

      <Section title="待验收事项" icon={<ClipboardCheck className="size-3.5" />}>
        <div className="space-y-2">
          {summary.checklist.map((item) => (
            <div
              key={item.label}
              className="flex items-center gap-2 rounded-md border px-2.5 py-2 text-xs"
              style={{
                color: item.done ? "#D0D5DD" : "var(--muted-foreground)",
                borderColor: item.done ? "rgba(74,158,107,0.3)" : "var(--border)",
                background: item.done ? "rgba(74,158,107,0.08)" : "var(--card)",
              }}
            >
              <CheckCircle2 className="size-3.5 shrink-0" style={{ color: item.done ? "#4A9E6B" : "#8C93A0" }} />
              <span className="min-w-0 flex-1">{item.label}</span>
              <span className="text-[10px]" style={{ color: item.done ? "#4A9E6B" : "var(--muted-foreground)" }}>
                {item.done ? "已通过" : "待确认"}
              </span>
            </div>
          ))}
        </div>
      </Section>

      <Section title="风险提醒" icon={<AlertTriangle className="size-3.5" />}>
        <div className="space-y-2">
          {summary.risks.map((risk) => (
            <div key={risk} className="rounded-md border px-2.5 py-2 text-xs leading-relaxed" style={{ borderColor: "var(--border)", color: "#D0D5DD", background: "var(--card)" }}>
              {risk}
            </div>
          ))}
        </div>
      </Section>

      <Section title="可用能力" icon={<Wrench className="size-3.5" />}>
        <div className="flex items-center justify-between rounded-md border px-3 py-2 text-xs" style={{ borderColor: "var(--border)", background: "var(--card)" }}>
          <span style={{ color: "#D0D5DD" }}>已启用能力</span>
          <span className="font-mono" style={{ color: "#D4A853" }}>{capabilityCount}</span>
        </div>
      </Section>

      <Section title="下一步建议" icon={<Lightbulb className="size-3.5" />}>
        <div className="space-y-2">
          {NEXT_PROMPTS.map((prompt) => (
            <button
              key={prompt}
              type="button"
              onClick={() => setPendingInput(prompt)}
              className="w-full rounded-md border px-3 py-2 text-left text-xs leading-relaxed transition-colors hover:text-foreground"
              style={{ borderColor: "var(--border)", background: "var(--card)", color: "var(--muted-foreground)" }}
            >
              {prompt}
            </button>
          ))}
        </div>
      </Section>
    </div>
  );
}

function FirstRunGuide({
  onDone,
  onSkip,
}: {
  onDone: (prompt: string) => void;
  onSkip: () => void;
}) {
  const [intent, setIntent] = useState("优化界面");
  const [mode, setMode] = useState("先给方案");
  const [confirm, setConfirm] = useState("改文件前确认");

  const prompt = `我第一次使用这个项目助手。我的目标偏向：${intent}。请按「${mode}」的方式工作，并且${confirm}。先看懂当前项目，再告诉我接下来最应该做的一步。`;

  return (
    <section className="rounded-lg border p-3" style={{ borderColor: "rgba(212,168,83,0.28)", background: "rgba(212,168,83,0.06)" }}>
      <div className="mb-2 flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-medium text-foreground">首次使用向导</div>
          <div className="mt-1 text-xs leading-relaxed" style={{ color: "var(--muted-foreground)" }}>
            回答三个问题，先把第一条请求组织清楚。
          </div>
        </div>
        <button type="button" onClick={onSkip} className="text-[11px]" style={{ color: "var(--muted-foreground)" }}>
          跳过
        </button>
      </div>

      <GuideChoice
        label="你想做什么？"
        value={intent}
        options={["做新功能", "修 bug", "优化界面"]}
        onChange={setIntent}
      />
      <GuideChoice
        label="希望 AI 怎么开始？"
        value={mode}
        options={["先给方案", "直接修改", "先问我问题"]}
        onChange={setMode}
      />
      <GuideChoice
        label="改文件前要不要确认？"
        value={confirm}
        options={["改文件前确认", "小改动可自动改", "只读分析"]}
        onChange={setConfirm}
      />

      <button
        type="button"
        onClick={() => onDone(prompt)}
        className="mt-3 w-full rounded-md px-3 py-2 text-xs font-medium"
        style={{ background: "#D4A853", color: "#111216" }}
      >
        生成第一条请求
      </button>
    </section>
  );
}

function GuideChoice({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: string[];
  onChange: (value: string) => void;
}) {
  return (
    <div className="mt-3">
      <div className="mb-1.5 text-[11px]" style={{ color: "var(--muted-foreground)" }}>{label}</div>
      <div className="grid grid-cols-3 gap-1.5">
        {options.map((option) => (
          <button
            key={option}
            type="button"
            onClick={() => onChange(option)}
            className="rounded-md border px-2 py-1.5 text-[11px] transition-colors"
            style={{
              borderColor: value === option ? "rgba(212,168,83,0.55)" : "var(--border)",
              background: value === option ? "rgba(212,168,83,0.14)" : "var(--secondary)",
              color: value === option ? "#D4A853" : "var(--muted-foreground)",
            }}
          >
            {option}
          </button>
        ))}
      </div>
    </div>
  );
}

function CheckpointPanel({
  checkpoint,
  error,
  busyAction,
  onCreate,
  onRestore,
}: {
  checkpoint: ProjectCheckpointStatus | null;
  error: string;
  busyAction: "create" | "restore" | null;
  onCreate: () => void;
  onRestore: () => void;
}) {
  const last = checkpoint?.last_checkpoint;
  const date = last ? new Date(last.created_at * 1000) : null;

  return (
    <div className="rounded-md border" style={{ borderColor: "var(--border)", background: "var(--card)" }}>
      <div className="px-3 py-2.5">
        <div className="flex items-center gap-2 text-xs" style={{ color: "#E4E7EC" }}>
          <span className="h-1.5 w-1.5 rounded-full" style={{ background: last ? "#4A9E6B" : "#8C93A0" }} />
          <span>{checkpoint?.message ?? "正在检查 Git 状态"}</span>
        </div>
        <div className="mt-1 font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>
          {last ? `${last.head} · ${date?.toLocaleString()}` : "发送任务前会自动创建"}
        </div>
        {checkpoint?.dirty && (
          <div className="mt-2 text-[11px]" style={{ color: "#D4A853" }}>
            当前有未提交改动，建议保留检查点后再继续。
          </div>
        )}
      </div>
      {error && (
        <div className="border-t px-3 py-2 text-xs leading-relaxed" style={{ borderColor: "var(--border)", color: "#D47777" }}>
          {error}
        </div>
      )}
      <div className="flex gap-2 border-t px-3 py-2" style={{ borderColor: "var(--border)" }}>
        <ActionButton
          disabled={!checkpoint?.is_git_repo || Boolean(busyAction)}
          active={busyAction === "create"}
          icon={<History className="size-3" />}
          label="创建"
          onClick={onCreate}
        />
        <ActionButton
          disabled={!last || Boolean(busyAction)}
          active={busyAction === "restore"}
          icon={<RefreshCw className="size-3" />}
          label="回到修改前"
          onClick={onRestore}
        />
      </div>
    </div>
  );
}

function RuntimePanel({
  runtime,
  error,
  busyAction,
  showLogs,
  onToggleLogs,
  onAction,
}: {
  runtime: ProjectRuntimeStatus | null;
  error: string;
  busyAction: string | null;
  showLogs: boolean;
  onToggleLogs: () => void;
  onAction: (action: "refresh" | "start" | "stop" | "open") => void;
}) {
  const running = runtime?.running ?? false;
  const statusColor = running ? "#4A9E6B" : "#8C93A0";

  return (
    <div className="rounded-md border" style={{ borderColor: "var(--border)", background: "var(--card)" }}>
      <div className="flex items-start justify-between gap-3 px-3 py-2.5">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-xs" style={{ color: "#E4E7EC" }}>
            <span className="h-1.5 w-1.5 rounded-full" style={{ background: statusColor }} />
            <span>{runtime?.message ?? "正在检测项目运行状态"}</span>
          </div>
          <div className="mt-1 truncate font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>
            {runtime?.command ?? "未检测到 dev 命令"}
          </div>
          <div className="mt-0.5 truncate font-mono text-[10px]" style={{ color: "var(--muted-foreground)" }}>
            {runtime?.url ?? "http://localhost"}
          </div>
        </div>
        <button
          type="button"
          onClick={() => onAction("refresh")}
          className="rounded p-1 transition-colors hover:bg-secondary"
          title="刷新运行状态"
        >
          <RefreshCw className={cn("size-3.5", busyAction === "refresh" && "animate-spin")} style={{ color: "var(--muted-foreground)" }} />
        </button>
      </div>

      {error && (
        <div className="border-t px-3 py-2 text-xs leading-relaxed" style={{ borderColor: "var(--border)", color: "#D47777" }}>
          {error}
        </div>
      )}

      <div className="flex gap-2 border-t px-3 py-2" style={{ borderColor: "var(--border)" }}>
        <ActionButton
          disabled={!runtime?.can_start || Boolean(busyAction)}
          active={busyAction === "start"}
          icon={<Play className="size-3" />}
          label="启动"
          onClick={() => onAction("start")}
        />
        <ActionButton
          disabled={!runtime?.can_open || Boolean(busyAction)}
          active={busyAction === "open"}
          icon={<ExternalLink className="size-3" />}
          label="打开"
          onClick={() => onAction("open")}
        />
        <ActionButton
          disabled={!runtime?.can_stop || Boolean(busyAction)}
          active={busyAction === "stop"}
          icon={<Square className="size-3" />}
          label="停止"
          onClick={() => onAction("stop")}
        />
      </div>

      <button
        type="button"
        onClick={onToggleLogs}
        className="w-full border-t px-3 py-2 text-left text-[11px] transition-colors hover:text-foreground"
        style={{ borderColor: "var(--border)", color: "var(--muted-foreground)" }}
      >
        {showLogs ? "隐藏日志" : "查看日志"} {runtime?.logs.length ? `(${runtime.logs.length})` : ""}
      </button>
      {showLogs && (
        <div className="max-h-[180px] overflow-auto border-t p-2 font-mono text-[10px] leading-relaxed" style={{ borderColor: "var(--border)", color: "#D0D5DD", background: "var(--background)" }}>
          {runtime?.logs.length
            ? runtime.logs.map((line, index) => <div key={`${line}-${index}`}>{line}</div>)
            : <div>暂无日志。由应用启动的预览服务会在这里显示最近输出。</div>}
        </div>
      )}
    </div>
  );
}

function ActionButton({
  icon,
  label,
  active,
  disabled,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-md px-2 py-1.5 text-[11px] transition-colors disabled:cursor-default disabled:opacity-40"
      style={{ background: "var(--secondary)", color: disabled ? "#8C93A0" : "#E4E7EC" }}
    >
      {active ? <Loader2 className="size-3 animate-spin" /> : icon}
      {label}
    </button>
  );
}

function Section({ title, icon, children }: { title: string; icon: ReactNode; children: ReactNode }) {
  return (
    <section>
      <div className="mb-2 flex items-center gap-2 text-[11px] font-medium" style={{ color: "var(--muted-foreground)" }}>
        <span style={{ color: "var(--muted-foreground)" }}>{icon}</span>
        {title}
      </div>
      {children}
    </section>
  );
}

function Metric({ label, value, valueColor = "#d0d0d0" }: { label: string; value: string; valueColor?: string }) {
  return (
    <div className="rounded-md border px-2.5 py-2" style={{ background: "var(--card)", borderColor: "var(--border)" }}>
      <div className="text-[10px]" style={{ color: "var(--muted-foreground)" }}>{label}</div>
      <div className="mt-1 font-mono text-sm" style={{ color: valueColor }}>{value}</div>
    </div>
  );
}

function buildDashboardSummary(session?: SessionState | null, runtime?: ProjectRuntimeStatus | null): DashboardSummary {
  const blocks = session?.blocks ?? [];
  const latestUser = [...blocks].reverse().find((block) => block.event_type === "user_message");
  const toolBlocks = blocks.filter((block) => block.event_type === "tool_call" || block.event_type === "shell");
  const errorCount = blocks.filter((block) => block.event_type === "error" || Boolean(block.metadata?.is_error)).length;
  const confirmCount = blocks.filter((block) => block.event_type === "confirm_ask").length;
  const diffStats = getDiffStats(blocks.filter((block) => block.event_type === "diff_view"));

  const checklist = [
    { label: "页面能打开", done: Boolean(runtime?.running) },
    { label: "没有构建错误", done: blocks.some((block) => /npm run build|cargo check|built in|Finished/.test(block.content)) },
    { label: "关键功能可点击", done: confirmCount > 0 || toolBlocks.length > 0 },
    { label: "改动符合目标", done: blocks.some((block) => block.event_type === "text" && block.isComplete) },
    { label: "未验证项已列出", done: blocks.some((block) => /未验证|待验收|验收/.test(block.content)) },
  ];

  const risks = [
    errorCount > 0 ? `发现 ${errorCount} 个异常，需要先确认。` : "",
    confirmCount > 0 ? `有 ${confirmCount} 次权限确认，说明这次任务涉及真实操作。` : "",
    diffStats.changedFiles === 0 ? "还没有检测到可展示的 diff，完成后建议让 AI 调用 git_diff。" : "",
    !session ? "当前没有活跃会话，先创建一个会话再开始。" : "",
  ].filter(Boolean);

  return {
    goal: latestUser?.content.trim() || "还没有当前目标。你可以从一句话开始描述想做什么。",
    added: diffStats.added,
    removed: diffStats.removed,
    changedFiles: diffStats.changedFiles,
    toolCount: toolBlocks.length,
    errorCount,
    confirmCount,
    checklist,
    risks: risks.length ? risks : ["暂时没有明显风险。下一步可以运行预览或让 AI 做一次验收检查。"],
  };
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
