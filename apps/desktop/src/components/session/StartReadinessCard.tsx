import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2, Circle, FolderOpen, GitBranch, KeyRound, Play, RefreshCw } from "lucide-react";
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
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import type { ForgeIconTone } from "@/lib/capability-icons";

interface StartReadinessCardProps {
  sessionId?: string;
  variant?: "panel" | "setup-strip";
  showDetails?: boolean;
}

export function StartReadinessCard({ sessionId, variant = "panel", showDetails = false }: StartReadinessCardProps) {
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
  const workspaceRow = readiness.rows.find((row) => row.label === "当前项目");
  const keyRow = readiness.rows.find((row) => row.label === "模型密钥");
  const panelState = readiness.issueCount === 0 ? "ready" : primaryAction?.tone === "blocked" ? "blocked" : "attention";
  const secondaryStatus = [
    workspaceRow?.tone === "ready" ? workspaceRow.value.replace("当前项目：", "") : null,
    keyRow?.tone === "blocked" ? keyRow.value : null,
  ].filter(Boolean).join(" · ");

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

  if (variant === "setup-strip") {
    const setupAction = primaryAction?.tone === "blocked" ? primaryAction : null;
    if (!setupAction?.action || !setupAction.actionLabel) return null;

    return (
      <div data-testid="start-readiness" className="mx-auto w-full max-w-[460px] px-1 py-1">
        <div data-testid="start-readiness-panel" className="forge-readiness-strip" data-state={panelState}>
          <div className="forge-readiness-strip-icon" aria-hidden="true">
            <KeyRound className="size-3.5" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium text-foreground">{readiness.title}</div>
            <div className="mt-0.5 truncate text-xs text-muted-foreground">{setupAction.value}</div>
          </div>
          <button
            type="button"
            disabled={busyAction === setupAction.action}
            onClick={() => runAction(setupAction.action)}
            className="forge-action justify-center disabled:cursor-default disabled:opacity-70"
          >
            {busyAction === setupAction.action ? "处理中" : setupAction.actionLabel}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="start-readiness" className="mx-auto max-w-[760px] px-1 py-1">
      <div
        data-testid="start-readiness-panel"
        className="forge-readiness-panel"
        data-state={panelState}
        data-details={showDetails ? "true" : "false"}
      >
        <div className="forge-readiness-header">
          <div className="flex min-w-0 items-start gap-3">
            <div className="forge-readiness-orb" aria-hidden="true">
              <CheckCircle2 className="size-4" />
            </div>
            <div className="min-w-0">
              <div className="text-sm font-medium text-foreground">{readiness.title}</div>
              <div className="mt-1 max-w-[34rem] truncate text-xs text-muted-foreground">
                {secondaryStatus || readiness.subtitle || (primaryAction ? primaryAction.value : "描述你想做什么，Forge 会在当前项目里继续。")}
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
              aria-label="刷新准备状态"
            >
              <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
            </button>
          </div>
        </div>

        {showDetails && (
          <div className="forge-readiness-grid" aria-label="开始前状态">
            {readiness.rows.map((row) => {
              const RowIcon = readinessIconFor(row.label);
              return (
                <div key={row.label} data-testid="start-readiness-row" className="forge-readiness-row" data-tone={row.tone}>
                  <ForgeIcon icon={RowIcon} tone={readinessIconTone(row.label, row.tone)} contained={false} className="size-3.5" />
                  <div className="min-w-0 flex-1">
                    <div className="forge-readiness-row-label">{row.label}</div>
                    <div className="forge-readiness-row-value">{row.value}</div>
                  </div>
                  {row.action && row.actionLabel ? (
                    <button
                      type="button"
                      disabled={busyAction === row.action}
                      onClick={() => runAction(row.action)}
                      className="forge-readiness-row-action disabled:cursor-default disabled:opacity-70"
                    >
                      {busyAction === row.action ? "处理中" : row.actionLabel}
                    </button>
                  ) : (
                    <span className="forge-readiness-row-state">
                      {row.tone === "ready" ? "就绪" : row.tone === "blocked" ? "待处理" : "可选"}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function readinessIconFor(label: string) {
  if (label === "当前项目") return FolderOpen;
  if (label === "模型密钥") return KeyRound;
  if (label === "预览") return Play;
  if (label === "检查点") return GitBranch;
  return Circle;
}

function readinessIconTone(label: string, tone: string): ForgeIconTone {
  if (tone === "blocked") return "danger";
  if (label === "当前项目") return "context";
  if (label === "模型密钥" || label === "检查点") return "safety";
  if (label === "预览") return "action";
  return tone === "ready" ? "safety" : "neutral";
}
