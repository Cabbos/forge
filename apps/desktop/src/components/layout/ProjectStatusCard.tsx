import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  confirmResponse,
  createProjectCheckpoint,
  getPermissionMode,
  openProjectPreview,
  setPermissionMode,
  startProjectDevServer,
  type PermissionModeState,
} from "@/lib/tauri";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { useProjectRuntimeStatusQuery } from "@/hooks/queries/useProjectRuntimeStatusQuery";
import { useProjectCheckpointStatusQuery } from "@/hooks/queries/useProjectCheckpointStatusQuery";
import { getDeliveryConfidence, type DeliveryAction } from "@/lib/delivery-confidence";
import type { BlockState } from "@/lib/protocol";
import { parseWriteBoundary } from "@/lib/write-boundary";
import { useActiveWorkspace, useStore } from "@/store";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";
import { ProjectStatusView } from "./ProjectStatusView";

interface ProjectStatusCardProps {
  sessionId: string | null;
}

export function ProjectStatusCard({ sessionId }: ProjectStatusCardProps) {
  const cardRef = useRef<HTMLElement>(null);
  const activeWorkspace = useActiveWorkspace();
  const session = useStore((s) => sessionId ? s.sessions.get(sessionId) ?? null : null);
  const [error, setError] = useState("");
  const queryClient = useQueryClient();
  const [actionBusy, setActionBusy] = useState<DeliveryAction | null>(null);
  const [permissionMode, setPermissionModeState] = useState<PermissionModeState>(manualPermissionMode);
  const [permissionBusy, setPermissionBusy] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const workingDir = session?.workingDir ?? activeWorkspace?.path ?? null;
  const updateBlock = useStore((s) => s.updateBlock);

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
  const loading = runtimeFetching || checkpointFetching;
  const queryError = getQueryErrorMessage(
    runtimeIsError ? runtimeError : null,
    checkpointIsError ? checkpointError : null,
  );
  const displayError = error || (queryError ? `状态读取失败：${queryError}` : "");

  const refresh = useCallback(async () => {
    await Promise.all([refetchRuntime(), refetchCheckpoint()]);
  }, [refetchRuntime, refetchCheckpoint]);

  const loadPermissionMode = useCallback(async () => {
    if (!sessionId || !workingDir) {
      setPermissionModeState(manualPermissionMode);
      return;
    }
    try {
      setPermissionModeState(await getPermissionMode(sessionId, workingDir));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [sessionId, workingDir]);

  useEffect(() => {
    void loadPermissionMode();
  }, [loadPermissionMode]);

  const runDeliveryAction = useCallback(async (action: DeliveryAction) => {
    setActionBusy(action);
    setError("");
    try {
      if (action === "start_preview") {
        await startProjectDevServer(sessionId ?? undefined, workingDir);
        await queryClient.invalidateQueries({ queryKey: queryKeys.projectRuntimeStatus(sessionId, workingDir) });
      } else if (action === "open_preview") {
        await openProjectPreview(sessionId ?? undefined, workingDir);
        await queryClient.invalidateQueries({ queryKey: queryKeys.projectRuntimeStatus(sessionId, workingDir) });
      } else if (action === "create_checkpoint") {
        await createProjectCheckpoint(sessionId ?? undefined, workingDir);
        await queryClient.invalidateQueries({ queryKey: queryKeys.projectCheckpointStatus(sessionId, workingDir) });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionBusy(null);
    }
  }, [queryClient, sessionId, workingDir]);

  const trustCurrentProject = useCallback(async () => {
    if (!sessionId || !workingDir) return;
    setPermissionBusy(true);
    setError("");
    try {
      const nextMode = await setPermissionMode({
        sessionId,
        mode: "trust_current_project",
        workspacePath: workingDir,
      });
      setPermissionModeState(nextMode);

      const pendingConfirm = findLatestPendingWorkspaceConfirm(session?.blocks ?? [], workingDir, false);
      if (pendingConfirm) {
        await confirmResponse(pendingConfirm.block_id, true);
        updateBlock(sessionId, pendingConfirm.block_id, {
          metadata: { ...pendingConfirm.metadata, confirmed: true, answer: true },
        });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPermissionBusy(false);
    }
  }, [session?.blocks, sessionId, updateBlock, workingDir]);

  const fullAccessCurrentProject = useCallback(async () => {
    if (!sessionId || !workingDir) return;
    setPermissionBusy(true);
    setError("");
    try {
      const nextMode = await setPermissionMode({
        sessionId,
        mode: "full_access",
        workspacePath: workingDir,
      });
      setPermissionModeState(nextMode);

      const pendingConfirm = findLatestPendingWorkspaceConfirm(session?.blocks ?? [], workingDir, true);
      if (pendingConfirm) {
        await confirmResponse(pendingConfirm.block_id, true);
        updateBlock(sessionId, pendingConfirm.block_id, {
          metadata: { ...pendingConfirm.metadata, confirmed: true, answer: true },
        });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPermissionBusy(false);
    }
  }, [session?.blocks, sessionId, updateBlock, workingDir]);

  const restoreManualConfirm = useCallback(async () => {
    if (!sessionId || !workingDir) return;
    setPermissionBusy(true);
    setError("");
    try {
      setPermissionModeState(await setPermissionMode({
        sessionId,
        mode: "manual_confirm",
        workspacePath: workingDir,
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPermissionBusy(false);
    }
  }, [sessionId, workingDir]);

  const projectName = useMemo(() => {
    const path = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "");
    if (!path) return "未选择项目";
    return path.split("/").filter(Boolean).pop() || path;
  }, [runtime?.working_dir, checkpoint?.working_dir]);

  const projectPath = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "") || "暂无项目路径";
  const projectPathLabel = projectPath === "暂无项目路径" ? "未选择项目" : "当前项目";
  const delivery = getDeliveryConfidence(runtime, checkpoint);
  const permissionDisabledReason = !workingDir
    ? "需要先打开一个项目"
    : !sessionId
      ? "需要先打开一个对话"
      : "";
  const deliveryActions = [
    delivery.preview.action && delivery.preview.actionLabel
      ? { action: delivery.preview.action, label: delivery.preview.actionLabel }
      : null,
    delivery.checkpoint.action && delivery.checkpoint.actionLabel
      ? { action: delivery.checkpoint.action, label: delivery.checkpoint.actionLabel }
      : null,
  ].filter(Boolean) as Array<{ action: DeliveryAction; label: string }>;

  useGSAP(() => {
    if (prefersReducedMotion()) return;
    const root = cardRef.current;
    if (!root) return;

    const entries = gsap.utils.toArray<HTMLElement>("[data-forge-motion='project-status-entry']", root);
    if (entries.length === 0) return;

    gsap.fromTo(
      entries,
      { autoAlpha: 0, y: 5, scale: 0.996 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        stagger: 0.024,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, {
    scope: cardRef,
    dependencies: [delivery.preview.label, delivery.checkpoint.label, expanded, displayError],
  });

  return (
    <ProjectStatusView
      cardRef={cardRef}
      projectName={projectName}
      projectPath={projectPath}
      projectPathLabel={projectPathLabel}
      delivery={delivery}
      deliveryActions={deliveryActions}
      actionBusy={actionBusy}
      permissionBusy={permissionBusy}
      permissionDisabledReason={permissionDisabledReason}
      permissionMode={permissionMode.mode}
      checkpoint={checkpoint}
      error={displayError}
      expanded={expanded}
      loading={loading}
      runtime={runtime}
      onRefresh={refresh}
      onFullAccessCurrentProject={fullAccessCurrentProject}
      onRestoreManualConfirm={restoreManualConfirm}
      onRunDeliveryAction={runDeliveryAction}
      onTrustCurrentProject={trustCurrentProject}
      onToggleExpanded={() => setExpanded((value) => !value)}
    />
  );
}

const manualPermissionMode: PermissionModeState = {
  mode: "manual_confirm",
  workspace_path: null,
  session_scoped: true,
};

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}

function findLatestPendingWorkspaceConfirm(
  blocks: BlockState[],
  workingDir: string,
  allowAnyOperation: boolean,
): BlockState | null {
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block.event_type !== "confirm_ask") continue;
    if (block.metadata.confirmed === true || block.metadata.confirm_interrupted === true) continue;

    const boundary = parseWriteBoundary(block.metadata.boundary);
    if (!boundary) continue;
    if (normalizeProjectPath(boundary.workspacePath) !== normalizedWorkingDir) continue;
    if (!allowAnyOperation && !isWriteBoundaryOperation(boundary.operationLabel)) continue;
    if (!isAutoApprovableBoundary(block.metadata.boundary, workingDir, allowAnyOperation)) continue;
    return block;
  }
  return null;
}

function isWriteBoundaryOperation(operationLabel: string): boolean {
  return operationLabel === "写入文件" || operationLabel === "编辑文件" || operationLabel === "修改文件";
}

function isAutoApprovableBoundary(
  boundary: unknown,
  workingDir: string,
  allowSensitiveWorkspaceFiles: boolean,
): boolean {
  if (!boundary || typeof boundary !== "object" || Array.isArray(boundary)) return false;
  const rawFiles = (boundary as { affected_files?: unknown }).affected_files;
  if (!Array.isArray(rawFiles)) return true;
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  return rawFiles.every((file) => {
    if (typeof file !== "string") return false;
    const normalizedFile = normalizeProjectPath(file);
    const projectRelativeFile = normalizedFile.startsWith(`${normalizedWorkingDir}/`)
      ? normalizedFile.slice(normalizedWorkingDir.length + 1)
      : normalizedFile;
    if (normalizedFile.startsWith("~")) return false;
    if (normalizedFile.startsWith("/") && normalizedFile !== normalizedWorkingDir && !normalizedFile.startsWith(`${normalizedWorkingDir}/`)) return false;
    if (projectRelativeFile === ".." || projectRelativeFile.startsWith("../") || projectRelativeFile.includes("/../")) return false;
    if (!allowSensitiveWorkspaceFiles && isSensitiveProjectPath(projectRelativeFile)) return false;
    return true;
  });
}

function isSensitiveProjectPath(path: string): boolean {
  const normalized = path.replace(/\\/g, "/").toLowerCase();
  return normalized === ".env" || normalized.startsWith(".env.") || normalized.endsWith("/.env") || normalized.includes("/.env.");
}
