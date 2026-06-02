import type { RefObject } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ChevronDown, ChevronRight, Folder, RefreshCw } from "lucide-react";
import type { DeliveryAction, DeliveryConfidence } from "@/lib/delivery-confidence";
import type { ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { ForgeIcon } from "@/components/primitives/icon";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { ForgeSurface } from "@/components/primitives/surface";
import { ProjectStatusActions } from "./ProjectStatusActions";
import { ProjectStatusDetails } from "./ProjectStatusDetails";
import { ProjectStatusSummary } from "./ProjectStatusSummary";

interface ProjectStatusViewProps {
  cardRef: RefObject<HTMLElement | null>;
  projectName: string;
  projectPath: string;
  projectPathLabel: string;
  delivery: DeliveryConfidence;
  deliveryActions: Array<{ action: DeliveryAction; label: string }>;
  actionBusy: DeliveryAction | null;
  checkpoint: ProjectCheckpointStatus | null;
  error: string;
  expanded: boolean;
  loading: boolean;
  runtime: ProjectRuntimeStatus | null;
  onRefresh: () => void;
  onRunDeliveryAction: (action: DeliveryAction) => void;
  onToggleExpanded: () => void;
}

export function ProjectStatusView({
  cardRef,
  projectName,
  projectPath,
  projectPathLabel,
  delivery,
  deliveryActions,
  actionBusy,
  checkpoint,
  error,
  expanded,
  loading,
  runtime,
  onRefresh,
  onRunDeliveryAction,
  onToggleExpanded,
}: ProjectStatusViewProps) {
  return (
    <ForgeSurface as="section" ref={cardRef as RefObject<HTMLElement>} data-testid="project-status-card" className="forge-project-status">
      <div data-forge-motion="project-status-entry" className="forge-project-status-header">
        <div className="forge-project-status-title-group">
          <ForgeIcon icon={Folder} tone="context" contained={false} className="size-3.5" />
          <div className="min-w-0" title={projectPath}>
            <div className="forge-project-status-title">{projectName}</div>
            <div className="forge-project-status-path">{projectPathLabel}</div>
          </div>
        </div>
        <ForgeIconButton
          onClick={onRefresh}
          className="size-7"
          title="刷新交付状态"
          aria-label="刷新交付状态"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </ForgeIconButton>
      </div>

      <div className="forge-project-status-body">
        <ProjectStatusSummary delivery={delivery} />
        <div data-forge-motion="project-status-entry" className="forge-project-status-next">
          {delivery.nextAction}
        </div>
        <ProjectStatusActions
          deliveryActions={deliveryActions}
          actionBusy={actionBusy}
          onRunDeliveryAction={onRunDeliveryAction}
        />
        {error && (
          <div data-forge-motion="project-status-entry" role="status" className="forge-project-status-error">
            {error}
          </div>
        )}
      </div>

      <ButtonPrimitive
        type="button"
        data-forge-motion="project-status-entry"
        onClick={onToggleExpanded}
        className="forge-project-status-disclosure"
      >
        <span>{expanded ? "收起详情" : "展开详情"}</span>
        {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
      </ButtonPrimitive>

      {expanded && (
        <ProjectStatusDetails checkpoint={checkpoint} runtime={runtime} />
      )}
    </ForgeSurface>
  );
}
