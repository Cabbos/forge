import type { StreamEvent } from "../lib/protocol";
import type {
  LoopRuntimeByTask,
  LoopRuntimeEntry,
  SubagentRuntimeByTask,
  SubagentRuntimeEntry,
} from "./types";

type SubagentRuntimeEvent = Extract<StreamEvent, { event_type: "subagent_runtime_event" }>;
type LoopRuntimeUpdatedEvent = Extract<StreamEvent, { event_type: "loop_runtime_updated" }>;

export function runtimeTaskKey(sessionId: string, taskId: string): string {
  return `${sessionId}:${taskId}`;
}

export function applySubagentRuntimeEvent(
  current: SubagentRuntimeByTask,
  event: SubagentRuntimeEvent,
): SubagentRuntimeByTask {
  const next = new Map(current);
  const previous = next.get(runtimeTaskKey(event.session_id, event.task_id));
  next.set(runtimeTaskKey(event.session_id, event.task_id), {
    session_id: event.session_id,
    loop_task_id: event.loop_task_id ?? previous?.loop_task_id ?? null,
    task_id: event.task_id,
    latest_event: event.event,
    ...entryFieldsForPayload(event.event, previous),
  });
  return next;
}

export function applyLoopRuntimeUpdate(
  current: LoopRuntimeByTask,
  event: LoopRuntimeUpdatedEvent,
): LoopRuntimeByTask {
  const next = new Map(current);
  const entry: LoopRuntimeEntry = {
    session_id: event.session_id,
    loop_task_id: event.loop_task_id,
    task: event.task,
  };
  next.set(runtimeTaskKey(event.session_id, event.loop_task_id), entry);
  return next;
}

function entryFieldsForPayload(
  event: SubagentRuntimeEvent["event"],
  previous?: SubagentRuntimeEntry,
): Pick<
  SubagentRuntimeEntry,
  "status" | "role" | "message" | "reason"
> {
  switch (event.type) {
    case "started":
      return runtimeFields({ status: "started", role: event.role });
    case "status":
      return runtimeFields({
        status: event.status,
        role: previous?.role,
        message: event.message ?? previous?.message,
        reason: previous?.reason,
      });
    case "ended":
      return runtimeFields({
        status: event.status,
        role: previous?.role,
        message: previous?.message,
        reason: previous?.reason,
      });
    case "failed":
      return runtimeFields({
        status: "failed",
        role: previous?.role,
        message: previous?.message,
        reason: event.reason,
      });
    case "interrupted":
      return runtimeFields({
        status: "interrupted",
        role: previous?.role,
        message: previous?.message,
        reason: event.reason,
      });
    case "file_io":
      return runtimeFields({
        status: previous?.status ?? "file_io",
        role: previous?.role,
        message: previous?.message,
        reason: previous?.reason,
      });
    case "usage_recorded":
      return runtimeFields({
        status: previous?.status ?? "usage_recorded",
        role: previous?.role,
        message: previous?.message,
        reason: previous?.reason,
      });
  }
}

function runtimeFields(fields: {
  status: string;
  role?: string | null;
  message?: string | null;
  reason?: string | null;
}): Pick<SubagentRuntimeEntry, "status" | "role" | "message" | "reason"> {
  const entry: Pick<SubagentRuntimeEntry, "status" | "role" | "message" | "reason"> = {
    status: fields.status,
  };
  if (fields.role != null) entry.role = fields.role;
  if (fields.message != null) entry.message = fields.message;
  if (fields.reason != null) entry.reason = fields.reason;
  return entry;
}
