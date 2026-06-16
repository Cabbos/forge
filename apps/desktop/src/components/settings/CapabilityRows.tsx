import type { ReactNode } from "react";
import { Info } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeIcon } from "@/components/primitives/icon";
import { capabilityIconMeta } from "@/lib/capability-icons";
import type { CapabilityInfo } from "@/lib/tauri";
import type { EcosystemItemStatus } from "@/lib/tauri";

const STATUS_LABELS: Record<EcosystemItemStatus, string> = {
  healthy: "正常",
  unavailable: "不可用",
  warning: "警告",
  unknown: "未知",
};

export function CapabilityRow({
  capability,
  description,
  nameClassName = "",
  descriptionClassName = "",
  action,
  status,
  statusMessage,
  configurable,
  onDetails,
}: {
  capability: CapabilityInfo;
  description: ReactNode;
  nameClassName?: string;
  descriptionClassName?: string;
  action?: ReactNode;
  status?: EcosystemItemStatus;
  statusMessage?: string | null;
  configurable?: boolean;
  onDetails?: () => void;
}) {
  const enabled = capability.enabled !== false;
  const meta = capabilityIconMeta(capability.kind);
  const hasConfigInfo = typeof configurable === "boolean";

  return (
    <div className="forge-capability-row" data-state={enabled ? "enabled" : "disabled"}>
      <ForgeIcon icon={meta.icon} tone={meta.tone} disabled={!enabled} />
      <div className="forge-capability-copy">
        <div className={`forge-capability-name ${nameClassName}`.trim()}>
          {capability.name}
          {status && status !== "unknown" && (
            <span
              className="forge-capability-status-badge"
              data-status={status}
              title={statusMessage ?? STATUS_LABELS[status]}
            >
              {STATUS_LABELS[status]}
            </span>
          )}
        </div>
        <div className={`forge-capability-description ${descriptionClassName}`.trim()}>
          {description}
          {statusMessage && (
            <span className="forge-capability-status-message"> — {statusMessage}</span>
          )}
          {hasConfigInfo && !configurable && enabled && (
            <span className="forge-capability-config-hint"> · 暂不支持界面配置</span>
          )}
        </div>
      </div>
      <div className="forge-capability-actions">
        {onDetails && (
          <ButtonPrimitive
            type="button"
            aria-label={`${capability.name} 详情`}
            title="详情"
            className="forge-capability-detail-button"
            onClick={onDetails}
          >
            <Info className="size-3" />
          </ButtonPrimitive>
        )}
        {action}
      </div>
    </div>
  );
}

export function CapabilitySectionHeader({ label, count }: { label: string; count?: number }) {
  return (
    <div className="forge-capability-section-header">
      <h5>{label}</h5>
      {typeof count === "number" && <span className="forge-capability-count">{count} 个</span>}
    </div>
  );
}

export function CapabilityStatusButton({ enabled, onClick }: { enabled: boolean; onClick: () => void }) {
  return (
    <ButtonPrimitive
      type="button"
      aria-pressed={enabled}
      data-state={enabled ? "enabled" : "disabled"}
      className="forge-capability-toggle"
      onClick={onClick}
    >
      {enabled ? "已启用" : "已停用"}
    </ButtonPrimitive>
  );
}

export function CapabilitySwitch({ enabled, label, onClick }: { enabled: boolean; label: string; onClick: () => void }) {
  return (
    <ButtonPrimitive
      type="button"
      aria-label={label}
      aria-pressed={enabled}
      data-state={enabled ? "enabled" : "disabled"}
      className="forge-capability-switch"
      onClick={onClick}
    >
      <span className="forge-capability-switch-thumb" />
    </ButtonPrimitive>
  );
}
