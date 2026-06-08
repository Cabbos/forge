import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ArrowUpRight, ClipboardCheck, ExternalLink, FileText, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import type { DeliveryPrimaryActionView, DeliverySummaryItem } from "@/lib/turn-closure";
import { ForgeIcon } from "@/components/primitives/icon";

export function DeliverySummaryItemView({ item }: { item: DeliverySummaryItem }) {
  return (
    <div data-testid="delivery-summary-item" className="forge-delivery-item" data-delivery-kind={item.kind}>
      <div className="forge-delivery-item-icon">{itemIcon(item)}</div>
      <div className="min-w-0">
        <div className="forge-delivery-label">{item.label}</div>
        <div className="forge-delivery-value">{item.value}</div>
      </div>
    </div>
  );
}

export function DeliveryPrimaryAction({
  action,
  loaded,
  sessionId,
  onClick,
}: {
  action: DeliveryPrimaryActionView;
  loaded: boolean;
  sessionId?: string;
  onClick: () => void;
}) {
  return (
    <div data-testid="delivery-action-bar" className="forge-delivery-action-bar">
      <ButtonPrimitive
        type="button"
        onClick={onClick}
        data-session-id={sessionId}
        data-testid="delivery-primary-action"
        data-loaded={loaded ? "true" : "false"}
        className="forge-delivery-action"
      >
        {loaded ? <ForgeIcon icon={ArrowUpRight} tone="action" contained={false} className="size-3.5" /> : primaryIcon(action.action)}
        {loaded ? "已放入" : action.label}
      </ButtonPrimitive>
    </div>
  );
}

function itemIcon(item: DeliverySummaryItem): ReactNode {
  switch (item.kind) {
    case "preview":
      return <ForgeIcon icon={ExternalLink} tone="context" contained={false} className="size-3.5" />;
    case "checkpoint":
      return <ForgeIcon icon={ShieldCheck} tone="safety" contained={false} className="size-3.5" />;
    case "verification":
      return <ForgeIcon icon={ClipboardCheck} tone="safety" contained={false} className="size-3.5" />;
    case "record":
      return <ForgeIcon icon={FileText} tone="context" contained={false} className="size-3.5" />;
    case "next":
      return <ForgeIcon icon={ClipboardCheck} tone="reasoning" contained={false} className="size-3.5" />;
  }
}

function primaryIcon(action: string): ReactNode {
  if (action === "open_records") return <ForgeIcon icon={FileText} tone="context" contained={false} className="size-3.5" />;
  if (action === "continue_fix") return <ForgeIcon icon={ArrowUpRight} tone="action" contained={false} className="size-3.5" />;
  return <ForgeIcon icon={ShieldCheck} tone="safety" contained={false} className="size-3.5" />;
}
