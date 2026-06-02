import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  const [runtime, setRuntime] = useState<ProjectRuntimeStatus | null>(null);
  const [checkpoint, setCheckpoint] = useState<ProjectCheckpointStatus | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [actionBusy, setActionBusy] = useState<DeliveryAction | null>(null);
  const [expanded, setExpanded] = useState(false);
  const workingDir = session?.workingDir ?? activeWorkspace?.path ?? null;

  const refresh = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [runtimeStatus, checkpointStatus] = await Promise.all([
        getProjectRuntimeStatus(sessionId ?? undefined, workingDir),
        getProjectCheckpointStatus(sessionId ?? undefined, workingDir),
      ]);
      setRuntime(runtimeStatus);
      setCheckpoint(checkpointStatus);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [sessionId, workingDir]);

  const runDeliveryAction = useCallback(async (action: DeliveryAction) => {
    setActionBusy(action);
    setError("");
    try {
      if (action === "start_preview") {
        const runtimeStatus = await startProjectDevServer(sessionId ?? undefined, workingDir);
        setRuntime(runtimeStatus);
      } else if (action === "open_preview") {
        const runtimeStatus = await openProjectPreview(sessionId ?? undefined, workingDir);
        setRuntime(runtimeStatus);
      } else if (action === "create_checkpoint") {
        const checkpointStatus = await createProjectCheckpoint(sessionId ?? undefined, workingDir);
        setCheckpoint(checkpointStatus);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionBusy(null);
    }
  }, [refresh, sessionId, workingDir]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const projectName = useMemo(() => {
    const path = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "");
    if (!path) return "未选择项目";
    return path.split("/").filter(Boolean).pop() || path;
  }, [runtime?.working_dir, checkpoint?.working_dir]);

  const projectPath = normalizeProjectPath(runtime?.working_dir || checkpoint?.working_dir || "") || "暂无项目路径";
  const projectPathLabel = projectPath === "暂无项目路径" ? "未选择项目" : "当前项目";
  const delivery = getDeliveryConfidence(runtime, checkpoint);
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
    dependencies: [delivery.preview.label, delivery.checkpoint.label, expanded, error],
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
      checkpoint={checkpoint}
      error={error}
      expanded={expanded}
      loading={loading}
      runtime={runtime}
      onRefresh={refresh}
      onRunDeliveryAction={runDeliveryAction}
      onToggleExpanded={() => setExpanded((value) => !value)}
    />
  );
}

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}
