import type { RuntimeHealthAlert } from "./types";

/**
 * Upsert a health alert into the list — replaces by alert_id if it already exists,
 * otherwise appends. Deduplication by alert_id prevents repeated watchdog alerts
 * from stacking up.
 */
export function upsertHealthAlert(
  alerts: RuntimeHealthAlert[],
  alert: RuntimeHealthAlert,
): RuntimeHealthAlert[] {
  const existingIdx = alerts.findIndex((a) => a.alert_id === alert.alert_id);
  if (existingIdx < 0) return [...alerts, alert];
  return alerts.map((a, i) => (i === existingIdx ? alert : a));
}
