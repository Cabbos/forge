import type { ReactNode } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeIcon } from "@/components/primitives/icon";
import { capabilityIconMeta } from "@/lib/capability-icons";
import type { CapabilityInfo } from "@/lib/tauri";

export function CapabilityRow({
  capability,
  description,
  nameClassName = "",
  descriptionClassName = "",
  action,
}: {
  capability: CapabilityInfo;
  description: ReactNode;
  nameClassName?: string;
  descriptionClassName?: string;
  action: ReactNode;
}) {
  const enabled = capability.enabled !== false;
  const meta = capabilityIconMeta(capability.kind);

  return (
    <div className="forge-capability-row" data-state={enabled ? "enabled" : "disabled"}>
      <ForgeIcon icon={meta.icon} tone={meta.tone} disabled={!enabled} />
      <div className="forge-capability-copy">
        <div className={`forge-capability-name ${nameClassName}`.trim()}>{capability.name}</div>
        <div className={`forge-capability-description ${descriptionClassName}`.trim()}>{description}</div>
      </div>
      {action}
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
