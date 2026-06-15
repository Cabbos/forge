//! IPC handlers for service management — autostart toggle, status query.

use crate::service::launchd;
use serde::Serialize;

/// Payload returned by `get_service_status`.
#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatusPayload {
    /// Whether the launchd plist is installed.
    pub installed: bool,
    /// Whether the service is currently running (if detectable).
    pub running: bool,
    /// Human-readable status message.
    pub message: String,
    /// Whether the current platform supports launchd/systemd.
    pub supported: bool,
    /// Launch service label.
    pub label: String,
    /// launchd domain used for status checks.
    pub launch_domain: String,
    /// Expected launchd plist path.
    pub plist_path: String,
    /// Gateway stdout log path.
    pub log_path: String,
    /// Gateway stderr log path.
    pub error_log_path: String,
}

impl From<launchd::LaunchdServiceStatus> for ServiceStatusPayload {
    fn from(status: launchd::LaunchdServiceStatus) -> Self {
        Self {
            installed: status.installed,
            running: status.running,
            message: status.message,
            supported: status.supported,
            label: status.label,
            launch_domain: status.launch_domain,
            plist_path: status.plist_path,
            log_path: status.log_path,
            error_log_path: status.error_log_path,
        }
    }
}

/// Query the current service status.
#[tauri::command]
pub async fn get_service_status() -> Result<ServiceStatusPayload, String> {
    launchd::query_status().map(ServiceStatusPayload::from)
}

/// Enable or disable autostart (installs/uninstalls the launchd service).
#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<ServiceStatusPayload, String> {
    let supported = cfg!(target_os = "macos");
    if !supported {
        return Err("Service management is only supported on macOS.".to_string());
    }

    if enabled {
        launchd::install()?;
    } else {
        launchd::uninstall()?;
    }

    get_service_status().await
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_status_payload_serializes() {
        let payload = ServiceStatusPayload {
            installed: true,
            running: true,
            message: "running".into(),
            supported: true,
            label: "com.forge.gateway".into(),
            launch_domain: "gui/123".into(),
            plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            log_path: "/Users/test/.forge/logs/gateway.log".into(),
            error_log_path: "/Users/test/.forge/logs/gateway-error.log".into(),
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        assert!(json.contains("\"installed\":true"));
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"supported\":true"));
        assert!(json.contains("\"launch_domain\":\"gui/123\""));
        assert!(json.contains("gateway-error.log"));
    }

    #[test]
    fn unsupported_platform_status() {
        // This test verifies the payload shape — actual platform detection
        // depends on cfg!(target_os) at compile time.
        let payload = ServiceStatusPayload {
            installed: false,
            running: false,
            message: "unsupported".into(),
            supported: false,
            label: "com.forge.gateway".into(),
            launch_domain: "unsupported".into(),
            plist_path: "".into(),
            log_path: "".into(),
            error_log_path: "".into(),
        };
        assert!(!payload.supported);
        assert_eq!(payload.message, "unsupported");
    }

    #[test]
    fn service_status_payload_uses_structured_launchd_status() {
        let payload = ServiceStatusPayload::from(launchd::LaunchdServiceStatus {
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

        assert!(payload.supported);
        assert!(payload.installed);
        assert!(!payload.running);
        assert_eq!(payload.launch_domain, "gui/123");
        assert!(payload.message.contains("installed but not running"));
    }
}
