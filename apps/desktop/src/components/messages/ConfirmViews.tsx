import type { ReactNode } from "react";
import { Check, ShieldAlert, X } from "lucide-react";
import type { WriteBoundaryViewModel } from "@/lib/write-boundary";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import {
  boundaryCommandLabel,
  confirmIconTone,
  confirmResolvedLabel,
  confirmRiskColor,
  type ConfirmPromptViewModel,
} from "@/components/messages/confirmPresentation";
import { ForgeIcon } from "@/components/primitives/icon";

export function ConfirmActionBar({
  responded,
  answer,
  onResponse,
}: {
  responded: boolean;
  answer: boolean | null;
  onResponse: (approved: boolean) => void;
}) {
  if (responded) {
    return (
      <div data-testid="confirm-action-bar" className="forge-confirm-action-bar">
        <span className="forge-confirm-resolved" data-state={answer ? "approved" : "cancelled"}>
          {confirmResolvedLabel(answer)}
        </span>
      </div>
    );
  }

  return (
    <div data-testid="confirm-action-bar" className="forge-confirm-action-bar">
      <button
        data-testid="confirm-approve"
        onClick={(e) => { e.stopPropagation(); onResponse(true); }}
        className="forge-confirm-button"
        data-variant="approve"
      >
        <Check className="size-3.5" />
        继续
      </button>
      <button
        data-testid="confirm-cancel"
        onClick={(e) => { e.stopPropagation(); onResponse(false); }}
        className="forge-confirm-button"
        data-variant="cancel"
      >
        <X className="size-3.5" />
        取消
      </button>
    </div>
  );
}

export function ConfirmBoundaryResolvedView({
  boundary,
  answer,
}: {
  boundary: WriteBoundaryViewModel;
  answer: boolean | null;
}) {
  const firstFile = boundary.affectedFiles[0];
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

      <div data-testid="confirm-resolved-summary" className="forge-confirm-resolved-summary">
        <span className="forge-confirm-resolved-summary-item">{boundary.workspaceLabel}</span>
        <span className="forge-confirm-resolved-summary-item">{boundary.operationLabel}</span>
        <span className="forge-confirm-resolved-summary-item">{boundary.affectedSummary}</span>
        {firstFile ? (
          <code className="forge-confirm-file-chip">
            {firstFile}
          </code>
        ) : null}
      </div>
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
  const riskColor = confirmRiskColor(boundary.riskTone);
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

      <dl data-testid="confirm-boundary-grid" className="forge-confirm-boundary-grid">
        <BoundaryLine label={boundary.targetLabel}>{boundary.workspaceLabel}</BoundaryLine>
        <BoundaryLine label="操作">{boundary.operationLabel}</BoundaryLine>
        <BoundaryLine label="影响范围">
          <span>{boundary.affectedSummary}</span>
          {boundary.affectedFiles.length > 0 ? (
            <div className="mt-1 flex flex-wrap gap-1.5">
              {boundary.affectedFiles.slice(0, 4).map((file) => (
                <code
                  key={file}
                  className="forge-confirm-file-chip"
                >
                  {file}
                </code>
              ))}
            </div>
          ) : null}
        </BoundaryLine>
        <BoundaryLine label="风险">
          <span className="forge-confirm-risk" style={{ color: riskColor }}>{boundary.riskLabel}</span>
        </BoundaryLine>
        <BoundaryLine label="恢复点">{boundary.recoveryLabel}</BoundaryLine>
        {boundary.command ? (
          <BoundaryLine label={boundaryCommandLabel(boundary.operationLabel)}>
            <code className="forge-confirm-command">
              {boundary.command}
            </code>
          </BoundaryLine>
        ) : null}
        {boundary.warning ? (
          <div data-testid="confirm-warning" role="note" className="forge-confirm-warning">
            {boundary.warning}
          </div>
        ) : null}
      </dl>

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

function BoundaryLine({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div data-testid="confirm-boundary-row" className="forge-confirm-boundary-row">
      <dt className="forge-confirm-boundary-label">{label}</dt>
      <dd className="forge-confirm-boundary-value">{children}</dd>
    </div>
  );
}
