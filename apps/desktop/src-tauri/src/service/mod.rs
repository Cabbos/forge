//! OS service management — install, uninstall, start, stop, status.
//!
//! Platform support:
//! - macOS: launchd plist at `~/Library/LaunchAgents/com.forge.gateway.plist`
//! - Linux: systemd user unit at `~/.config/systemd/user/forge-gateway.service`
//! - Windows: service wrapper command plan

use std::fmt::Display;
use std::path::PathBuf;

pub mod launchd;
pub mod systemd;
pub mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceBackend {
    Launchd,
    Systemd,
    Windows,
    Unsupported,
}

impl ServiceBackend {
    pub fn for_target_os(target_os: &str) -> Self {
        match target_os {
            "macos" => Self::Launchd,
            "linux" => Self::Systemd,
            "windows" => Self::Windows,
            _ => Self::Unsupported,
        }
    }

    pub fn current() -> Self {
        Self::for_target_os(std::env::consts::OS)
    }

    pub fn supports_command(self, _command: ServiceCommand) -> bool {
        match self {
            Self::Launchd | Self::Systemd | Self::Windows => true,
            Self::Unsupported => false,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Systemd => "systemd",
            Self::Windows => "windows-service",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatusSnapshot {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub message: String,
    pub backend: String,
    pub service_id: String,
    pub label: String,
    pub launch_domain: String,
    pub service_path: String,
    pub plist_path: String,
    pub log_path: String,
    pub error_log_path: String,
    pub status_message: String,
}

impl ServiceStatusSnapshot {
    pub fn from_launchd_status(status: launchd::LaunchdServiceStatus) -> Self {
        Self {
            supported: status.supported,
            installed: status.installed,
            running: status.running,
            message: status.message,
            backend: ServiceBackend::Launchd.label().to_string(),
            service_id: status.label.clone(),
            label: status.label,
            launch_domain: status.launch_domain,
            service_path: status.plist_path.clone(),
            plist_path: status.plist_path,
            log_path: status.log_path,
            error_log_path: status.error_log_path,
            status_message: status.status_message,
        }
    }

    pub fn from_systemd_status(status: systemd::SystemdServiceStatus) -> Self {
        Self {
            supported: status.supported,
            installed: status.installed,
            running: status.running,
            message: status.message,
            backend: ServiceBackend::Systemd.label().to_string(),
            service_id: status.unit_name.clone(),
            label: status.unit_name,
            launch_domain: "systemd-user".to_string(),
            service_path: status.unit_path.clone(),
            plist_path: status.unit_path,
            log_path: status.log_path,
            error_log_path: status.error_log_path,
            status_message: status.status_message,
        }
    }

    pub fn from_windows_status(status: windows::WindowsServiceStatus) -> Self {
        Self {
            supported: status.supported,
            installed: status.installed,
            running: status.running,
            message: status.message,
            backend: ServiceBackend::Windows.label().to_string(),
            service_id: status.service_name.clone(),
            label: status.service_name,
            launch_domain: "windows-service-control".to_string(),
            service_path: String::new(),
            plist_path: String::new(),
            log_path: String::new(),
            error_log_path: String::new(),
            status_message: status.status_message,
        }
    }

    fn unsupported(backend: ServiceBackend, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            supported: false,
            installed: false,
            running: false,
            message: message.clone(),
            backend: backend.label().to_string(),
            service_id: String::new(),
            label: String::new(),
            launch_domain: "unsupported".to_string(),
            service_path: String::new(),
            plist_path: String::new(),
            log_path: String::new(),
            error_log_path: String::new(),
            status_message: message,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCommand {
    Install,
    Uninstall,
    Status,
    Start,
    Stop,
    Restart,
}

impl ServiceCommand {
    pub fn parse(command: &str) -> Result<Self, String> {
        match command {
            "install" => Ok(Self::Install),
            "uninstall" => Ok(Self::Uninstall),
            "status" => Ok(Self::Status),
            "start" => Ok(Self::Start),
            "stop" => Ok(Self::Stop),
            "restart" => Ok(Self::Restart),
            cmd => Err(format!("Unknown service command: {cmd}")),
        }
    }
}

pub fn run_service_command(command: &str) -> Result<String, String> {
    match ServiceCommand::parse(command)? {
        ServiceCommand::Install => install(),
        ServiceCommand::Uninstall => uninstall(),
        ServiceCommand::Status => status(),
        ServiceCommand::Start => start(),
        ServiceCommand::Stop => stop(),
        ServiceCommand::Restart => restart(),
    }
}

pub fn install() -> Result<String, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => launchd::install(),
        ServiceBackend::Systemd => systemd::install(),
        ServiceBackend::Windows => windows::install(),
        backend => unsupported_lifecycle_operation(backend, "install"),
    }
}

pub fn uninstall() -> Result<String, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => launchd::uninstall(),
        ServiceBackend::Systemd => systemd::uninstall(),
        ServiceBackend::Windows => windows::uninstall(),
        backend => unsupported_lifecycle_operation(backend, "uninstall"),
    }
}

pub fn status() -> Result<String, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => launchd::status(),
        ServiceBackend::Systemd => systemd::status(),
        ServiceBackend::Windows => windows::status(),
        ServiceBackend::Unsupported => {
            unsupported_lifecycle_operation(ServiceBackend::Unsupported, "status")
        }
    }
}

pub fn query_status_snapshot() -> Result<ServiceStatusSnapshot, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => {
            launchd::query_status().map(ServiceStatusSnapshot::from_launchd_status)
        }
        ServiceBackend::Systemd => {
            systemd::query_status().map(ServiceStatusSnapshot::from_systemd_status)
        }
        ServiceBackend::Windows => {
            windows::query_status().map(ServiceStatusSnapshot::from_windows_status)
        }
        ServiceBackend::Unsupported => Ok(ServiceStatusSnapshot::unsupported(
            ServiceBackend::Unsupported,
            "Service management is not supported on this platform.",
        )),
    }
}

pub fn service_definition_path(backend: ServiceBackend) -> Option<PathBuf> {
    match backend {
        ServiceBackend::Launchd => Some(launchd::plist_path()),
        ServiceBackend::Systemd => Some(systemd::user_unit_path()),
        ServiceBackend::Windows | ServiceBackend::Unsupported => None,
    }
}

pub fn unavailable_status_snapshot(error: impl Display) -> ServiceStatusSnapshot {
    unavailable_status_snapshot_for_backend(ServiceBackend::current(), error)
}

pub fn unavailable_status_snapshot_for_backend(
    backend: ServiceBackend,
    error: impl Display,
) -> ServiceStatusSnapshot {
    let service_path = service_definition_path(backend);
    let installed = service_path.as_ref().is_some_and(|path| path.exists());
    let service_path = service_path
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let message = format!("Gateway service status unavailable: {error}");

    ServiceStatusSnapshot {
        supported: backend != ServiceBackend::Unsupported,
        installed,
        running: false,
        message: message.clone(),
        backend: backend.label().to_string(),
        service_id: service_id_for_backend(backend).to_string(),
        label: service_id_for_backend(backend).to_string(),
        launch_domain: launch_domain_for_backend(backend),
        service_path: service_path.clone(),
        plist_path: service_path,
        log_path: log_path_for_backend(backend),
        error_log_path: error_log_path_for_backend(backend),
        status_message: message,
    }
}

fn service_id_for_backend(backend: ServiceBackend) -> &'static str {
    match backend {
        ServiceBackend::Launchd => launchd::SERVICE_LABEL,
        ServiceBackend::Systemd => systemd::UNIT_NAME,
        ServiceBackend::Windows => windows::SERVICE_NAME,
        ServiceBackend::Unsupported => "",
    }
}

fn launch_domain_for_backend(backend: ServiceBackend) -> String {
    match backend {
        ServiceBackend::Launchd => launchd::launchctl_domain(),
        ServiceBackend::Systemd => "systemd-user".to_string(),
        ServiceBackend::Windows => "windows-service-control".to_string(),
        ServiceBackend::Unsupported => "unsupported".to_string(),
    }
}

fn log_path_for_backend(backend: ServiceBackend) -> String {
    match backend {
        ServiceBackend::Launchd => launchd::gateway_log_path().display().to_string(),
        ServiceBackend::Systemd => systemd::gateway_log_path().display().to_string(),
        ServiceBackend::Windows | ServiceBackend::Unsupported => String::new(),
    }
}

fn error_log_path_for_backend(backend: ServiceBackend) -> String {
    match backend {
        ServiceBackend::Launchd => launchd::gateway_error_log_path().display().to_string(),
        ServiceBackend::Systemd => systemd::gateway_error_log_path().display().to_string(),
        ServiceBackend::Windows | ServiceBackend::Unsupported => String::new(),
    }
}

pub fn start() -> Result<String, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => launchd::start(),
        ServiceBackend::Systemd => systemd::start(),
        ServiceBackend::Windows => windows::start(),
        backend => unsupported_lifecycle_operation(backend, "start"),
    }
}

pub fn stop() -> Result<String, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => launchd::stop(),
        ServiceBackend::Systemd => systemd::stop(),
        ServiceBackend::Windows => windows::stop(),
        backend => unsupported_lifecycle_operation(backend, "stop"),
    }
}

pub fn restart() -> Result<String, String> {
    match ServiceBackend::current() {
        ServiceBackend::Launchd => launchd::restart(),
        ServiceBackend::Systemd => systemd::restart(),
        ServiceBackend::Windows => windows::restart(),
        backend => unsupported_lifecycle_operation(backend, "restart"),
    }
}

fn unsupported_lifecycle_operation(
    backend: ServiceBackend,
    operation: &str,
) -> Result<String, String> {
    Err(format!(
        "Service {operation} is not available for the {} backend yet.",
        backend.label()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_for_target_os_maps_supported_services() {
        assert_eq!(
            ServiceBackend::for_target_os("macos"),
            ServiceBackend::Launchd
        );
        assert_eq!(
            ServiceBackend::for_target_os("linux"),
            ServiceBackend::Systemd
        );
        assert_eq!(
            ServiceBackend::for_target_os("windows"),
            ServiceBackend::Windows
        );
        assert_eq!(
            ServiceBackend::for_target_os("freebsd"),
            ServiceBackend::Unsupported
        );
    }

    #[test]
    fn service_command_parse_accepts_lifecycle_commands() {
        assert_eq!(
            ServiceCommand::parse("install"),
            Ok(ServiceCommand::Install)
        );
        assert_eq!(
            ServiceCommand::parse("uninstall"),
            Ok(ServiceCommand::Uninstall)
        );
        assert_eq!(ServiceCommand::parse("status"), Ok(ServiceCommand::Status));
        assert_eq!(ServiceCommand::parse("start"), Ok(ServiceCommand::Start));
        assert_eq!(ServiceCommand::parse("stop"), Ok(ServiceCommand::Stop));
        assert_eq!(
            ServiceCommand::parse("restart"),
            Ok(ServiceCommand::Restart)
        );
    }

    #[test]
    fn service_command_parse_rejects_unknown_commands() {
        assert_eq!(
            ServiceCommand::parse("reload"),
            Err("Unknown service command: reload".to_string())
        );
    }

    #[test]
    fn backend_command_support_marks_systemd_lifecycle_available() {
        assert!(ServiceBackend::Launchd.supports_command(ServiceCommand::Install));
        assert!(ServiceBackend::Systemd.supports_command(ServiceCommand::Install));
        assert!(ServiceBackend::Systemd.supports_command(ServiceCommand::Restart));
        assert!(ServiceBackend::Windows.supports_command(ServiceCommand::Status));
        assert!(ServiceBackend::Windows.supports_command(ServiceCommand::Install));
        assert!(ServiceBackend::Windows.supports_command(ServiceCommand::Restart));
        assert!(!ServiceBackend::Unsupported.supports_command(ServiceCommand::Status));
    }

    #[test]
    fn status_snapshot_maps_systemd_status() {
        let snapshot = ServiceStatusSnapshot::from_systemd_status(systemd::SystemdServiceStatus {
            supported: true,
            installed: true,
            running: false,
            message: "Gateway systemd user service is installed but not running.".into(),
            unit_name: "forge-gateway.service".into(),
            unit_path: "/home/alice/.config/systemd/user/forge-gateway.service".into(),
            log_path: "/home/alice/.forge/logs/gateway.log".into(),
            error_log_path: "/home/alice/.forge/logs/gateway-error.log".into(),
            status_message: "Service 'forge-gateway.service' is not running: inactive".into(),
        });

        assert_eq!(snapshot.backend, "systemd");
        assert_eq!(snapshot.service_id, "forge-gateway.service");
        assert_eq!(snapshot.label, "forge-gateway.service");
        assert_eq!(snapshot.launch_domain, "systemd-user");
        assert_eq!(
            snapshot.service_path,
            "/home/alice/.config/systemd/user/forge-gateway.service"
        );
        assert_eq!(snapshot.plist_path, snapshot.service_path);
        assert!(!snapshot.running);
    }

    #[test]
    fn status_snapshot_maps_windows_status() {
        let snapshot = ServiceStatusSnapshot::from_windows_status(windows::WindowsServiceStatus {
            supported: true,
            installed: true,
            running: true,
            message: "Gateway Windows service is installed and running.".into(),
            service_name: "ForgeGateway".into(),
            status_message: "Service 'ForgeGateway' is running.".into(),
        });

        assert_eq!(snapshot.backend, "windows-service");
        assert_eq!(snapshot.service_id, "ForgeGateway");
        assert_eq!(snapshot.label, "ForgeGateway");
        assert_eq!(snapshot.launch_domain, "windows-service-control");
        assert!(snapshot.service_path.is_empty());
        assert!(snapshot.plist_path.is_empty());
        assert!(snapshot.running);
    }

    #[test]
    fn service_definition_path_tracks_file_backed_platforms() {
        let launchd_path = service_definition_path(ServiceBackend::Launchd).unwrap();
        let systemd_path = service_definition_path(ServiceBackend::Systemd).unwrap();

        assert!(launchd_path.ends_with("com.forge.gateway.plist"));
        assert!(systemd_path.ends_with("forge-gateway.service"));
        assert_eq!(service_definition_path(ServiceBackend::Windows), None);
        assert_eq!(service_definition_path(ServiceBackend::Unsupported), None);
    }

    #[test]
    fn unavailable_status_snapshot_uses_backend_metadata() {
        let snapshot =
            unavailable_status_snapshot_for_backend(ServiceBackend::Windows, "sc.exe failed");

        assert!(snapshot.supported);
        assert!(!snapshot.installed);
        assert!(!snapshot.running);
        assert_eq!(snapshot.backend, "windows-service");
        assert_eq!(snapshot.service_id, "ForgeGateway");
        assert_eq!(snapshot.label, "ForgeGateway");
        assert_eq!(snapshot.launch_domain, "windows-service-control");
        assert!(snapshot.service_path.is_empty());
        assert!(snapshot.plist_path.is_empty());
        assert!(snapshot.log_path.is_empty());
        assert!(snapshot.error_log_path.is_empty());
        assert!(snapshot.status_message.contains("sc.exe failed"));
        assert!(snapshot.message.contains("status unavailable"));
    }
}
