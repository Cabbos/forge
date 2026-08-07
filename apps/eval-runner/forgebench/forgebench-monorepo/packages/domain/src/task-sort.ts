import type { Task } from "./task";

const PRIORITY_WEIGHT: Record<Task["priority"], number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
};

export function sortTasks(tasks: readonly Task[]): Task[] {
  return tasks
    .map((task, index) => ({ task, index }))
    .sort(
      (left, right) =>
        PRIORITY_WEIGHT[left.task.priority] - PRIORITY_WEIGHT[right.task.priority] ||
        left.task.title.localeCompare(right.task.title) ||
        left.index - right.index,
    )
    .map(({ task }) => task);
}
