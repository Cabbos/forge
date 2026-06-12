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
        });
    }

    let plist_path = launchd::plist_path();
    let installed = plist_path.exists();

    // Check running status via launchctl.
    let running = if installed {
        let output = std::process::Command::new("launchctl")
            .args(["print", &format!("gui/501/{}", launchd::SERVICE_LABEL)])
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
    })
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
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        assert!(json.contains("\"installed\":true"));
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"supported\":true"));
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
        };
        assert!(!payload.supported);
        assert_eq!(payload.message, "unsupported");
    }
}
