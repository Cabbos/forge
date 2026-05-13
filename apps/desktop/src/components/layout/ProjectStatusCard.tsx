import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight, Circle, Folder, GitBranch, RefreshCw } from "lucide-react";
import {
  getProjectCheckpointStatus,
  getProjectRuntimeStatus,
  type ProjectCheckpointStatus,
  type ProjectRuntimeStatus,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface ProjectStatusCardProps {
  sessionId: string | null;
}

export function ProjectStatusCard({ sessionId }: ProjectStatusCardProps) {
  const [runtime, setRuntime] = useState<ProjectRuntimeStatus | null>(null);
  const [checkpoint, setCheckpoint] = useState<ProjectCheckpointStatus | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [runtimeStatus, checkpointStatus] = await Promise.all([
        getProjectRuntimeStatus(sessionId ?? undefined),
        getProjectCheckpointStatus(sessionId ?? undefined),
      ]);
      setRuntime(runtimeStatus);
      setCheckpoint(checkpointStatus);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const projectName = useMemo(() => {
    const path = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "");
    if (!path) return "未选择项目";
    return path.split("/").filter(Boolean).pop() || path;
  }, [runtime?.working_dir, checkpoint?.working_dir]);

  const projectPath = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "") || "暂无项目路径";
  const previewRunning = runtime?.running ?? false;
  const previewLabel = runtime
    ? previewRunning
      ? "预览运行中"
      : "预览未运行"
    : "预览状态未知";
  const checkpointLabel = checkpoint
    ? checkpoint.last_checkpoint
      ? checkpoint.dirty
        ? "有检查点，当前有改动"
        : "检查点已就绪"
      : checkpoint.is_git_repo
        ? "尚未创建检查点"
        : "不是 Git 项目"
    : "检查点状态未知";

  return (
    <section className="rounded-md border border-border bg-card">
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <Folder className="size-3.5 shrink-0 text-muted-foreground" />
          <div className="min-w-0">
            <div className="truncate text-xs font-medium text-foreground">{projectName}</div>
            <div className="mt-0.5 truncate font-mono text-[10px] text-muted-foreground/75">
              {projectPath}
            </div>
          </div>
        </div>
        <button
          type="button"
          onClick={refresh}
          className="rounded p-1 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
          title="刷新交付状态"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </button>
      </div>

      <div className="space-y-2 px-3 py-2.5">
        <StatusLine
          color={previewRunning ? "#4A9E6B" : "#8C93A0"}
          label="预览"
          value={previewLabel}
        />
        <StatusLine
          color={checkpoint?.last_checkpoint ? "#D4A853" : "#8C93A0"}
          label="检查点"
          value={checkpointLabel}
        />
        {error && (
          <div className="rounded border border-destructive/20 bg-destructive/5 px-2 py-1.5 text-[11px] leading-relaxed text-destructive">
            {error}
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={() => setExpanded((value) => !value)}
        className="flex w-full items-center justify-between border-t border-border px-3 py-2 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
      >
        <span>{expanded ? "收起详情" : "展开详情"}</span>
        {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
      </button>

      {expanded && (
        <div className="space-y-2 border-t border-border px-3 py-2.5 text-[11px]">
          <DetailLine label="预览地址" value={runtime?.url || "暂无"} />
          <DetailLine label="运行命令" value={runtime?.command || "未检测到"} />
          <DetailLine label="检查点" value={checkpoint?.message || "暂无"} />
          {checkpoint?.last_checkpoint && (
            <div className="flex min-w-0 items-center gap-2 rounded bg-background/60 px-2 py-1.5 font-mono text-[10px] text-muted-foreground">
              <GitBranch className="size-3 shrink-0" />
              <span className="truncate">{checkpoint.last_checkpoint.head}</span>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function StatusLine({ color, label, value }: { color: string; label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 text-xs">
      <span className="flex items-center gap-2 text-muted-foreground">
        <Circle className="size-2.5 fill-current" style={{ color }} />
        {label}
      </span>
      <span className="min-w-0 truncate text-right text-foreground/80">{value}</span>
    </div>
  );
}

function DetailLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="shrink-0 text-muted-foreground">{label}</span>
      <span className="min-w-0 truncate text-right font-mono text-muted-foreground/70">{value}</span>
    </div>
  );
}

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}
