import { canTransition, type Task, type TaskStatus } from "../../../packages/domain/src";
import { canBulkEdit, type AdminRole } from "./permissions";

export function bulkTransition(
  tasks: readonly Task[],
  target: TaskStatus,
  role: AdminRole = "admin",
): Task[] {
  if (!canBulkEdit(role) || !tasks.every((task) => canTransition(task.status, target))) {
    throw new Error("bulk transition is invalid");
  }
  return tasks.map((task) => ({ ...task, status: target, revision: task.revision + 1 }));
}

export function pageForQuery(previousQuery: string, nextQuery: string, page: number): number {
  return 1;
}
