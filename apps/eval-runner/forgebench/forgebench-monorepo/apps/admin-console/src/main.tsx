import { parseFeatureFlag } from "../../../packages/config/src";
import type { Task, TaskStatus } from "../../../packages/domain/src";
import { priorityBadge } from "../../../packages/ui/src";
import { bulkTransition } from "./bulk-actions";
import type { AdminRole } from "./permissions";

export function renderAdminConsole(tasks: readonly Task[]) {
  return {
    enabled: parseFeatureFlag(undefined, true),
    rows: tasks.map((task) => ({ id: task.id, badge: priorityBadge(task.priority) })),
  };
}

export function applyAdminBulkAction(
  tasks: readonly Task[],
  target: TaskStatus,
  role: AdminRole,
) {
  return bulkTransition(tasks, target, role);
}
