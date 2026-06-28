import type { StreamEvent } from "../lib/protocol";
import type { LoopRuntimeFactSource } from "../lib/loopRuntime";
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
  const latestUsageEvent = retainedUsageEvent(event.event, previous);
  const latestFileIoEvent = retainedFileIoEvent(event.event, previous);
  next.set(runtimeTaskKey(event.session_id, event.task_id), {
    session_id: event.session_id,
    loop_task_id: event.loop_task_id ?? previous?.loop_task_id ?? null,
    task_id: event.task_id,
    latest_event: event.event,
    ...(latestUsageEvent ? { latest_usage_event: latestUsageEvent } : {}),
    ...(latestFileIoEvent ? { latest_file_io_event: latestFileIoEvent } : {}),
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

export function runtimeFactSourcesForSubagentTasks({
  entries,
  taskIds,
  sessionId,
}: {
  entries: SubagentRuntimeByTask;
  taskIds: Set<string>;
  sessionId?: string | null;
}): LoopRuntimeFactSource[] {
  return [...entries.values()]
    .filter((entry) => taskIds.has(entry.task_id))
    .filter((entry) => !sessionId || entry.session_id === sessionId)
    .flatMap(runtimeFactSourcesForEntry);
}

function runtimeFactSourcesForEntry(entry: SubagentRuntimeEntry): LoopRuntimeFactSource[] {
  const sources: LoopRuntimeFactSource[] = [{
    loop_task_id: entry.loop_task_id ?? null,
    task_id: entry.task_id,
    latest_event: entry.latest_event,
  }];
  if (entry.latest_usage_event && entry.latest_usage_event !== entry.latest_event) {
    sources.push({
      loop_task_id: entry.loop_task_id ?? null,
      task_id: entry.task_id,
      latest_event: entry.latest_usage_event,
    });
  }
  if (entry.latest_file_io_event && entry.latest_file_io_event !== entry.latest_event) {
    sources.push({
      loop_task_id: entry.loop_task_id ?? null,
      task_id: entry.task_id,
      latest_event: entry.latest_file_io_event,
    });
  }
  return sources;
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

function retainedUsageEvent(
  event: SubagentRuntimeEvent["event"],
  previous?: SubagentRuntimeEntry,
): SubagentRuntimeEntry["latest_usage_event"] {
  return event.type === "usage_recorded" ? event : previous?.latest_usage_event;
}

function retainedFileIoEvent(
  event: SubagentRuntimeEvent["event"],
  previous?: SubagentRuntimeEntry,
): SubagentRuntimeEntry["latest_file_io_event"] {
  return event.type === "file_io" ? event : previous?.latest_file_io_event;
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
