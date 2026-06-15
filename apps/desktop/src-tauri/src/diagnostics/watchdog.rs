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

/// How often the gateway service watchdog probes launchd.
const GATEWAY_WATCHDOG_CHECK_INTERVAL_SECS: u64 = 30;

/// Initial restart delay after a failed gateway repair attempt.
const GATEWAY_RESTART_BACKOFF_BASE_SECS: u64 = 5;

/// Maximum restart delay after repeated gateway repair failures.
const GATEWAY_RESTART_BACKOFF_MAX_SECS: u64 = 300;

/// Synthetic session id used for global runtime health alerts.
const RUNTIME_HEALTH_SESSION_ID: &str = "__runtime__";

// ── Gateway service watchdog ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayServiceProbe {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub message: String,
}

impl From<crate::service::launchd::LaunchdServiceStatus> for GatewayServiceProbe {
    fn from(status: crate::service::launchd::LaunchdServiceStatus) -> Self {
        Self {
            supported: status.supported,
            installed: status.installed,
            running: status.running,
            message: status.message,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayWatchdogAction {
    Observe,
    RestartGateway,
    WaitForBackoff,
    AlertOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayWatchdogDecision {
    pub action: GatewayWatchdogAction,
    pub alert_level: Option<String>,
    pub message: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayWatchdogState {
    restart_failures: u32,
    last_restart_attempt: Option<Instant>,
}

impl GatewayWatchdogState {
    pub fn record_restart_failure(&mut self, now: Instant) {
        self.restart_failures = self.restart_failures.saturating_add(1);
        self.last_restart_attempt = Some(now);
    }

    fn record_restart_success(&mut self) {
        self.restart_failures = 0;
        self.last_restart_attempt = None;
    }

    fn restart_backoff_remaining_secs(&self, now: Instant) -> Option<u64> {
        let last_attempt = self.last_restart_attempt?;
        let elapsed = now.duration_since(last_attempt).as_secs();
        let delay = gateway_restart_backoff_secs(self.restart_failures);
        Some(delay.saturating_sub(elapsed))
    }
}

fn gateway_restart_backoff_secs(failure_count: u32) -> u64 {
    let shift = failure_count.min(16);
    let multiplier = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
    GATEWAY_RESTART_BACKOFF_BASE_SECS
        .saturating_mul(multiplier)
        .min(GATEWAY_RESTART_BACKOFF_MAX_SECS)
}

fn evaluate_gateway_watchdog(
    probe: GatewayServiceProbe,
    state: &GatewayWatchdogState,
    now: Instant,
) -> GatewayWatchdogDecision {
    if !probe.supported {
        return GatewayWatchdogDecision {
            action: GatewayWatchdogAction::Observe,
            alert_level: None,
            message: probe.message,
            remediation: None,
        };
    }

    if probe.running {
        return GatewayWatchdogDecision {
            action: GatewayWatchdogAction::Observe,
            alert_level: None,
            message: probe.message,
            remediation: None,
        };
    }

    if !probe.installed {
        return GatewayWatchdogDecision {
            action: GatewayWatchdogAction::AlertOnly,
            alert_level: Some("warn".to_string()),
            message: "Gateway service is not installed.".to_string(),
            remediation: Some(
                "Enable autostart in Settings -> General or run reinstall_service.".to_string(),
            ),
        };
    }

    if let Some(remaining) = state.restart_backoff_remaining_secs(now) {
        if remaining > 0 {
            return GatewayWatchdogDecision {
                action: GatewayWatchdogAction::WaitForBackoff,
                alert_level: None,
                message: format!(
                    "Gateway service restart is waiting for backoff ({remaining}s remaining)."
                ),
                remediation: None,
            };
        }
    }

    GatewayWatchdogDecision {
        action: GatewayWatchdogAction::RestartGateway,
        alert_level: Some("warn".to_string()),
        message: "Gateway service is installed but is not running.".to_string(),
        remediation: Some(
            "Forge will try to restart the Gateway service automatically.".to_string(),
        ),
    }
}

fn probe_gateway_service() -> GatewayServiceProbe {
    match crate::service::launchd::query_status() {
        Ok(status) => GatewayServiceProbe::from(status),
        Err(error) => GatewayServiceProbe {
            supported: cfg!(target_os = "macos"),
            installed: crate::service::launchd::plist_path().exists(),
            running: false,
            message: format!("Gateway service status unavailable: {error}"),
        },
    }
}

fn gateway_watchdog_alert(
    level: impl Into<String>,
    message: impl Into<String>,
    remediation: Option<String>,
) -> StreamEvent {
    StreamEvent::HealthAlert {
        session_id: RUNTIME_HEALTH_SESSION_ID.to_string(),
        alert_id: "gateway-service-watchdog".to_string(),
        level: level.into(),
        title: "Gateway service watchdog".to_string(),
        message: message.into(),
        remediation,
    }
}

/// Spawn the gateway service watchdog background task.
///
/// The task is conservative: it only attempts automatic restart when the
/// platform supports service management, the launchd plist is installed, and
/// the service is not running. Failed restart attempts use exponential backoff.
pub fn spawn_gateway_watchdog(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut state = GatewayWatchdogState::default();

        loop {
            tokio::time::sleep(Duration::from_secs(GATEWAY_WATCHDOG_CHECK_INTERVAL_SECS)).await;

            let now = Instant::now();
            let decision = evaluate_gateway_watchdog(probe_gateway_service(), &state, now);

            match decision.action {
                GatewayWatchdogAction::Observe => {
                    state.record_restart_success();
                }
                GatewayWatchdogAction::AlertOnly => {
                    if let Some(level) = decision.alert_level {
                        crate::transcript::emit_stream_event(
                            &app_handle,
                            gateway_watchdog_alert(level, decision.message, decision.remediation),
                        );
                    }
                }
                GatewayWatchdogAction::WaitForBackoff => {}
                GatewayWatchdogAction::RestartGateway => {
                    let result = crate::diagnostics::repair::run_repair("restart_gateway");
                    if result.success {
                        state.record_restart_success();
                        crate::transcript::emit_stream_event(
                            &app_handle,
                            gateway_watchdog_alert(
                                "info",
                                "Gateway service was restarted automatically.",
                                Some(result.message),
                            ),
                        );
                    } else {
                        state.record_restart_failure(now);
                        crate::transcript::emit_stream_event(
                            &app_handle,
                            gateway_watchdog_alert(
                                decision.alert_level.unwrap_or_else(|| "warn".to_string()),
                                format!(
                                    "{} Automatic restart failed: {}",
                                    decision.message, result.message
                                ),
                                decision.remediation,
                            ),
                        );
                    }
                }
            }
        }
    });
}

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

    #[test]
    fn gateway_watchdog_restarts_installed_stopped_service() {
        let now = Instant::now();
        let state = GatewayWatchdogState::default();
        let probe = GatewayServiceProbe {
            supported: true,
            installed: true,
            running: false,
            message: "Gateway service is installed but not running.".into(),
        };

        let decision = evaluate_gateway_watchdog(probe, &state, now);

        assert_eq!(decision.action, GatewayWatchdogAction::RestartGateway);
        assert_eq!(decision.alert_level.as_deref(), Some("warn"));
        assert!(decision.message.contains("installed but is not running"));
    }

    #[test]
    fn gateway_restart_backoff_is_exponential_and_capped() {
        assert_eq!(gateway_restart_backoff_secs(0), 5);
        assert_eq!(gateway_restart_backoff_secs(1), 10);
        assert_eq!(gateway_restart_backoff_secs(2), 20);
        assert_eq!(gateway_restart_backoff_secs(4), 80);
        assert_eq!(gateway_restart_backoff_secs(10), 300);
    }

    #[test]
    fn gateway_watchdog_waits_for_backoff_after_failure() {
        let now = Instant::now();
        let mut state = GatewayWatchdogState::default();
        state.record_restart_failure(now);
        let probe = GatewayServiceProbe {
            supported: true,
            installed: true,
            running: false,
            message: "Gateway service is installed but not running.".into(),
        };

        let decision = evaluate_gateway_watchdog(probe, &state, now + Duration::from_secs(4));

        assert_eq!(decision.action, GatewayWatchdogAction::WaitForBackoff);
        assert_eq!(decision.alert_level, None);
    }

    #[test]
    fn gateway_service_probe_uses_structured_launchd_status() {
        let probe = GatewayServiceProbe::from(crate::service::launchd::LaunchdServiceStatus {
            supported: true,
            installed: true,
            running: false,
            message: "Gateway service is installed but not running.".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "gui/123".into(),
            plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            log_path: "/Users/test/.forge/logs/gateway.log".into(),
            error_log_path: "/Users/test/.forge/logs/gateway-error.log".into(),
            status_message: "Service 'com.forge.gateway' is not installed.".into(),
        });

        assert!(probe.supported);
        assert!(probe.installed);
        assert!(!probe.running);
        assert!(probe.message.contains("installed but not running"));
    }
}
