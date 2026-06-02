import { ExternalLink, Play, ShieldCheck } from "lucide-react";
import type { DeliveryAction } from "@/lib/delivery-confidence";
import { cn } from "@/lib/utils";
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeIcon } from "@/components/primitives/icon";
import type { ForgeIconTone } from "@/lib/capability-icons";

export function ProjectStatusActions({
  deliveryActions,
  actionBusy,
  onRunDeliveryAction,
}: {
  deliveryActions: Array<{ action: DeliveryAction; label: string }>;
  actionBusy: DeliveryAction | null;
  onRunDeliveryAction: (action: DeliveryAction) => void;
}) {
  if (deliveryActions.length === 0) return null;

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
