export type BackgroundTaskStatus = {
  id: string;
  state: "queued" | "running" | "failed" | "completed";
  updatedAt: string;
};

export function orderBackgroundTasks(tasks: BackgroundTaskStatus[]): BackgroundTaskStatus[] {
  return tasks;
}
