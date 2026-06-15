//! Windows service wrapper planning for the Forge Gateway.

use std::path::{Path, PathBuf};
use std::process::Command;

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

pub fn install() -> Result<String, String> {
    if !cfg!(target_os = "windows") {
        return unsupported_lifecycle_operation("install");
    }

    let plan = command_plan();
    run_sc_plan_command(&plan.create, false, true, false)?;
    run_sc_plan_command(&plan.start, false, false, true)?;

    Ok(format!(
        "Service '{SERVICE_NAME}' installed and started via Windows Service Control."
    ))
}

pub fn uninstall() -> Result<String, String> {
    if !cfg!(target_os = "windows") {
        return unsupported_lifecycle_operation("uninstall");
    }

    let plan = command_plan();
    run_sc_plan_command(&plan.stop, true, false, false)?;
    run_sc_plan_command(&plan.delete, true, false, false)?;

    Ok(format!("Service '{SERVICE_NAME}' uninstalled."))
}

pub fn start() -> Result<String, String> {
    if !cfg!(target_os = "windows") {
        return unsupported_lifecycle_operation("start");
    }

    let plan = command_plan();
    run_sc_plan_command(&plan.start, false, false, true)
        .map(|_| format!("Service '{SERVICE_NAME}' started."))
}

pub fn stop() -> Result<String, String> {
    if !cfg!(target_os = "windows") {
        return unsupported_lifecycle_operation("stop");
    }

    let plan = command_plan();
    run_sc_plan_command(&plan.stop, true, false, false)
        .map(|_| format!("Service '{SERVICE_NAME}' stopped."))
}

pub fn restart() -> Result<String, String> {
    if !cfg!(target_os = "windows") {
        return unsupported_lifecycle_operation("restart");
    }

    let plan = command_plan();
    run_sc_plan_command(&plan.stop, true, false, false)?;
    run_sc_plan_command(&plan.start, false, false, true)
        .map(|_| format!("Service '{SERVICE_NAME}' restarted."))
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

    let output = Command::new("sc.exe")
        .args(["query", SERVICE_NAME])
        .output()
        .map_err(|error| format!("sc.exe: {error}"))?;

    Ok(service_status_from_query_output(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    ))
}

fn service_status_from_query_output(
    query_success: bool,
    stdout: &str,
    stderr: &str,
) -> WindowsServiceStatus {
    let missing = sc_reports_missing_service(stdout) || sc_reports_missing_service(stderr);
    let output = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let upper = output.to_uppercase();
    let installed = query_success && !missing;
    let running = installed && upper.contains("STATE") && upper.contains("RUNNING");
    let status_message = if running {
        format!("Service '{SERVICE_NAME}' is running.")
    } else if missing || !installed {
        format!("Service '{SERVICE_NAME}' is not installed.")
    } else {
        let state = if upper.contains("STOPPED") {
            "stopped"
        } else if output.trim().is_empty() {
            "unknown"
        } else {
            output.trim()
        };
        format!("Service '{SERVICE_NAME}' is not running: {state}")
    };
    let message = match (installed, running) {
        (true, true) => "Gateway Windows service is installed and running.".to_string(),
        (true, false) => "Gateway Windows service is installed but not running.".to_string(),
        (false, _) => "Gateway Windows service is not installed.".to_string(),
    };

    WindowsServiceStatus {
        supported: true,
        installed,
        running,
        message,
        service_name: SERVICE_NAME.to_string(),
        status_message,
    }
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

fn unsupported_lifecycle_operation(operation: &str) -> Result<String, String> {
    Err(format!(
        "Windows service {operation} is only supported on Windows."
    ))
}

fn run_sc_plan_command(
    command: &[String],
    allow_missing: bool,
    allow_existing: bool,
    allow_already_running: bool,
) -> Result<String, String> {
    let args: Vec<&str> = command
        .iter()
        .map(String::as_str)
        .skip_while(|arg| arg.eq_ignore_ascii_case("sc.exe"))
        .collect();
    let output = Command::new("sc.exe")
        .args(&args)
        .output()
        .map_err(|error| format!("sc.exe: {error}"))?;
    interpret_sc_result(
        &args,
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        allow_missing,
    )
    .or_else(|error| {
        if allow_existing
            && (sc_reports_existing_service(&error)
                || sc_reports_existing_service(&String::from_utf8_lossy(&output.stderr)))
        {
            Ok(format!("Service '{SERVICE_NAME}' already exists."))
        } else if allow_already_running
            && (sc_reports_already_running(&error)
                || sc_reports_already_running(&String::from_utf8_lossy(&output.stderr)))
        {
            Ok(format!("Service '{SERVICE_NAME}' is already running."))
        } else {
            Err(error)
        }
    })
}

fn interpret_sc_result(
    args: &[&str],
    success: bool,
    stdout: &str,
    stderr: &str,
    allow_missing: bool,
) -> Result<String, String> {
    if success {
        return Ok(stdout.trim().to_string());
    }

    if allow_missing && (sc_reports_missing_service(stdout) || sc_reports_missing_service(stderr)) {
        return Ok(format!("Service '{SERVICE_NAME}' is not installed."));
    }

    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    Err(format!("sc.exe {} failed: {detail}", args.join(" ")))
}

fn sc_reports_missing_service(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("failed 1060")
        || lower.contains("does not exist as an installed service")
        || lower.contains("specified service does not exist")
}

fn sc_reports_existing_service(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("failed 1073") || lower.contains("specified service already exists")
}

fn sc_reports_already_running(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("failed 1056")
        || lower.contains("instance of the service is already running")
        || lower.contains("service has already been started")
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

    #[test]
    fn service_management_api_exports_lifecycle_commands() {
        let _: fn() -> Result<String, String> = install;
        let _: fn() -> Result<String, String> = uninstall;
        let _: fn() -> Result<String, String> = start;
        let _: fn() -> Result<String, String> = stop;
        let _: fn() -> Result<String, String> = restart;
    }

    #[test]
    fn service_status_from_query_output_detects_running_service() {
        let output = r#"
SERVICE_NAME: ForgeGateway
        TYPE               : 10  WIN32_OWN_PROCESS
        STATE              : 4  RUNNING
"#;
        let status = service_status_from_query_output(true, output, "");

        assert!(status.supported);
        assert!(status.installed);
        assert!(status.running);
        assert!(status.message.contains("installed and running"));
        assert!(status.status_message.contains("is running"));
    }

    #[test]
    fn service_status_from_query_output_detects_stopped_service() {
        let output = r#"
SERVICE_NAME: ForgeGateway
        STATE              : 1  STOPPED
"#;
        let status = service_status_from_query_output(true, output, "");

        assert!(status.supported);
        assert!(status.installed);
        assert!(!status.running);
        assert!(status.message.contains("installed but not running"));
        assert!(status.status_message.contains("not running"));
    }

    #[test]
    fn service_status_from_query_output_detects_missing_service() {
        let status = service_status_from_query_output(
            false,
            "",
            "[SC] OpenService FAILED 1060: The specified service does not exist as an installed service.",
        );

        assert!(status.supported);
        assert!(!status.installed);
        assert!(!status.running);
        assert!(status.message.contains("not installed"));
        assert!(status.status_message.contains("not installed"));
    }

    #[test]
    fn sc_result_allows_missing_service_when_requested() {
        let message = interpret_sc_result(
            &["delete", "ForgeGateway"],
            false,
            "",
            "[SC] OpenService FAILED 1060",
            true,
        )
        .expect("missing service");

        assert!(message.contains("not installed"));
    }
}
