import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2, RefreshCw } from "lucide-react";
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
import { deriveStartReadiness, type ReadinessAction } from "@/lib/start-readiness";
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

  const primaryAction = readiness.rows.find((row) => row.action && row.actionLabel);

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
    <div data-testid="start-readiness" className="mx-auto max-w-[760px] px-1 py-1">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <CheckCircle2 className="size-4 shrink-0" style={{ color: readiness.issueCount === 0 ? "#4A9E6B" : "#D4A853" }} />
          <div className="min-w-0">
            <div className="text-sm font-medium text-foreground">{readiness.title}</div>
            <div className="truncate text-xs text-muted-foreground">
              {primaryAction ? primaryAction.value : "描述你想做什么，Forge 会在当前项目里继续。"}
            </div>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {primaryAction?.action && primaryAction.actionLabel && (
            <button
              type="button"
              disabled={busyAction === primaryAction.action}
              onClick={() => runAction(primaryAction.action)}
              className="forge-action justify-center disabled:cursor-default disabled:opacity-70"
            >
              {busyAction === primaryAction.action ? "处理中" : primaryAction.actionLabel}
            </button>
          )}
          <button
            type="button"
            onClick={refresh}
            className="forge-icon-button"
            title="刷新准备状态"
          >
            <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
          </button>
        </div>
      </div>
    </div>
  );
}
