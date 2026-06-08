import { ProcessStatusDots } from "./ProcessStatusDots";

export function PendingBlock() {
  return (
    <div
      data-testid="pending-block"
      data-state="running"
      role="status"
      aria-live="polite"
      className="forge-status-row"
    >
      <ProcessStatusDots testId="pending-dots" />
      <span>正在组织回答</span>
    </div>
  );
}
