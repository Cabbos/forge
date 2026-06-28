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

export function clearStaleSessionHealthAlerts(
  alerts: RuntimeHealthAlert[],
  sessionId: string,
): RuntimeHealthAlert[] {
  const nextAlerts = alerts.filter((alert) => {
    const isSameSession = alert.session_id === sessionId;
    const isStaleSessionAlert = alert.alert_id.startsWith("session-stale");
    return !(isSameSession && isStaleSessionAlert);
  });
  return nextAlerts.length === alerts.length ? alerts : nextAlerts;
}

export function visibleHealthAlertsForSession(
  alerts: RuntimeHealthAlert[],
  activeSessionId: string | null,
): RuntimeHealthAlert[] {
  if (!activeSessionId) return alerts;
  return alerts.filter((alert) => {
    if (!alert.alert_id.startsWith("session-stale")) return true;
    return alert.session_id === activeSessionId;
  });
}
