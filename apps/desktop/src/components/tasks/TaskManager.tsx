import { Activity, Bell, CalendarClock, ShieldAlert } from "lucide-react";
import type { BackgroundTaskListItem } from "@/lib/backgroundTaskStatus";

function iconFor(kind: BackgroundTaskListItem["kind"]) {
  if (kind === "scheduler") return CalendarClock;
  if (kind === "alert") return Bell;
  if (kind === "review") return ShieldAlert;
  return Activity;
}

function labelFor(kind: BackgroundTaskListItem["kind"]) {
  if (kind === "scheduler") return "调度";
  if (kind === "alert") return "告警";
  if (kind === "review") return "审阅";
  return "子任务";
}

export function TaskManager({ tasks }: { tasks: BackgroundTaskListItem[] }) {
  if (tasks.length === 0) return null;

  return (
    <section className="forge-background-task-list" data-testid="background-task-list" aria-label="后台任务列表">
      <div className="forge-background-task-list-header">
        <span className="forge-background-task-list-title">后台任务列表</span>
        <span className="forge-background-task-list-count">{tasks.length} 项</span>
      </div>
      <ul className="forge-background-task-list-items">
        {tasks.map((task) => {
          const Icon = iconFor(task.kind);
          return (
            <li key={task.id} className="forge-background-task-item" data-tone={task.tone}>
              <Icon className="size-3.5" />
              <span className="forge-background-task-kind">{labelFor(task.kind)}</span>
              <span className="forge-background-task-title">{task.title}</span>
              <span className="forge-background-task-detail">{task.detail}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
