import { Activity, Bell, CalendarClock, ShieldAlert } from "lucide-react";
import type { BackgroundTaskListItem, BackgroundTaskNotificationItem } from "@/lib/backgroundTaskStatus";
import { LoopTaskPanel } from "@/components/loop/LoopTaskPanel";

function iconFor(kind: BackgroundTaskListItem["kind"]) {
  if (kind === "scheduler") return CalendarClock;
  if (kind === "alert") return Bell;
  if (kind === "review") return ShieldAlert;
  return Activity;
}

function labelFor(kind: BackgroundTaskListItem["kind"]) {
  if (kind === "loop") return "Loop";
  if (kind === "scheduler") return "调度";
  if (kind === "alert") return "告警";
  if (kind === "review") return "审阅";
  return "子任务";
}

function notificationLabelFor(kind: BackgroundTaskNotificationItem["kind"]) {
  if (kind === "loop") return "Loop 待处理";
  if (kind === "scheduler") return "调度已启用";
  if (kind === "alert") return "运行告警";
  if (kind === "review") return "需要审阅";
  return "子任务运行";
}

export function TaskManager({
  tasks,
  notifications,
}: {
  tasks: BackgroundTaskListItem[];
  notifications: BackgroundTaskNotificationItem[];
}) {
  if (tasks.length === 0 && notifications.length === 0) return null;

  return (
    <section className="forge-background-task-list" data-testid="background-task-list" aria-label="后台任务列表">
      {notifications.length > 0 && (
        <section
          className="forge-background-notification-list"
          data-testid="background-notification-list"
          aria-label="后台通知"
          aria-live="polite"
        >
          <div className="forge-background-task-list-header">
            <span className="forge-background-task-list-title">通知</span>
            <span className="forge-background-task-list-count">{notifications.length} 条</span>
          </div>
          <ul className="forge-background-task-list-items">
            {notifications.map((notification) => {
              const Icon = iconFor(notification.kind);
              return (
                <li key={notification.id} className="forge-background-notification-item" data-tone={notification.tone}>
                  <Icon className="size-3.5" />
                  <span className="forge-background-task-kind">{notificationLabelFor(notification.kind)}</span>
                  <span className="forge-background-task-title">{notification.title}</span>
                  <span className="forge-background-task-detail">{notification.detail}</span>
                </li>
              );
            })}
          </ul>
        </section>
      )}
      {tasks.length > 0 && (
        <>
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
                  {task.kind === "loop" && task.loopTask && task.loopSummary && (
                    <LoopTaskPanel task={task.loopTask} summary={task.loopSummary} />
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}
    </section>
  );
}
