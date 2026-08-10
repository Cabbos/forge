import {
  matchesTaskQuery,
  parseTaskQuery,
  sortTasks,
  type Task,
} from "../../../packages/domain/src";
import { emptyState, priorityBadge } from "../../../packages/ui/src";

export function taskList(tasks: readonly Task[], rawQuery: string) {
  const query = parseTaskQuery(rawQuery);
  const filtered = sortTasks(tasks.filter((task) => matchesTaskQuery(task, query)));
  if (filtered.length === 0) {
    return { empty: emptyState("No tasks", "Try another filter."), rows: [] };
  }
  return {
    empty: null,
    rows: filtered.map((task) => ({
      id: task.id,
      title: task.title,
      badge: priorityBadge(task.priority),
      unread: task.unreadNotifications ?? 0,
    })),
  };
}

export function shouldHandleShortcut(targetTag: string, isContentEditable: boolean): boolean {
  return !isContentEditable && !["INPUT", "TEXTAREA", "SELECT"].includes(targetTag.toUpperCase());
}
