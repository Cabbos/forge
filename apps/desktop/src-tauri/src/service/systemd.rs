//! Linux systemd user service integration for the Forge Gateway.

use std::path::{Path, PathBuf};

pub const UNIT_NAME: &str = "forge-gateway.service";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdServiceStatus {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub message: String,
    pub unit_name: String,
    pub unit_path: String,
    pub log_path: String,
    pub error_log_path: String,
    pub status_message: String,
}

pub fn generate_unit() -> String {
    generate_unit_for_paths(
        gateway_binary_path(),
        home_dir(),
        gateway_log_path(),
        gateway_error_log_path(),
    )
}

pub fn generate_unit_for_paths(
    gateway_binary: impl AsRef<Path>,
    home: impl AsRef<Path>,
    stdout_log: impl AsRef<Path>,
    stderr_log: impl AsRef<Path>,
) -> String {
    format!(
        r#"[Unit]
Description=Forge Gateway
After=network.target

[Service]
Type=simple
ExecStart={gateway_binary}
Restart=always
RestartSec=5
Environment=HOME={home}
Environment=RUST_LOG=info
StandardOutput=append:{stdout_log}
StandardError=append:{stderr_log}

[Install]
WantedBy=default.target
"#,
        gateway_binary = gateway_binary.as_ref().display(),
        home = home.as_ref().display(),
        stdout_log = stdout_log.as_ref().display(),
        stderr_log = stderr_log.as_ref().display(),
    )
}

pub fn user_unit_path() -> PathBuf {
    user_unit_path_for_home(home_dir())
}

pub fn user_unit_path_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref()
        .join(".config")
        .join("systemd")
        .join("user")
        .join(UNIT_NAME)
}

pub fn gateway_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("FORGE_GATEWAY_BIN") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("gateway"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("gateway")
}

pub fn gateway_log_path() -> PathBuf {
    home_dir().join(".forge").join("logs").join("gateway.log")
}

pub fn gateway_error_log_path() -> PathBuf {
    home_dir()
        .join(".forge")
        .join("logs")
        .join("gateway-error.log")
}

pub fn status() -> Result<String, String> {
    query_status().map(|status| status.status_message)
}

pub fn query_status() -> Result<SystemdServiceStatus, String> {
    if !cfg!(target_os = "linux") {
        return Ok(unsupported_service_status());
    }

    let unit_path = user_unit_path();
    let installed = unit_path.exists();
    Ok(SystemdServiceStatus {
        supported: true,
        installed,
        running: false,
        message: if installed {
            "Gateway systemd user service is installed.".to_string()
        } else {
            "Gateway systemd user service is not installed.".to_string()
        },
        unit_name: UNIT_NAME.to_string(),
        unit_path: unit_path.display().to_string(),
        log_path: gateway_log_path().display().to_string(),
        error_log_path: gateway_error_log_path().display().to_string(),
        status_message: "Systemd status probing is not installed yet.".to_string(),
    })
}

fn unsupported_service_status() -> SystemdServiceStatus {
    SystemdServiceStatus {
        supported: false,
        installed: false,
        running: false,
        message: "Systemd user service management is only supported on Linux.".to_string(),
        unit_name: UNIT_NAME.to_string(),
        unit_path: String::new(),
        log_path: String::new(),
        error_log_path: String::new(),
        status_message: "Systemd user service management is only supported on Linux.".to_string(),
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_unit_contains_gateway_binary_and_restart_policy() {
        let unit = generate_unit_for_paths(
            "/opt/forge/gateway",
            "/home/alice",
            "/home/alice/.forge/logs/gateway.log",
            "/home/alice/.forge/logs/gateway-error.log",
        );

        assert!(unit.contains("[Unit]"));
        assert!(unit.contains("Description=Forge Gateway"));
        assert!(unit.contains("ExecStart=/opt/forge/gateway"));
        assert!(unit.contains("Restart=always"));
        assert!(unit.contains("Environment=HOME=/home/alice"));
        assert!(unit.contains("Environment=RUST_LOG=info"));
        assert!(unit.contains("StandardOutput=append:/home/alice/.forge/logs/gateway.log"));
        assert!(unit.contains("StandardError=append:/home/alice/.forge/logs/gateway-error.log"));
        assert!(unit.contains("WantedBy=default.target"));
    }

    #[test]
    fn user_unit_path_uses_systemd_user_directory() {
        let home = std::path::PathBuf::from("/home/alice");

        assert_eq!(
            user_unit_path_for_home(&home),
            home.join(".config")
                .join("systemd")
                .join("user")
                .join("forge-gateway.service")
        );
    }

    #[test]
    fn unsupported_status_is_structured_on_non_linux() {
        let status = unsupported_service_status();

        assert!(!status.supported);
        assert!(!status.installed);
        assert!(!status.running);
        assert_eq!(status.unit_name, "forge-gateway.service");
        assert!(status.message.contains("Linux"));
    }
}
