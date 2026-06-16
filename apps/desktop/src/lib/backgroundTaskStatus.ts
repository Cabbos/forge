import type { AgentA2AProjection } from "./protocol.ts";
import type { SchedulerListPayload } from "./tauri.ts";
import type { RuntimeHealthAlert } from "../store/types.ts";
import { normalizeA2ATaskProjection } from "./workbenchSummary.ts";

export interface BackgroundTaskStatusItem {
  key: "agents-running" | "review-needed" | "scheduler" | "health-alerts";
  label: string;
  tone: "running" | "review" | "scheduler" | "alert";
}

export interface BackgroundTaskStatusView {
  visible: boolean;
  hasAgentWork: boolean;
  items: BackgroundTaskStatusItem[];
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

  return {
    visible: items.length > 0,
    hasAgentWork: runningAgents > 0 || reviewNeeded > 0,
    items,
  };
}
