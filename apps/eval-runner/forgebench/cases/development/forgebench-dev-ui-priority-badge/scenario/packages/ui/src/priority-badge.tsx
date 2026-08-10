import type { TaskPriority } from "../../domain/src/task";

export interface PriorityBadgeView {
  ariaLabel: string;
  className: string;
  text: string;
}

export function priorityBadge(priority: TaskPriority): PriorityBadgeView {
  return {
    ariaLabel: "Task priority",
    className: "priority-badge",
    text: priority,
  };
}
