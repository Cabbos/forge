import { parsePort } from "../../../packages/config/src";
import type { Task } from "../../../packages/domain/src";
import { taskList } from "./task-list";

export function renderWebConsole(tasks: readonly Task[], query = "") {
  return {
    port: parsePort(undefined),
    taskList: taskList(tasks, query),
  };
}
