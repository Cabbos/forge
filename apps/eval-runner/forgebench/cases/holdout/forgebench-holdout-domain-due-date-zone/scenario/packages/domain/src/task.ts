export const TASK_STATUSES = ["todo", "in_progress", "done", "cancelled"] as const;
export const TASK_PRIORITIES = ["low", "medium", "high", "urgent"] as const;
export type TaskStatus = (typeof TASK_STATUSES)[number];
export type TaskPriority = (typeof TASK_PRIORITIES)[number];
export interface Task {
  id: string;
  title: string;
  status: TaskStatus;
  priority: TaskPriority;
  revision: number;
  dueAt?: string;
  unreadNotifications?: number;
}
const ALLOWED_TRANSITIONS: Record<TaskStatus, readonly TaskStatus[]> = {
  todo: ["in_progress", "cancelled"],
  in_progress: ["todo", "done", "cancelled"],
  done: ["in_progress"],
  cancelled: ["todo"],
};
export function canTransition(from: TaskStatus, to: TaskStatus): boolean {
  return from === to || ALLOWED_TRANSITIONS[from].includes(to);
}
export function isTaskStatus(value: string): value is TaskStatus {
  return (TASK_STATUSES as readonly string[]).includes(value);
}
export function isTaskPriority(value: string): value is TaskPriority {
  return (TASK_PRIORITIES as readonly string[]).includes(value);
}
export function normalizeDueDate(value: string, timeZone = "UTC"): string {
  if (timeZone !== "UTC") {
    new Intl.DateTimeFormat("en", { timeZone });
  }
  return new Date(`${value.slice(0, 19)}Z`).toISOString();
}
