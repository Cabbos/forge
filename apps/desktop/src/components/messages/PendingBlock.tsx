export function PendingBlock() {
  return (
    <div
      data-testid="pending-block"
      data-state="running"
      role="status"
      aria-live="polite"
      className="forge-status-row"
    >
      <span data-testid="pending-dots" className="forge-status-dots">
        <span className="forge-status-dot" />
        <span className="forge-status-dot" style={{ animationDelay: "0.18s" }} />
        <span className="forge-status-dot" style={{ animationDelay: "0.36s" }} />
      </span>
      <span>正在组织回答</span>
    </div>
  );
}
