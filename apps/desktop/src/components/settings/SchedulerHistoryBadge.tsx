export function SchedulerHistoryBadge({ status }: { status: string }) {
  const style =
    status === "completed"
      ? { bg: "var(--forge-active)", fg: "var(--forge-text-primary)" }
      : status === "skipped"
        ? { bg: "rgba(184, 138, 86, 0.15)", fg: "var(--forge-text-muted)" }
        : { bg: "rgba(220, 80, 60, 0.12)", fg: "#b33a2e" };

  const label =
    status === "completed" ? "完成" : status === "skipped" ? "跳过" : "错误";

  return (
    <span
      className="forge-scheduler-status-badge"
      style={{ background: style.bg, color: style.fg }}
    >
      {label}
    </span>
  );
}
