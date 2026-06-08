import { ShieldAlert } from "lucide-react";
import type { WriteBoundaryViewModel } from "@/lib/write-boundary";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import {
  confirmIconTone,
  confirmResolvedLabel,
  type ConfirmPromptViewModel,
} from "@/components/messages/confirmPresentation";
import { ConfirmActionBar } from "@/components/messages/ConfirmActions";
import {
  ConfirmBoundaryGrid,
  ConfirmResolvedSummary,
} from "@/components/messages/ConfirmBoundaryViews";
import { ForgeIcon } from "@/components/primitives/icon";

export function ConfirmBoundaryResolvedView({
  boundary,
  answer,
}: {
  boundary: WriteBoundaryViewModel;
  answer: boolean | null;
}) {
  const resolvedStatus = (
    <span className="forge-confirm-resolved" data-state={answer ? "approved" : "cancelled"}>
      {confirmResolvedLabel(answer)}
    </span>
  );

  return (
    <MessagePanel
      tone="default"
      className="forge-confirm-card"
      data-confirm-state="resolved"
    >
      <MessagePanelHeader
        icon={<ForgeIcon icon={ShieldAlert} tone={answer ? "safety" : "danger"} contained={false} className="size-3.5" />}
        title={boundary.title}
        actions={resolvedStatus}
      />

      <ConfirmResolvedSummary boundary={boundary} />
    </MessagePanel>
  );
}

export function ConfirmBoundaryPendingView({
  boundary,
  onResponse,
}: {
  boundary: WriteBoundaryViewModel;
  onResponse: (approved: boolean) => void;
}) {
  const iconTone = confirmIconTone(boundary.riskTone);

  return (
    <MessagePanel
      tone={boundary.riskTone === "high" ? "danger" : "warning"}
      className="forge-confirm-card"
      data-confirm-state="pending"
    >
      <MessagePanelHeader
        icon={<ForgeIcon icon={ShieldAlert} tone={iconTone} />}
        title={boundary.title}
        meta="继续前确认改动范围"
      />

      <ConfirmBoundaryGrid boundary={boundary} />

      <ConfirmActionBar responded={false} answer={null} onResponse={onResponse} />
    </MessagePanel>
  );
}

export function ConfirmPromptView({
  prompt,
  responded,
  answer,
  onResponse,
}: {
  prompt: ConfirmPromptViewModel;
  responded: boolean;
  answer: boolean | null;
  onResponse: (approved: boolean) => void;
}) {
  return (
    <MessagePanel tone="warning" className="forge-confirm-card" data-confirm-state={responded ? "resolved" : "pending"}>
      <MessagePanelHeader
        icon={<ForgeIcon icon={ShieldAlert} tone="safety" />}
        title={prompt.kindLabel}
        meta="继续前需要你确认"
      />
      <div className="px-3 py-2.5">
        <p className="whitespace-pre-wrap text-sm leading-relaxed text-foreground">{prompt.question}</p>
        <p className="mt-2 text-xs leading-relaxed text-muted-foreground">{prompt.helperText}</p>
      </div>

      <ConfirmActionBar responded={responded} answer={answer} onResponse={onResponse} />
    </MessagePanel>
  );
}
