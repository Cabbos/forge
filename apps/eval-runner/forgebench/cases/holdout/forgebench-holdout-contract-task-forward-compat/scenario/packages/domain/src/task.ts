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
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    throw new Error("invalid due date");
  }
  if (timeZone !== "UTC") {
    new Intl.DateTimeFormat("en", { timeZone }).format(parsed);
  }
  return parsed.toISOString();
}

export function decodeTask(value: unknown): Task {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("task must be an object");
  }
  const record = value as Record<string, unknown>;
  const known = new Set([
    "id",
    "title",
    "status",
    "priority",
    "revision",
    "dueAt",
    "unreadNotifications",
  ]);
  if (Object.keys(record).some((key) => !known.has(key))) {
    throw new Error("unknown task field");
  }
  return record as unknown as Task;
}
