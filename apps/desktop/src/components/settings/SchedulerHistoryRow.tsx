import { type RunHistoryEntry } from "@/lib/tauri";
import { formatTimestamp } from "./settingsUtils";
import { SchedulerHistoryBadge } from "./SchedulerHistoryBadge";

export function SchedulerHistoryRow({ entry }: { entry: RunHistoryEntry }) {
  return (
    <li className="forge-scheduler-history-item">
      <SchedulerHistoryBadge status={entry.status} />
      <span className="forge-scheduler-history-time">
        {formatTimestamp(entry.started_at_ms)}
      </span>
      <span className="forge-scheduler-history-msg">{entry.message}</span>
    </li>
  );
}
