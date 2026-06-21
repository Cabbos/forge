import { useCallback, useMemo, useState } from "react";
import { useActiveWorkspace, useStore } from "@/store";
import {
  createProjectCheckpoint,
  startProjectDevServer,
} from "@/lib/tauri";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { useApiKeyStatusQuery } from "@/hooks/queries/useApiKeyStatusQuery";
import { useProjectRuntimeStatusQuery } from "@/hooks/queries/useProjectRuntimeStatusQuery";
import { useProjectCheckpointStatusQuery } from "@/hooks/queries/useProjectCheckpointStatusQuery";
import { useProviderCatalog } from "@/hooks/queries/useProviderCatalogQuery";
import { getProviderDefinition } from "@/lib/providers";
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
  const selectedModel = useStore((s) => s.selectedModel);
  const providers = useProviderCatalog(true);
  const provider = useMemo(() => getProviderDefinition(selectedProvider, providers), [providers, selectedProvider]);
  const [busyAction, setBusyAction] = useState<ReadinessAction>(null);
  const queryClient = useQueryClient();
  const workingDir = session?.workingDir ?? activeWorkspace?.path ?? null;

  const {
    data: keys = [],
    isFetching: keysFetching,
    isError: keysIsError,
    error: keysError,
    refetch: refetchKeys,
  } = useApiKeyStatusQuery(true);
  const {
    data: runtime = null,
    isFetching: runtimeFetching,
    isError: runtimeIsError,
    error: runtimeError,
    refetch: refetchRuntime,
  } = useProjectRuntimeStatusQuery(sessionId, workingDir, !!workingDir);
  const {
    data: checkpoint = null,
    isFetching: checkpointFetching,
    isError: checkpointIsError,
    error: checkpointError,
    refetch: refetchCheckpoint,
  } = useProjectCheckpointStatusQuery(sessionId, workingDir, !!workingDir);
  const loading = keysFetching || runtimeFetching || checkpointFetching;
  const queryError = getQueryErrorMessage(
    keysIsError ? keysError : null,
    runtimeIsError ? runtimeError : null,
    checkpointIsError ? checkpointError : null,
  );

  const refresh = useCallback(async () => {
    await Promise.all([refetchKeys(), refetchRuntime(), refetchCheckpoint()]);
  }, [refetchKeys, refetchRuntime, refetchCheckpoint]);

  const readiness = useMemo(() => deriveStartReadiness({
    workspace: activeWorkspace,
    providerId: selectedProvider,
    providerLabel: provider.label,
    provider,
    model: selectedModel,
    keyStatuses: keys,
    runtime,
    checkpoint,
  }), [activeWorkspace, checkpoint, keys, provider, runtime, selectedModel, selectedProvider]);

  const primaryAction = readiness.rows.find((row) => row.action && row.actionLabel);
  const workspaceRow = readiness.rows.find((row) => row.label === "当前项目");
  const keyRow = readiness.rows.find((row) => row.label === "模型密钥");
  const evidenceRow = readiness.rows.find((row) => row.label === "Provider 证据");
  const panelState = queryError ? "attention" : readiness.issueCount === 0 ? "ready" : primaryAction?.tone === "blocked" ? "blocked" : "attention";
  const secondaryStatus = queryError
    ? `状态读取失败：${queryError}`
    : evidenceRow?.tone === "blocked"
      ? readiness.subtitle
    : [
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
        await queryClient.invalidateQueries({ queryKey: queryKeys.projectRuntimeStatus(sessionId, workingDir) });
      } else if (action === "create_checkpoint") {
        await createProjectCheckpoint(sessionId, workingDir);
        await queryClient.invalidateQueries({ queryKey: queryKeys.projectCheckpointStatus(sessionId, workingDir) });
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
