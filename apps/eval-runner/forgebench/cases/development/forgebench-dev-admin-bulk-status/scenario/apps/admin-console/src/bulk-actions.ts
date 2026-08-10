import { canTransition, type Task, type TaskStatus } from "../../../packages/domain/src";
import { canBulkEdit, type AdminRole } from "./permissions";

export function bulkTransition(
  tasks: readonly Task[],
  target: TaskStatus,
  role: AdminRole = "admin",
): Task[] {
  if (!canBulkEdit(role)) {
    throw new Error("bulk edit is not authorized");
  }
  return tasks.map((task) => {
    if (!canTransition(task.status, target)) {
      throw new Error("bulk transition is invalid");
    }
    task.status = target;
    task.revision += 1;
    return task;
  });
}

export function pageForQuery(previousQuery: string, nextQuery: string, page: number): number {
  return previousQuery === nextQuery ? page : 1;
}
