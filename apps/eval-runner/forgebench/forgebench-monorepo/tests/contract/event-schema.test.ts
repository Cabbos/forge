import { expect, test } from "bun:test";

interface TaskUpdatedEvent {
  event_id: string;
  type: "task.updated";
  payload: {
    task_id: string;
    revision: number;
    actor_id?: string;
  };
}

test("v2 optional fields preserve the v1 event shape", () => {
  const event: TaskUpdatedEvent = {
    event_id: "event-1",
    type: "task.updated",
    payload: { task_id: "task-1", revision: 2, actor_id: "user-1" },
  };
  const { actor_id: _, ...v1Payload } = event.payload;
  expect(v1Payload).toEqual({ task_id: "task-1", revision: 2 });
});
