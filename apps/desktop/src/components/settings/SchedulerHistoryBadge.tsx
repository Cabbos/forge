export function SchedulerHistoryBadge({ status }: { status: string }) {
  const style =
    status === "queued"
      ? { bg: "rgba(var(--forge-amber-rgb), 0.16)", fg: "var(--forge-text-primary)" }
      : status === "completed"
      ? { bg: "var(--forge-active)", fg: "var(--forge-text-primary)" }
      : status === "skipped"
        ? { bg: "rgba(var(--forge-amber-rgb), 0.15)", fg: "var(--forge-text-muted)" }
        : { bg: "rgba(var(--forge-danger-rgb), 0.12)", fg: "var(--forge-danger)" };

  const label =
    status === "queued"
      ? "已排队"
      : status === "completed"
        ? "完成"
        : status === "skipped"
          ? "跳过"
          : "错误";

  return (
    <span
      className="forge-scheduler-status-badge"
      style={{ background: style.bg, color: style.fg }}
    >
      {label}
    </span>
  );
}
