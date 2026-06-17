import assert from "node:assert/strict";
import test from "node:test";

import { orderBackgroundTasks, type BackgroundTaskStatus } from "./backgroundTasks.ts";

test("running and queued tasks appear before terminal tasks", () => {
  const tasks: BackgroundTaskStatus[] = [
    { id: "done", state: "completed", updatedAt: "2026-06-01T10:00:00Z" },
    { id: "run", state: "running", updatedAt: "2026-06-01T09:00:00Z" },
    { id: "queue", state: "queued", updatedAt: "2026-06-01T08:00:00Z" }
  ];

  assert.deepEqual(orderBackgroundTasks(tasks).map((task) => task.id), ["run", "queue", "done"]);
});

test("ordering does not mutate input array", () => {
  const tasks: BackgroundTaskStatus[] = [
    { id: "a", state: "completed", updatedAt: "2026-06-01T10:00:00Z" },
    { id: "b", state: "running", updatedAt: "2026-06-01T11:00:00Z" }
  ];

  orderBackgroundTasks(tasks);

  assert.deepEqual(tasks.map((task) => task.id), ["a", "b"]);
});
