import type { AgentA2AProjection } from "./protocol.ts";
import type { SchedulerListPayload } from "./tauri.ts";
import type { RuntimeHealthAlert } from "../store/types.ts";
import { normalizeA2ATaskProjection } from "./workbenchSummary.ts";

export interface BackgroundTaskStatusItem {
  key: "agents-running" | "review-needed" | "scheduler" | "health-alerts";
  label: string;
  tone: "running" | "review" | "scheduler" | "alert";
}

export interface BackgroundTaskListItem {
  id: string;
  kind: "agent" | "review" | "scheduler" | "alert";
  title: string;
  detail: string;
  tone: BackgroundTaskStatusItem["tone"];
}

export interface BackgroundTaskStatusView {
  visible: boolean;
  hasAgentWork: boolean;
  items: BackgroundTaskStatusItem[];
  tasks: BackgroundTaskListItem[];
}

export function deriveBackgroundTaskStatus({
  agentA2A,
  scheduler,
  healthAlerts,
}: {
  agentA2A: AgentA2AProjection | null;
  scheduler: SchedulerListPayload | null | undefined;
  healthAlerts: RuntimeHealthAlert[];
}): BackgroundTaskStatusView {
  const runningAgents = agentA2A?.running_count ?? 0;
  const reviewNeeded = (agentA2A?.tasks ?? []).filter(
    (task) => normalizeA2ATaskProjection(task).needs_human_review === true,
  ).length;
  const enabledScheduledTasks = (scheduler?.tasks ?? []).filter((task) => task.enabled).length;
  const alertCount = healthAlerts.length;

  const items: BackgroundTaskStatusItem[] = [];
  if (runningAgents > 0) {
    items.push({
      key: "agents-running",
      label: `${runningAgents} 子任务运行`,
      tone: "running",
    });
  }
  if (reviewNeeded > 0) {
    items.push({
      key: "review-needed",
      label: `${reviewNeeded} 待审阅`,
      tone: "review",
    });
  }
  if (enabledScheduledTasks > 0) {
    items.push({
      key: "scheduler",
      label: `${enabledScheduledTasks} 调度任务`,
      tone: "scheduler",
    });
  }
  if (alertCount > 0) {
    items.push({
      key: "health-alerts",
      label: `${alertCount} 告警`,
      tone: "alert",
    });
  }

  const tasks: BackgroundTaskListItem[] = [];
  for (const rawTask of agentA2A?.tasks ?? []) {
    const task = normalizeA2ATaskProjection(rawTask);
    if (task.status === "running") {
      tasks.push({
        id: `agent:${task.task_id}`,
        kind: "agent",
        title: task.title,
        detail: task.latest_progress ?? task.latest_message ?? "子任务运行中",
        tone: "running",
      });
    }
    if (task.needs_human_review === true) {
      tasks.push({
        id: `review:${task.task_id}`,
        kind: "review",
        title: task.title,
        detail: task.suggested_action ?? "等待人工审阅",
        tone: "review",
      });
    }
  }
  for (const task of scheduler?.tasks ?? []) {
    if (!task.enabled) continue;
    tasks.push({
      id: `scheduler:${task.id}`,
      kind: "scheduler",
      title: task.title,
      detail: task.interval_seconds === 0 ? "手动触发" : `${task.interval_seconds}s 间隔`,
      tone: "scheduler",
    });
  }
  for (const alert of healthAlerts) {
    tasks.push({
      id: `alert:${alert.alert_id}`,
      kind: "alert",
      title: alert.title,
      detail: alert.message,
      tone: "alert",
    });
  }

  return {
    visible: items.length > 0,
    hasAgentWork: runningAgents > 0 || reviewNeeded > 0,
    items,
    tasks,
  };
}
