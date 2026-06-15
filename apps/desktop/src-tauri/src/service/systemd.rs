//! Linux systemd user service integration for the Forge Gateway.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdInstallCommandPlan {
    pub unit_path: String,
    pub daemon_reload: Vec<String>,
    pub enable_now: Vec<String>,
}

pub fn generate_unit() -> String {
    generate_unit_for_paths(
        gateway_binary_path(),
        home_dir(),
        gateway_log_path(),
        gateway_error_log_path(),
    )
}

pub fn install() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return unsupported_lifecycle_operation("install");
    }

    let unit_path = user_unit_path();
    let log_dir = gateway_log_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home_dir().join(".forge").join("logs"));

    install_with_runner(&unit_path, &log_dir, generate_unit(), run_systemctl)
}

fn install_with_runner(
    unit_path: impl AsRef<Path>,
    log_dir: impl AsRef<Path>,
    unit_content: String,
    mut run: impl FnMut(&[&str], bool) -> Result<String, String>,
) -> Result<String, String> {
    let unit_path = unit_path.as_ref();
    if let Some(parent) = unit_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create systemd dir: {error}"))?;
    }
    fs::create_dir_all(log_dir.as_ref()).map_err(|error| format!("create log dir: {error}"))?;
    fs::write(unit_path, unit_content.as_bytes())
        .map_err(|error| format!("write systemd unit: {error}"))?;

    run(&["--user", "daemon-reload"], false)?;
    run(&["--user", "enable", "--now", UNIT_NAME], false)?;

    Ok(format!(
        "Service '{UNIT_NAME}' installed and started via systemd."
    ))
}

pub fn uninstall() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return unsupported_lifecycle_operation("uninstall");
    }

    uninstall_with_runner(user_unit_path(), run_systemctl)
}

fn uninstall_with_runner(
    unit_path: impl AsRef<Path>,
    mut run: impl FnMut(&[&str], bool) -> Result<String, String>,
) -> Result<String, String> {
    let unit_path = unit_path.as_ref();
    run(&["--user", "disable", "--now", UNIT_NAME], true)?;
    if unit_path.exists() {
        fs::remove_file(unit_path).map_err(|error| format!("remove systemd unit: {error}"))?;
    }
    run(&["--user", "daemon-reload"], false)?;

    Ok(format!("Service '{UNIT_NAME}' uninstalled."))
}

pub fn start() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return unsupported_lifecycle_operation("start");
    }

    if !user_unit_path().exists() {
        return install();
    }

    run_systemctl(&["--user", "start", UNIT_NAME], false)
        .map(|_| format!("Service '{UNIT_NAME}' started."))
}

pub fn stop() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return unsupported_lifecycle_operation("stop");
    }

    run_systemctl(&["--user", "stop", UNIT_NAME], true)
        .map(|_| format!("Service '{UNIT_NAME}' stopped."))
}

pub fn restart() -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return unsupported_lifecycle_operation("restart");
    }

    if !user_unit_path().exists() {
        return install();
    }

    run_systemctl(&["--user", "restart", UNIT_NAME], false)
        .map(|_| format!("Service '{UNIT_NAME}' restarted."))
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

pub fn install_command_plan() -> SystemdInstallCommandPlan {
    install_command_plan_for_unit_path(user_unit_path())
}

pub fn install_command_plan_for_unit_path(
    unit_path: impl AsRef<Path>,
) -> SystemdInstallCommandPlan {
    SystemdInstallCommandPlan {
        unit_path: unit_path.as_ref().display().to_string(),
        daemon_reload: vec!["--user".to_string(), "daemon-reload".to_string()],
        enable_now: vec![
            "--user".to_string(),
            "enable".to_string(),
            "--now".to_string(),
            UNIT_NAME.to_string(),
        ],
    }
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
    let output = Command::new("systemctl")
        .args(["--user", "is-active", UNIT_NAME])
        .output()
        .map_err(|error| format!("systemctl: {error}"))?;

    Ok(service_status_from_parts(
        true,
        installed,
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    ))
}

fn service_status_from_parts(
    supported: bool,
    installed: bool,
    is_active_success: bool,
    stdout: &str,
    stderr: &str,
) -> SystemdServiceStatus {
    if !supported {
        return unsupported_service_status();
    }

    let state = stdout.trim();
    let missing =
        systemctl_reports_missing_service(stdout) || systemctl_reports_missing_service(stderr);
    let running = installed && is_active_success && state == "active";
    let status_message = if running {
        format!("Service '{UNIT_NAME}' is running.")
    } else if !installed || missing {
        format!("Service '{UNIT_NAME}' is not installed.")
    } else if state.is_empty() {
        let detail = if stderr.trim().is_empty() {
            "unknown"
        } else {
            stderr.trim()
        };
        format!("Service '{UNIT_NAME}' is not running: {detail}")
    } else {
        format!("Service '{UNIT_NAME}' is not running: {state}")
    };
    let message = match (installed, running) {
        (true, true) => "Gateway systemd user service is installed and running.".to_string(),
        (true, false) => "Gateway systemd user service is installed but not running.".to_string(),
        (false, _) => "Gateway systemd user service is not installed.".to_string(),
    };

    SystemdServiceStatus {
        supported: true,
        installed,
        running,
        message,
        unit_name: UNIT_NAME.to_string(),
        unit_path: user_unit_path().display().to_string(),
        log_path: gateway_log_path().display().to_string(),
        error_log_path: gateway_error_log_path().display().to_string(),
        status_message,
    }
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

fn unsupported_lifecycle_operation(operation: &str) -> Result<String, String> {
    Err(format!(
        "Systemd service {operation} is only supported on Linux."
    ))
}

fn run_systemctl(args: &[&str], allow_missing: bool) -> Result<String, String> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .map_err(|error| format!("systemctl: {error}"))?;
    interpret_systemctl_result(
        args,
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        allow_missing,
    )
}

fn interpret_systemctl_result(
    args: &[&str],
    success: bool,
    stdout: &str,
    stderr: &str,
    allow_missing: bool,
) -> Result<String, String> {
    if success {
        return Ok(stdout.trim().to_string());
    }

    if allow_missing
        && (systemctl_reports_missing_service(stdout) || systemctl_reports_missing_service(stderr))
    {
        return Ok(format!("Service '{UNIT_NAME}' is not installed."));
    }

    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    Err(format!("systemctl {} failed: {detail}", args.join(" ")))
}

fn systemctl_reports_missing_service(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("not found")
        || lower.contains("could not be found")
        || lower.contains("does not exist")
        || lower.contains("no such file")
        || lower.contains("unit forge-gateway.service not loaded")
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

    #[test]
    fn service_management_api_exports_lifecycle_commands() {
        let _: fn() -> Result<String, String> = install;
        let _: fn() -> Result<String, String> = uninstall;
        let _: fn() -> Result<String, String> = start;
        let _: fn() -> Result<String, String> = stop;
        let _: fn() -> Result<String, String> = restart;
    }

    #[test]
    fn install_command_plan_enables_user_unit_now() {
        let plan = install_command_plan_for_unit_path(
            "/home/alice/.config/systemd/user/forge-gateway.service",
        );

        assert_eq!(
            plan.daemon_reload,
            vec!["--user".to_string(), "daemon-reload".to_string()]
        );
        assert_eq!(
            plan.enable_now,
            vec![
                "--user".to_string(),
                "enable".to_string(),
                "--now".to_string(),
                "forge-gateway.service".to_string(),
            ]
        );
        assert_eq!(
            plan.unit_path,
            "/home/alice/.config/systemd/user/forge-gateway.service"
        );
    }

    #[test]
    fn service_status_from_parts_detects_running_unit() {
        let status = service_status_from_parts(true, true, true, "active\n", "");

        assert!(status.supported);
        assert!(status.installed);
        assert!(status.running);
        assert!(status.message.contains("installed and running"));
        assert!(status.status_message.contains("is running"));
    }

    #[test]
    fn service_status_from_parts_detects_installed_but_inactive_unit() {
        let status = service_status_from_parts(true, true, false, "inactive\n", "");

        assert!(status.supported);
        assert!(status.installed);
        assert!(!status.running);
        assert!(status.message.contains("installed but not running"));
        assert!(status.status_message.contains("not running"));
    }

    #[test]
    fn install_with_runner_writes_unit_and_runs_systemctl_commands() {
        let root = temp_root("systemd-install-runner");
        let unit_path = user_unit_path_for_home(&root);
        let log_dir = root.join(".forge").join("logs");
        let mut calls = Vec::new();

        let message = install_with_runner(
            &unit_path,
            &log_dir,
            generate_unit_for_paths(
                "/opt/forge/gateway",
                &root,
                log_dir.join("gateway.log"),
                log_dir.join("gateway-error.log"),
            ),
            |args, allow_missing| {
                calls.push((
                    args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
                    allow_missing,
                ));
                Ok("ok".to_string())
            },
        )
        .expect("install with runner");

        assert!(message.contains("installed and started via systemd"));
        assert!(unit_path.exists());
        assert!(log_dir.exists());
        let unit = std::fs::read_to_string(&unit_path).expect("unit written");
        assert!(unit.contains("ExecStart=/opt/forge/gateway"));
        assert_eq!(
            calls,
            vec![
                (
                    vec!["--user".to_string(), "daemon-reload".to_string()],
                    false,
                ),
                (
                    vec![
                        "--user".to_string(),
                        "enable".to_string(),
                        "--now".to_string(),
                        "forge-gateway.service".to_string(),
                    ],
                    false,
                ),
            ]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn uninstall_with_runner_removes_unit_and_reloads_systemd() {
        let root = temp_root("systemd-uninstall-runner");
        let unit_path = user_unit_path_for_home(&root);
        std::fs::create_dir_all(unit_path.parent().expect("unit parent")).expect("unit parent");
        std::fs::write(&unit_path, "[Unit]\nDescription=Forge Gateway\n").expect("unit");
        let mut calls = Vec::new();

        let message = uninstall_with_runner(&unit_path, |args, allow_missing| {
            calls.push((
                args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
                allow_missing,
            ));
            Ok("ok".to_string())
        })
        .expect("uninstall with runner");

        assert!(message.contains("uninstalled"));
        assert!(!unit_path.exists());
        assert_eq!(
            calls,
            vec![
                (
                    vec![
                        "--user".to_string(),
                        "disable".to_string(),
                        "--now".to_string(),
                        "forge-gateway.service".to_string(),
                    ],
                    true,
                ),
                (
                    vec!["--user".to_string(), "daemon-reload".to_string()],
                    false,
                ),
            ]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("forge-{name}-{nanos}"))
    }
}
