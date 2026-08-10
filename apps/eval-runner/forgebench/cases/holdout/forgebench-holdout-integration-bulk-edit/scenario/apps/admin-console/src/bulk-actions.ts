import {
  canTransition,
  type Task,
  type TaskStatus,
} from "../../../packages/domain/src";
import { canBulkEdit, type AdminRole } from "./permissions";

export function bulkTransition(
  tasks: readonly Task[],
  target: TaskStatus,
  role: AdminRole = "admin",
): Task[] {
  if (!canBulkEdit(role)) {
    throw new Error("bulk edit is not authorized");
  }
  if (!tasks.every((task) => canTransition(task.status, target))) {
    throw new Error("bulk transition is invalid");
  }
  return tasks.map((task) => ({ ...task, status: target, revision: task.revision + 1 }));
}

export function buildBulkUpdateRequest(
  tasks: readonly Task[],
  target: TaskStatus,
  role: AdminRole,
) {
  return {
    role,
    updates: tasks.map((task) => ({
      task_id: task.id,
      changes: { status: target },
    })),
  };
}

export function pageForQuery(previousQuery: string, nextQuery: string, page: number): number {
  return previousQuery === nextQuery ? page : 1;
}
