//! IPC handlers for service management — autostart toggle, status query.

use crate::service::{self, launchd};
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
    /// Platform backend handling the service.
    pub backend: String,
    /// Platform-native service identifier.
    pub service_id: String,
    /// Launch service label.
    pub label: String,
    /// launchd domain used for status checks.
    pub launch_domain: String,
    /// Platform-native service definition path, when one exists.
    pub service_path: String,
    /// Expected launchd plist path.
    pub plist_path: String,
    /// Gateway stdout log path.
    pub log_path: String,
    /// Gateway stderr log path.
    pub error_log_path: String,
    /// Raw platform status message.
    pub status_message: String,
}

impl From<launchd::LaunchdServiceStatus> for ServiceStatusPayload {
    fn from(status: launchd::LaunchdServiceStatus) -> Self {
        Self::from(service::ServiceStatusSnapshot::from_launchd_status(status))
    }
}

impl From<service::ServiceStatusSnapshot> for ServiceStatusPayload {
    fn from(status: service::ServiceStatusSnapshot) -> Self {
        Self {
            installed: status.installed,
            running: status.running,
            message: status.message,
            supported: status.supported,
            backend: status.backend,
            service_id: status.service_id,
            label: status.label,
            launch_domain: status.launch_domain,
            service_path: status.service_path,
            plist_path: status.plist_path,
            log_path: status.log_path,
            error_log_path: status.error_log_path,
            status_message: status.status_message,
        }
    }
}

/// Query the current service status.
#[tauri::command]
pub async fn get_service_status() -> Result<ServiceStatusPayload, String> {
    service::query_status_snapshot().map(ServiceStatusPayload::from)
}

/// Enable or disable autostart (installs/uninstalls the platform service).
#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<ServiceStatusPayload, String> {
    if enabled {
        service::install()?;
    } else {
        service::uninstall()?;
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
            backend: "launchd".into(),
            service_id: "com.forge.gateway".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "gui/123".into(),
            service_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            log_path: "/Users/test/.forge/logs/gateway.log".into(),
            error_log_path: "/Users/test/.forge/logs/gateway-error.log".into(),
            status_message: "Service 'com.forge.gateway' is running.".into(),
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        assert!(json.contains("\"installed\":true"));
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"supported\":true"));
        assert!(json.contains("\"backend\":\"launchd\""));
        assert!(json.contains("\"service_id\":\"com.forge.gateway\""));
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
            backend: "unsupported".into(),
            service_id: "".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "unsupported".into(),
            service_path: "".into(),
            plist_path: "".into(),
            log_path: "".into(),
            error_log_path: "".into(),
            status_message: "unsupported".into(),
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

    #[test]
    fn service_status_payload_preserves_cross_platform_snapshot_fields() {
        let payload = ServiceStatusPayload::from(crate::service::ServiceStatusSnapshot {
            supported: true,
            installed: true,
            running: true,
            message: "Gateway systemd user service is installed and running.".into(),
            backend: "systemd".into(),
            service_id: "forge-gateway.service".into(),
            label: "forge-gateway.service".into(),
            launch_domain: "systemd-user".into(),
            service_path: "/home/alice/.config/systemd/user/forge-gateway.service".into(),
            plist_path: "/home/alice/.config/systemd/user/forge-gateway.service".into(),
            log_path: "/home/alice/.forge/logs/gateway.log".into(),
            error_log_path: "/home/alice/.forge/logs/gateway-error.log".into(),
            status_message: "Service 'forge-gateway.service' is running.".into(),
        });

        assert_eq!(payload.backend, "systemd");
        assert_eq!(payload.service_id, "forge-gateway.service");
        assert_eq!(payload.service_path, payload.plist_path);
        assert_eq!(
            payload.status_message,
            "Service 'forge-gateway.service' is running."
        );
    }
}
