import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight, Circle, ExternalLink, Folder, GitBranch, Play, RefreshCw, ShieldCheck } from "lucide-react";
import {
  createProjectCheckpoint,
  getProjectCheckpointStatus,
  getProjectRuntimeStatus,
  openProjectPreview,
  type ProjectCheckpointStatus,
  type ProjectRuntimeStatus,
  startProjectDevServer,
} from "@/lib/tauri";
import { getDeliveryConfidence, type DeliveryAction } from "@/lib/delivery-confidence";
import { cn } from "@/lib/utils";

interface ProjectStatusCardProps {
  sessionId: string | null;
}

export function ProjectStatusCard({ sessionId }: ProjectStatusCardProps) {
  const [runtime, setRuntime] = useState<ProjectRuntimeStatus | null>(null);
  const [checkpoint, setCheckpoint] = useState<ProjectCheckpointStatus | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [actionBusy, setActionBusy] = useState<DeliveryAction | null>(null);
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

  const runDeliveryAction = useCallback(async (action: DeliveryAction) => {
    setActionBusy(action);
    setError("");
    try {
      if (action === "start_preview") {
        const runtimeStatus = await startProjectDevServer(sessionId ?? undefined);
        setRuntime(runtimeStatus);
      } else if (action === "open_preview") {
        const runtimeStatus = await openProjectPreview(sessionId ?? undefined);
        setRuntime(runtimeStatus);
      } else if (action === "create_checkpoint") {
        const checkpointStatus = await createProjectCheckpoint(sessionId ?? undefined);
        setCheckpoint(checkpointStatus);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionBusy(null);
    }
  }, [refresh, sessionId]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const projectName = useMemo(() => {
    const path = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "");
    if (!path) return "未选择项目";
    return path.split("/").filter(Boolean).pop() || path;
  }, [runtime?.working_dir, checkpoint?.working_dir]);

  const projectPath = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "") || "暂无项目路径";
  const delivery = getDeliveryConfidence(runtime, checkpoint);

  return (
    <section className="forge-surface">
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <Folder className="size-3.5 shrink-0 text-muted-foreground" />
          <div className="min-w-0" title={projectPath}>
            <div className="truncate text-xs font-medium text-foreground">{projectName}</div>
          </div>
        </div>
        <button
          type="button"
          onClick={refresh}
          className="forge-icon-button size-7"
          title="刷新交付状态"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </button>
      </div>

      <div className="space-y-2 px-3 py-2.5">
        <StatusLine
          color={delivery.preview.color}
          label="预览"
          value={delivery.preview.label}
        />
        <StatusLine
          color={delivery.checkpoint.color}
          label="检查点"
          value={delivery.checkpoint.label}
        />
        <div className="forge-surface-quiet px-2 py-1.5 text-[11px] leading-relaxed text-muted-foreground">
          {delivery.nextAction}
        </div>
        <div className="flex flex-wrap gap-1.5">
          {delivery.preview.action && delivery.preview.actionLabel && (
            <DeliveryButton
              action={delivery.preview.action}
              busy={actionBusy === delivery.preview.action}
              label={delivery.preview.actionLabel}
              onClick={runDeliveryAction}
            />
          )}
          {delivery.checkpoint.action && delivery.checkpoint.actionLabel && (
            <DeliveryButton
              action={delivery.checkpoint.action}
              busy={actionBusy === delivery.checkpoint.action}
              label={delivery.checkpoint.actionLabel}
              onClick={runDeliveryAction}
            />
          )}
        </div>
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

function DeliveryButton({
  action,
  busy,
  label,
  onClick,
}: {
  action: DeliveryAction;
  busy: boolean;
  label: string;
  onClick: (action: DeliveryAction) => void;
}) {
  const Icon = action === "start_preview" ? Play : action === "open_preview" ? ExternalLink : ShieldCheck;

  return (
    <button
      type="button"
      disabled={busy}
      onClick={() => onClick(action)}
      className="forge-action disabled:cursor-default disabled:opacity-70"
    >
      <Icon className={cn("size-3.5", busy && "animate-pulse")} />
      {label}
    </button>
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
