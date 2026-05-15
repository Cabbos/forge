import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2, KeyRound, MonitorPlay, RefreshCw, ShieldCheck, FolderOpen } from "lucide-react";
import { useActiveWorkspace, useStore } from "@/store";
import {
  createProjectCheckpoint,
  getApiKeyStatus,
  getProjectCheckpointStatus,
  getProjectRuntimeStatus,
  startProjectDevServer,
  type KeyStatus,
  type ProjectCheckpointStatus,
  type ProjectRuntimeStatus,
} from "@/lib/tauri";
import { getProviderLabel } from "@/lib/providers";
import { deriveStartReadiness, type ReadinessAction, type ReadinessTone } from "@/lib/start-readiness";
import { cn } from "@/lib/utils";

interface StartReadinessCardProps {
  sessionId?: string;
}

export function StartReadinessCard({ sessionId }: StartReadinessCardProps) {
  const activeWorkspace = useActiveWorkspace();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const [keys, setKeys] = useState<KeyStatus[]>([]);
  const [runtime, setRuntime] = useState<ProjectRuntimeStatus | null>(null);
  const [checkpoint, setCheckpoint] = useState<ProjectCheckpointStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [busyAction, setBusyAction] = useState<ReadinessAction>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [keyStatus, runtimeStatus, checkpointStatus] = await Promise.all([
        getApiKeyStatus().catch(() => []),
        getProjectRuntimeStatus(sessionId).catch(() => null),
        getProjectCheckpointStatus(sessionId).catch(() => null),
      ]);
      setKeys(keyStatus);
      setRuntime(runtimeStatus);
      setCheckpoint(checkpointStatus);
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const readiness = useMemo(() => deriveStartReadiness({
    workspace: activeWorkspace,
    providerId: selectedProvider,
    providerLabel: getProviderLabel(selectedProvider),
    keyStatuses: keys,
    runtime,
    checkpoint,
  }), [activeWorkspace, checkpoint, keys, runtime, selectedProvider]);

  const runAction = async (action: ReadinessAction) => {
    if (!action || busyAction) return;
    setBusyAction(action);
    try {
      if (action === "open_settings") {
        window.dispatchEvent(new Event("forge:open-settings"));
      } else if (action === "start_preview") {
        await startProjectDevServer(sessionId);
        await refresh();
      } else if (action === "create_checkpoint") {
        await createProjectCheckpoint(sessionId);
        await refresh();
      }
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <div className="mx-auto max-w-[760px] rounded-lg border" style={{ background: "var(--card)", borderColor: "var(--border)" }}>
      <div className="flex items-center justify-between gap-3 border-b px-4 py-3" style={{ borderColor: "var(--border)" }}>
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-sm font-medium text-foreground">
            <CheckCircle2 className="size-4" style={{ color: readiness.issueCount === 0 ? "#4A9E6B" : "#D4A853" }} />
            {readiness.title}
          </div>
          <div className="mt-1 text-xs text-muted-foreground">{readiness.subtitle}</div>
        </div>
        <button
          type="button"
          onClick={refresh}
          className="rounded p-1.5 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
          title="刷新准备状态"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </button>
      </div>

      <div className="divide-y divide-border">
        {readiness.rows.map((row) => (
          <div key={row.label} className="grid items-center gap-3 px-4 py-3 text-xs sm:grid-cols-[128px_1fr_auto]">
            <div className="flex min-w-0 items-center gap-2 text-muted-foreground">
              <ReadinessIcon label={row.label} tone={row.tone} />
              <span>{row.label}</span>
            </div>
            <div className="min-w-0 truncate text-foreground/90">{row.value}</div>
            {row.action && row.actionLabel ? (
              <button
                type="button"
                disabled={busyAction === row.action}
                onClick={() => runAction(row.action)}
                className="inline-flex h-7 items-center justify-center rounded-md border border-border bg-secondary/50 px-2.5 text-[11px] text-foreground transition-colors hover:bg-secondary disabled:cursor-default disabled:opacity-70"
              >
                {busyAction === row.action ? "处理中" : row.actionLabel}
              </button>
            ) : (
              <span className="hidden text-[11px] text-muted-foreground/55 sm:block">{toneLabel(row.tone)}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function ReadinessIcon({ label, tone }: { label: string; tone: ReadinessTone }) {
  const color = tone === "ready" ? "#4A9E6B" : tone === "blocked" ? "#D47777" : "#D4A853";
  const iconClass = "size-3.5 shrink-0";
  if (label === "工作空间") return <FolderOpen className={iconClass} style={{ color }} />;
  if (label === "模型密钥") return <KeyRound className={iconClass} style={{ color }} />;
  if (label === "预览") return <MonitorPlay className={iconClass} style={{ color }} />;
  return <ShieldCheck className={iconClass} style={{ color }} />;
}

function toneLabel(tone: ReadinessTone) {
  if (tone === "ready") return "就绪";
  if (tone === "blocked") return "需要处理";
  if (tone === "warning") return "可稍后处理";
  return "待确认";
}
