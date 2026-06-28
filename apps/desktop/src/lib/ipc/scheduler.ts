import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  ScheduledTask,
  SchedulerListPayload,
  UpsertScheduledTaskInput,
} from "./types";

export async function listScheduledTasks(): Promise<SchedulerListPayload> {
  if (!hasTauriRuntime()) {
    return { tasks: [], recent_history: [], load_error: null };
  }
  return invoke<SchedulerListPayload>("list_scheduled_tasks");
}

export async function upsertScheduledTask(input: UpsertScheduledTaskInput): Promise<ScheduledTask> {
  return invoke<ScheduledTask>("upsert_scheduled_task", { input });
}

export async function deleteScheduledTask(id: string): Promise<boolean> {
  return invoke<boolean>("delete_scheduled_task", { id });
}

export async function setScheduledTaskEnabled(
  id: string,
  enabled: boolean,
): Promise<boolean> {
  return invoke<boolean>("set_scheduled_task_enabled", { id, enabled });
}

export async function runScheduledTaskNow(id: string): Promise<ScheduledTask> {
  return invoke<ScheduledTask>("run_scheduled_task_now", { id });
}
