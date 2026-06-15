//! Windows service wrapper planning for the Forge Gateway.

use std::path::{Path, PathBuf};

pub const SERVICE_NAME: &str = "ForgeGateway";
pub const DISPLAY_NAME: &str = "Forge Gateway";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsServiceCommandPlan {
    pub service_name: String,
    pub display_name: String,
    pub create: Vec<String>,
    pub start: Vec<String>,
    pub stop: Vec<String>,
    pub delete: Vec<String>,
    pub query: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsServiceStatus {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub message: String,
    pub service_name: String,
    pub status_message: String,
}

pub fn command_plan() -> WindowsServiceCommandPlan {
    command_plan_for_binary(gateway_binary_path())
}

pub fn command_plan_for_binary(gateway_binary: impl AsRef<Path>) -> WindowsServiceCommandPlan {
    let gateway_binary = gateway_binary.as_ref().display().to_string();

    WindowsServiceCommandPlan {
        service_name: SERVICE_NAME.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        create: vec![
            "sc.exe".to_string(),
            "create".to_string(),
            SERVICE_NAME.to_string(),
            "binPath=".to_string(),
            gateway_binary,
            "start=".to_string(),
            "auto".to_string(),
            "DisplayName=".to_string(),
            DISPLAY_NAME.to_string(),
        ],
        start: vec![
            "sc.exe".to_string(),
            "start".to_string(),
            SERVICE_NAME.to_string(),
        ],
        stop: vec![
            "sc.exe".to_string(),
            "stop".to_string(),
            SERVICE_NAME.to_string(),
        ],
        delete: vec![
            "sc.exe".to_string(),
            "delete".to_string(),
            SERVICE_NAME.to_string(),
        ],
        query: vec![
            "sc.exe".to_string(),
            "query".to_string(),
            SERVICE_NAME.to_string(),
        ],
    }
}

pub fn gateway_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("FORGE_GATEWAY_BIN") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("gateway.exe"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("gateway.exe")
}

pub fn status() -> Result<String, String> {
    query_status().map(|status| status.status_message)
}

pub fn query_status() -> Result<WindowsServiceStatus, String> {
    if !cfg!(target_os = "windows") {
        return Ok(unsupported_service_status());
    }

    Ok(WindowsServiceStatus {
        supported: true,
        installed: false,
        running: false,
        message: "Windows service status probing is not installed yet.".to_string(),
        service_name: SERVICE_NAME.to_string(),
        status_message: "Windows service status probing is not installed yet.".to_string(),
    })
}

fn unsupported_service_status() -> WindowsServiceStatus {
    WindowsServiceStatus {
        supported: false,
        installed: false,
        running: false,
        message: "Windows service management is only supported on Windows.".to_string(),
        service_name: SERVICE_NAME.to_string(),
        status_message: "Windows service management is only supported on Windows.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_plan_contains_sc_exe_lifecycle_commands() {
        let plan = command_plan_for_binary("C:\\Forge\\gateway.exe");

        assert_eq!(plan.service_name, "ForgeGateway");
        assert_eq!(plan.display_name, "Forge Gateway");
        assert_eq!(plan.create.first().map(String::as_str), Some("sc.exe"));
        assert!(plan.create.iter().any(|part| part == "create"));
        assert!(plan.create.iter().any(|part| part == "binPath="));
        assert!(plan
            .create
            .iter()
            .any(|part| part == "C:\\Forge\\gateway.exe"));
        assert!(plan.create.iter().any(|part| part == "start="));
        assert!(plan.start.iter().any(|part| part == "start"));
        assert!(plan.stop.iter().any(|part| part == "stop"));
        assert!(plan.delete.iter().any(|part| part == "delete"));
        assert!(plan.query.iter().any(|part| part == "query"));
    }

    #[test]
    fn unsupported_status_is_structured_on_non_windows() {
        let status = unsupported_service_status();

        assert!(!status.supported);
        assert!(!status.installed);
        assert!(!status.running);
        assert_eq!(status.service_name, "ForgeGateway");
        assert!(status.message.contains("Windows"));
    }
}
