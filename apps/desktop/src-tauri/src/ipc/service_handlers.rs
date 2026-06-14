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

/// Query the current service status.
#[tauri::command]
pub async fn get_service_status() -> Result<ServiceStatusPayload, String> {
    // Check if we're on macOS (only supported platform for now).
    let supported = cfg!(target_os = "macos");
    if !supported {
        return Ok(ServiceStatusPayload {
            installed: false,
            running: false,
            message: "Service management is only supported on macOS.".to_string(),
            supported: false,
            label: launchd::SERVICE_LABEL.to_string(),
            launch_domain: "unsupported".to_string(),
            plist_path: String::new(),
            log_path: String::new(),
            error_log_path: String::new(),
        });
    }

    let plist_path = launchd::plist_path();
    let installed = plist_path.exists();
    let launch_domain = launchd::launchctl_domain();

    // Check running status via launchctl.
    let running = if installed {
        let output = std::process::Command::new("launchctl")
            .args(["print", &launchctl_print_target(&launch_domain)])
            .output();
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains("state = running")
            }
            Err(_) => false,
        }
    } else {
        false
    };

    let message = match (installed, running) {
        (true, true) => "Gateway service is installed and running.".to_string(),
        (true, false) => "Gateway service is installed but not running.".to_string(),
        (false, _) => "Gateway service is not installed.".to_string(),
    };

    Ok(ServiceStatusPayload {
        installed,
        running,
        message,
        supported: true,
        label: launchd::SERVICE_LABEL.to_string(),
        launch_domain,
        plist_path: plist_path.display().to_string(),
        log_path: launchd::gateway_log_path().display().to_string(),
        error_log_path: launchd::gateway_error_log_path().display().to_string(),
    })
}

fn launchctl_print_target(domain: &str) -> String {
    format!("{domain}/{}", launchd::SERVICE_LABEL)
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

    // Return updated status.
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
    fn launchctl_print_target_uses_supplied_domain() {
        assert_eq!(
            launchctl_print_target("gui/123"),
            "gui/123/com.forge.gateway"
        );
    }
}
