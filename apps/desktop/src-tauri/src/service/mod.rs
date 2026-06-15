//! OS service management — install, uninstall, start, stop, status.
//!
//! Platform support:
//! - macOS: launchd plist at `~/Library/LaunchAgents/com.forge.gateway.plist`
//! - Linux: systemd user unit at `~/.config/systemd/user/forge-gateway.service`
//! - Windows: service wrapper command plan

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
}
