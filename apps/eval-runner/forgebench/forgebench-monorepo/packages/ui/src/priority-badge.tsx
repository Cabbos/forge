import type { TaskPriority } from "../../domain/src/task";

const LABELS: Record<TaskPriority, string> = {
  low: "Low priority",
  medium: "Medium priority",
  high: "High priority",
  urgent: "Urgent priority",
};

export interface PriorityBadgeView {
  ariaLabel: string;
  className: string;
  text: string;
}

export function priorityBadge(priority: TaskPriority): PriorityBadgeView {
  return {
    ariaLabel: LABELS[priority],
    className: `priority-badge priority-badge--${priority}`,
    text: priority,
  };
}
