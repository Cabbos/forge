import { useCallback, useEffect, useMemo, useState } from "react";
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
import { StartReadinessView } from "./StartReadinessView";

interface StartReadinessCardProps {
  sessionId?: string;
  variant?: "panel" | "setup-strip";
  showDetails?: boolean;
}

export function StartReadinessCard({ sessionId, variant = "panel", showDetails = false }: StartReadinessCardProps) {
  const activeWorkspace = useActiveWorkspace();
  const session = useStore((s) => sessionId ? s.sessions.get(sessionId) ?? null : null);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const [keys, setKeys] = useState<KeyStatus[]>([]);
  const [runtime, setRuntime] = useState<ProjectRuntimeStatus | null>(null);
  const [checkpoint, setCheckpoint] = useState<ProjectCheckpointStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [busyAction, setBusyAction] = useState<ReadinessAction>(null);
  const workingDir = session?.workingDir ?? activeWorkspace?.path ?? null;

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [keyStatus, runtimeStatus, checkpointStatus] = await Promise.all([
        getApiKeyStatus().catch(() => []),
        getProjectRuntimeStatus(sessionId, workingDir).catch(() => null),
        getProjectCheckpointStatus(sessionId, workingDir).catch(() => null),
      ]);
      setKeys(keyStatus);
      setRuntime(runtimeStatus);
      setCheckpoint(checkpointStatus);
    } finally {
      setLoading(false);
    }
  }, [sessionId, workingDir]);

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
        await startProjectDevServer(sessionId, workingDir);
        await refresh();
      } else if (action === "create_checkpoint") {
        await createProjectCheckpoint(sessionId, workingDir);
        await refresh();
      }
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <StartReadinessView
      readiness={readiness}
      primaryAction={primaryAction}
      panelState={panelState}
      secondaryStatus={secondaryStatus}
      variant={variant}
      showDetails={showDetails}
      loading={loading}
      busyAction={busyAction}
      onRefresh={refresh}
      onRunAction={runAction}
    />
  );
}
