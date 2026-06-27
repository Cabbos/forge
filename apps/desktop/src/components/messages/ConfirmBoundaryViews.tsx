import type { ReactNode } from "react";
import type { WriteBoundaryViewModel } from "@/lib/write-boundary";
import {
  boundaryCommandLabel,
  confirmRiskColor,
} from "@/components/messages/confirmPresentation";

export function ConfirmResolvedSummary({ boundary }: { boundary: WriteBoundaryViewModel }) {
  const firstFile = boundary.affectedFiles[0];

  return (
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
  );
}

export function ConfirmBoundaryGrid({ boundary }: { boundary: WriteBoundaryViewModel }) {
  const riskColor = confirmRiskColor(boundary.riskTone);

  return (
    <dl data-testid="confirm-boundary-grid" className="forge-confirm-boundary-grid">
      <BoundaryLine label={boundary.targetLabel}>
        <span>{boundary.workspaceLabel}</span>
        {boundary.workspacePath !== boundary.workspaceLabel ? (
          <code className="forge-confirm-file-chip">
            {boundary.workspacePath}
          </code>
        ) : null}
      </BoundaryLine>
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
