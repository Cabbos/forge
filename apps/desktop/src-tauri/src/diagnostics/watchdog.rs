//! Session watchdog: monitors live sessions and emits HealthAlert events
//! when a session has produced no event for longer than the threshold.
//!
//! Phase 2.4: Session Watchdog — minimal, no-throw, advisory-only.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::agent::session::SessionStatus;
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

// ── Configuration ────────────────────────────────────────────────────────

/// Default threshold: emit a health alert if a session hasn't produced an
/// event in 5 minutes.
const DEFAULT_STALE_THRESHOLD_SECS: u64 = 300;

/// How often the watchdog inspects sessions.
const WATCHDOG_CHECK_INTERVAL_SECS: u64 = 60;

/// Minimum interval between repeated alerts for the same session, to avoid
/// alert spam. The watchdog checks every 60s but won't re-alert for the
/// same session until this cooldown passes.
const ALERT_COOLDOWN_SECS: u64 = 300;

// ── Session event tracker ────────────────────────────────────────────────

/// Tracks the latest event timestamp per session, keyed by session_id.
static SESSION_EVENT_TRACKER: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

/// Record a session event. Called from `emit_stream_event` for every event
/// that carries a session_id. Minimal, no-throw — if the lock is poisoned
/// or the registry isn't initialized, we silently skip.
pub fn record_session_event(event: &StreamEvent) {
    let session_id = event.session_id().to_string();
    if let Some(registry) = SESSION_EVENT_TRACKER.get() {
        if let Ok(mut guard) = registry.lock() {
            guard.insert(session_id, Instant::now());
        }
    }
}

/// Initialize the tracker (called once on startup).
pub fn init_session_event_tracker() {
    SESSION_EVENT_TRACKER.get_or_init(|| Mutex::new(HashMap::new()));
}

// ── Watchdog task ────────────────────────────────────────────────────────

/// Spawn the session watchdog background task.
///
/// The watchdog:
/// 1. Waits for the check interval.
/// 2. Inspects live sessions via AppState.
/// 3. For sessions that are "running" or "resuming", checks the last event
///    timestamp from the tracker.
/// 4. If the session has no recorded event or has exceeded the stale
///    threshold, emits a StreamEvent::HealthAlert via the app handle.
/// 5. Uses a per-session cooldown to avoid repeated alerts.
pub fn spawn_session_watchdog(app_handle: tauri::AppHandle) {
    // Ensure the tracker is initialized before the watchdog starts.
    init_session_event_tracker();

    tauri::async_runtime::spawn(async move {
        // Track when we last alerted per session to avoid spam.
        let mut last_alert: HashMap<String, Instant> = HashMap::new();

        loop {
            tokio::time::sleep(Duration::from_secs(WATCHDOG_CHECK_INTERVAL_SECS)).await;

            let state: Arc<AppState> = match app_handle.try_state::<Arc<AppState>>() {
                Some(s) => s.inner().clone(),
                None => continue,
            };

            let now = Instant::now();
            let sessions = state.sessions.read().await;
            let registry = match SESSION_EVENT_TRACKER.get() {
                Some(r) => r,
                None => continue,
            };

            // Collect session IDs to check
            let live_ids: Vec<String> = sessions
                .iter()
                .filter(|(_id, session)| {
                    let status = session.status.lock();
                    matches!(&*status, SessionStatus::Running | SessionStatus::Resuming)
                })
                .map(|(id, _session)| id.clone())
                .collect();

            for session_id in live_ids {
                // Check cooldown
                let cooldown_passed = last_alert
                    .get(&session_id)
                    .map(|last| now.duration_since(*last).as_secs() >= ALERT_COOLDOWN_SECS)
                    .unwrap_or(true);

                if !cooldown_passed {
                    continue;
                }

                // Check last event timestamp
                let is_stale = match registry.lock() {
                    Ok(guard) => match guard.get(&session_id) {
                        Some(last_event) => {
                            now.duration_since(*last_event).as_secs()
                                >= DEFAULT_STALE_THRESHOLD_SECS
                        }
                        None => true, // no event ever recorded → stale
                    },
                    Err(_) => continue,
                };

                if is_stale {
                    last_alert.insert(session_id.clone(), now);

                    let alert = StreamEvent::HealthAlert {
                        session_id: session_id.clone(),
                        alert_id: format!("session-stale-{session_id}"),
                        level: "warn".to_string(),
                        title: "会话无响应".to_string(),
                        message: format!(
                            "会话 {} 在过去 {} 分钟内没有产生新事件。",
                            &session_id[..session_id.len().min(12)],
                            DEFAULT_STALE_THRESHOLD_SECS / 60
                        ),
                        remediation: Some(
                            "请检查会话状态。如需恢复，可以尝试重启会话。".to_string(),
                        ),
                    };

                    crate::transcript::emit_stream_event(&app_handle, alert);
                }
            }
        }
    });
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn record_session_event_updates_tracker() {
        init_session_event_tracker();

        let event = StreamEvent::SessionStatus {
            session_id: "test-session".to_string(),
            status: "idle".to_string(),
        };

        record_session_event(&event);

        let registry = SESSION_EVENT_TRACKER.get().unwrap();
        let guard = registry.lock().unwrap();
        assert!(guard.contains_key("test-session"));
    }

    #[test]
    fn record_session_event_multiple_sessions() {
        init_session_event_tracker();

        for id in ["s1", "s2", "s3"] {
            record_session_event(&StreamEvent::SessionStatus {
                session_id: id.to_string(),
                status: "idle".to_string(),
            });
        }

        let guard = SESSION_EVENT_TRACKER.get().unwrap().lock().unwrap();
        // Check that our entries exist (len may be larger due to other
        // tests that ran before us, since the tracker is a static OnceLock).
        assert!(guard.len() >= 3);
        assert!(guard.contains_key("s1"));
        assert!(guard.contains_key("s2"));
        assert!(guard.contains_key("s3"));
    }

    #[test]
    fn stale_detection_threshold_is_reasonable() {
        // The default threshold should be a positive number >= 60 seconds.
        let threshold = watchdog_threshold_for_test(None);
        assert!(threshold >= 60);
        assert_eq!(threshold, 300);
    }

    #[test]
    fn alert_cooldown_is_gte_check_interval() {
        // Alert cooldown should be at least the check interval to avoid
        // re-alerting on every tick.
        let cooldown = watchdog_cooldown_for_test(None);
        let interval = watchdog_interval_for_test(None);
        assert!(cooldown >= interval);
    }

    fn watchdog_threshold_for_test(override_value: Option<u64>) -> u64 {
        override_value.unwrap_or(DEFAULT_STALE_THRESHOLD_SECS)
    }

    fn watchdog_cooldown_for_test(override_value: Option<u64>) -> u64 {
        override_value.unwrap_or(ALERT_COOLDOWN_SECS)
    }

    fn watchdog_interval_for_test(override_value: Option<u64>) -> u64 {
        override_value.unwrap_or(WATCHDOG_CHECK_INTERVAL_SECS)
    }

    #[test]
    fn health_alert_from_stale_session_has_correct_shape() {
        let alert = StreamEvent::HealthAlert {
            session_id: "s".to_string(),
            alert_id: "session-stale-s".to_string(),
            level: "warn".to_string(),
            title: "会话无响应".to_string(),
            message: "会话 s 在过去 5 分钟内没有产生新事件。".to_string(),
            remediation: Some("请检查会话状态。如需恢复，可以尝试重启会话。".to_string()),
        };

        assert_eq!(alert.session_id(), "s");
        assert_eq!(alert.event_type(), "health_alert");

        let json = serde_json::to_value(&alert).unwrap();
        assert_eq!(json["event_type"], "health_alert");
        assert_eq!(json["level"], "warn");
        assert!(json["remediation"].as_str().is_some());
    }

    #[test]
    fn cooldown_prevents_immediate_realert() {
        let mut last_alert: HashMap<String, Instant> = HashMap::new();
        let session_id = "test-cooldown".to_string();

        // Record an alert just now
        last_alert.insert(session_id.clone(), Instant::now());

        // Cooldown should NOT have passed
        let now = Instant::now();
        let cooldown_passed = last_alert
            .get(&session_id)
            .map(|last| now.duration_since(*last).as_secs() >= ALERT_COOLDOWN_SECS)
            .unwrap_or(true);

        assert!(!cooldown_passed);
    }

    #[test]
    fn cooldown_allows_alert_after_expiry() {
        let mut last_alert: HashMap<String, Instant> = HashMap::new();
        let session_id = "test-cooldown-expired".to_string();

        // Record an alert far in the past
        last_alert.insert(
            session_id.clone(),
            Instant::now() - Duration::from_secs(ALERT_COOLDOWN_SECS + 1),
        );

        let now = Instant::now();
        let cooldown_passed = last_alert
            .get(&session_id)
            .map(|last| now.duration_since(*last).as_secs() >= ALERT_COOLDOWN_SECS)
            .unwrap_or(true);

        assert!(cooldown_passed);
    }

    #[test]
    fn no_alerts_for_session_with_fresh_event() {
        init_session_event_tracker();

        let session_id = "fresh-session";
        record_session_event(&StreamEvent::SessionStarted {
            session_id: session_id.to_string(),
            agent_type: "deepseek".to_string(),
            model: "deepseek-v4".to_string(),
            context_window_tokens: None,
        });

        let registry = SESSION_EVENT_TRACKER.get().unwrap();
        let guard = registry.lock().unwrap();
        let last_event = guard.get(session_id).unwrap();

        // The recorded event should be very recent
        assert!(last_event.elapsed().as_secs() < 5);
    }
}
