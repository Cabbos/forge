import type { Task } from "./task";

const PRIORITY_WEIGHT: Record<Task["priority"], number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
};

export function sortTasks(tasks: readonly Task[]): Task[] {
  return [...tasks].sort(
    (left, right) =>
      PRIORITY_WEIGHT[left.priority] - PRIORITY_WEIGHT[right.priority] ||
      left.title.localeCompare(right.title) ||
      right.id.localeCompare(left.id),
  );
}
