import type { StreamEvent } from "../lib/protocol";
import type { RuntimeRecoveryNotice } from "./types";

export function upsertRecoveryNotice(
  notices: RuntimeRecoveryNotice[],
  event: Extract<StreamEvent, { event_type: "recovery_notice" }>,
): RuntimeRecoveryNotice[] {
  const nextNotice: RuntimeRecoveryNotice = {
    notice_id: event.notice_id,
    session_id: event.session_id,
    title: event.title,
    message: event.message,
    reason: event.reason,
    recoverable: event.recoverable,
  };
  const existingIdx = notices.findIndex((notice) => notice.notice_id === event.notice_id);
  if (existingIdx < 0) return [...notices, nextNotice];
  return notices.map((notice, index) => index === existingIdx ? nextNotice : notice);
}
