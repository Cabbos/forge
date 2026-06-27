import { ExternalLink, Play, RotateCcw, ShieldAlert, ShieldCheck } from "lucide-react";
import type { DeliveryAction } from "@/lib/delivery-confidence";
import type { PermissionMode } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeIcon } from "@/components/primitives/icon";
import type { ForgeIconTone } from "@/lib/capability-icons";

export function ProjectStatusActions({
  deliveryActions,
  actionBusy,
  permissionBusy,
  permissionDisabledReason,
  permissionMode,
  onFullAccessCurrentProject,
  onRestoreManualConfirm,
  onRunDeliveryAction,
  onTrustCurrentProject,
}: {
  deliveryActions: Array<{ action: DeliveryAction; label: string }>;
  actionBusy: DeliveryAction | null;
  permissionBusy: boolean;
  permissionDisabledReason: string;
  permissionMode: PermissionMode;
  onFullAccessCurrentProject: () => void;
  onRestoreManualConfirm: () => void;
  onRunDeliveryAction: (action: DeliveryAction) => void;
  onTrustCurrentProject: () => void;
}) {
  const trustActive = permissionMode === "trust_current_project";
  const fullAccessActive = permissionMode === "full_access";
  const permissionAvailable = !permissionDisabledReason;
  const permissionActive = trustActive || fullAccessActive;

  return (
    <div data-forge-motion="project-status-entry" className="forge-project-status-actions">
      {deliveryActions.map(({ action, label }) => (
        <DeliveryButton
          key={action}
          action={action}
          busy={actionBusy === action}
          label={label}
          onClick={onRunDeliveryAction}
        />
      ))}
      <span
        data-testid="project-status-permission-mode"
        data-state={fullAccessActive ? "full_access" : trustActive ? "trusted" : "manual"}
        className="forge-project-status-permission-pill"
      >
        {fullAccessActive ? "完全访问" : trustActive ? "已信任" : "手动确认"}
      </span>
      <ForgeActionButton
        data-testid="project-status-permission-action"
        disabled={permissionBusy || !permissionAvailable}
        title={permissionDisabledReason || (permissionActive ? "恢复手动确认" : "信任当前项目")}
        aria-label={permissionActive ? "恢复手动确认" : "信任当前项目"}
        onClick={permissionActive ? onRestoreManualConfirm : onTrustCurrentProject}
        className="forge-project-status-action disabled:cursor-default disabled:opacity-70"
      >
        <ForgeIcon
          icon={permissionActive ? RotateCcw : ShieldCheck}
          tone="safety"
          contained={false}
          className={cn("size-3.5", permissionBusy && "animate-pulse")}
        />
        {permissionActive ? "恢复手动确认" : "信任当前项目"}
      </ForgeActionButton>
      {!permissionActive ? (
        <ForgeActionButton
          data-testid="project-status-full-access-action"
          disabled={permissionBusy || !permissionAvailable}
          title={permissionDisabledReason || "完全访问"}
          aria-label="完全访问"
          onClick={onFullAccessCurrentProject}
          className="forge-project-status-action disabled:cursor-default disabled:opacity-70"
        >
          <ForgeIcon
            icon={ShieldAlert}
            tone="safety"
            contained={false}
            className={cn("size-3.5", permissionBusy && "animate-pulse")}
          />
          完全访问
        </ForgeActionButton>
      ) : null}
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
