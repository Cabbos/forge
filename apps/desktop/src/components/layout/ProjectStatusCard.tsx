import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, ChevronRight, ExternalLink, Folder, GitBranch, Play, RefreshCw, ShieldCheck, type LucideIcon } from "lucide-react";
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
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeIcon } from "@/components/primitives/icon";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { ForgeSurface } from "@/components/primitives/surface";
import type { ForgeIconTone } from "@/lib/capability-icons";
import { useActiveWorkspace, useStore } from "@/store";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

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
    <ForgeSurface as="section" ref={cardRef} data-testid="project-status-card" className="forge-project-status">
      <div data-forge-motion="project-status-entry" className="forge-project-status-header">
        <div className="forge-project-status-title-group">
          <ForgeIcon icon={Folder} tone="context" contained={false} className="size-3.5" />
          <div className="min-w-0" title={projectPath}>
            <div className="forge-project-status-title">{projectName}</div>
            <div className="forge-project-status-path">{projectPathLabel}</div>
          </div>
        </div>
        <ForgeIconButton
          onClick={refresh}
          className="size-7"
          title="刷新交付状态"
          aria-label="刷新交付状态"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </ForgeIconButton>
      </div>

      <div className="forge-project-status-body">
        <div data-testid="project-status-summary" data-forge-motion="project-status-entry" className="forge-project-status-summary">
          <ProjectStatusMetric
            icon={Play}
            iconTone="action"
            color={delivery.preview.color}
            label="预览"
            value={delivery.preview.label}
          />
          <ProjectStatusMetric
            icon={GitBranch}
            iconTone="safety"
            color={delivery.checkpoint.color}
            label="检查点"
            value={delivery.checkpoint.label}
          />
        </div>
        <div data-forge-motion="project-status-entry" className="forge-project-status-next">
          {delivery.nextAction}
        </div>
        {deliveryActions.length > 0 && (
          <div data-forge-motion="project-status-entry" className="forge-project-status-actions">
            {deliveryActions.map(({ action, label }) => (
              <DeliveryButton
                key={action}
                action={action}
                busy={actionBusy === action}
                label={label}
                onClick={runDeliveryAction}
              />
            ))}
          </div>
        )}
        {error && (
          <div data-forge-motion="project-status-entry" role="status" className="forge-project-status-error">
            {error}
          </div>
        )}
      </div>

      <button
        type="button"
        data-forge-motion="project-status-entry"
        onClick={() => setExpanded((value) => !value)}
        className="forge-project-status-disclosure"
      >
        <span>{expanded ? "收起详情" : "展开详情"}</span>
        {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
      </button>

      {expanded && (
        <div data-forge-motion="project-status-entry" className="forge-project-status-details">
          <DetailLine label="预览状态" value={runtime?.message || "暂无"} />
          <DetailLine label="预览地址" value={runtime?.url || "暂无"} />
          <DetailLine label="运行命令" value={runtime?.command || "未检测到"} />
          <DetailLine label="检查点" value={checkpoint?.message || "暂无"} />
          {checkpoint?.last_checkpoint && (
            <div className="forge-project-status-commit">
              <ForgeIcon icon={GitBranch} tone="safety" contained={false} className="size-3.5" />
              <span className="truncate">{checkpoint.last_checkpoint.head}</span>
            </div>
          )}
        </div>
      )}
    </ForgeSurface>
  );
}

function ProjectStatusMetric({
  icon,
  iconTone,
  color,
  label,
  value,
}: {
  icon: LucideIcon;
  iconTone: ForgeIconTone;
  color: string;
  label: string;
  value: string;
}) {
  return (
    <div data-testid="project-status-metric" className="forge-project-status-metric">
      <ForgeIcon icon={icon} tone={iconTone} contained={false} className="size-3.5" />
      <div className="min-w-0">
        <div className="forge-project-status-label">{label}</div>
        <div className="forge-project-status-value">{value}</div>
      </div>
      <span className="forge-project-status-dot" style={{ backgroundColor: color, color }} aria-hidden="true" />
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
  const tone: ForgeIconTone = action === "create_checkpoint" ? "safety" : "action";

  return (
    <ForgeActionButton
      data-testid="project-status-action"
      disabled={busy}
      onClick={() => onClick(action)}
      className="forge-project-status-action disabled:cursor-default disabled:opacity-70"
    >
      <ForgeIcon icon={Icon} tone={tone} contained={false} className={cn("size-3.5", busy && "animate-pulse")} />
      {label}
    </ForgeActionButton>
  );
}

function DetailLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="forge-project-status-detail-line">
      <span className="forge-project-status-detail-label">{label}</span>
      <span className="forge-project-status-detail-value">{value}</span>
    </div>
  );
}

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}
