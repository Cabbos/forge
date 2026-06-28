//! macOS launchd integration — plist generation, install, uninstall, and
//! status checks for the Forge Gateway background service.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Label used in the launchd plist and `launchctl` commands.
pub const SERVICE_LABEL: &str = "com.forge.gateway";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchdServiceStatus {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub message: String,
    pub label: String,
    pub launch_domain: String,
    pub plist_path: String,
    pub log_path: String,
    pub error_log_path: String,
    pub status_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchctlCommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Current user's launchd GUI domain, e.g. `gui/501`.
pub fn launchctl_domain() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

/// Path to the launchd plist file.
pub fn plist_path() -> PathBuf {
    home_dir()
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{SERVICE_LABEL}.plist"))
}

/// Path to the gateway binary (same directory as the current executable at
/// runtime, or `target/debug/gateway` during development).
pub fn gateway_binary_path() -> PathBuf {
    // Prefer `FORGE_GATEWAY_BIN` env override for testing.
    if let Ok(p) = std::env::var("FORGE_GATEWAY_BIN") {
        return PathBuf::from(p);
    }
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("gateway"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("gateway")
}

/// Path to the gateway log file.
pub fn gateway_log_path() -> PathBuf {
    home_dir().join(".forge").join("logs").join("gateway.log")
}

/// Path to the gateway error log file.
pub fn gateway_error_log_path() -> PathBuf {
    home_dir()
        .join(".forge")
        .join("logs")
        .join("gateway-error.log")
}

// ── plist generation ────────────────────────────────────────────────────────

/// Generate the XML plist content for the launchd service.
pub fn generate_plist() -> String {
    let binary = gateway_binary_path();
    let stdout_log = gateway_log_path();
    let stderr_log = gateway_error_log_path();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{stdout_log}</string>
    <key>StandardErrorPath</key>
    <string>{stderr_log}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>{home}</string>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
</dict>
</plist>"#,
        label = SERVICE_LABEL,
        binary = binary.display(),
        stdout_log = stdout_log.display(),
        stderr_log = stderr_log.display(),
        home = home_dir().display(),
    )
}

// ── service operations ──────────────────────────────────────────────────────

/// Install the launchd service: write plist, create log dir, run
/// `launchctl bootstrap`.
pub fn install() -> Result<String, String> {
    let plist_path = plist_path();
    let log_dir = gateway_log_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| home_dir().join(".forge").join("logs"));

    install_with_runner(
        &plist_path,
        &log_dir,
        generate_plist(),
        launchctl_domain(),
        run_launchctl,
    )
}

fn install_with_runner(
    plist_path: impl AsRef<Path>,
    log_dir: impl AsRef<Path>,
    plist_content: String,
    domain: String,
    mut run: impl FnMut(&[&str]) -> Result<LaunchctlCommandOutput, String>,
) -> Result<String, String> {
    let plist_path = plist_path.as_ref();
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }

    fs::create_dir_all(log_dir.as_ref()).map_err(|e| format!("create log dir: {e}"))?;

    fs::write(plist_path, plist_content.as_bytes()).map_err(|e| format!("write plist: {e}"))?;

    let plist = plist_path.to_string_lossy().to_string();
    let output = run(&["bootstrap", domain.as_str(), plist.as_str()])?;

    interpret_bootstrap_result(
        output.success,
        output.stdout.as_str(),
        output.stderr.as_str(),
    )
}

/// Uninstall the launchd service: run `launchctl bootout`, remove plist.
pub fn uninstall() -> Result<String, String> {
    let plist_path = plist_path();
    uninstall_with_runner(&plist_path, launchctl_domain(), run_launchctl)
}

fn uninstall_with_runner(
    plist_path: impl AsRef<Path>,
    domain: String,
    mut run: impl FnMut(&[&str]) -> Result<LaunchctlCommandOutput, String>,
) -> Result<String, String> {
    let plist_path = plist_path.as_ref();
    let plist = plist_path.to_string_lossy().to_string();
    let output = run(&["bootout", domain.as_str(), plist.as_str()])?;

    let _ = interpret_bootout_result(
        output.success,
        output.stdout.as_str(),
        output.stderr.as_str(),
    )?;

    if plist_path.exists() {
        fs::remove_file(plist_path).map_err(|e| format!("remove plist: {e}"))?;
    }

    Ok(format!("Service '{SERVICE_LABEL}' uninstalled."))
}

/// Start the launchd service by ensuring its plist is installed and bootstrapped.
pub fn start() -> Result<String, String> {
    install()
}

/// Stop the launchd service without removing its plist.
pub fn stop() -> Result<String, String> {
    let plist_path = plist_path();
    let domain = launchctl_domain();
    let output = Command::new("launchctl")
        .args([
            "bootout",
            domain.as_str(),
            plist_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;

    interpret_bootout_result(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    )
}

/// Restart the launchd service, preserving the plist.  If the plist is missing,
/// restart behaves like start and installs it.
pub fn restart() -> Result<String, String> {
    if plist_path().exists() {
        stop()?;
    }
    start()
}

/// Check if the launchd service is currently running.
pub fn status() -> Result<String, String> {
    query_status().map(|status| status.status_message)
}

pub fn query_status() -> Result<LaunchdServiceStatus, String> {
    if !cfg!(target_os = "macos") {
        return Ok(unsupported_service_status());
    }

    let plist_path = plist_path();
    let installed = plist_path.exists();
    let service_target = format!("{}/{}", launchctl_domain(), SERVICE_LABEL);
    let output = Command::new("launchctl")
        .args(["print", service_target.as_str()])
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;

    Ok(service_status_from_parts(
        true,
        installed,
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    ))
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn run_launchctl(args: &[&str]) -> Result<LaunchctlCommandOutput, String> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;

    Ok(LaunchctlCommandOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn interpret_bootstrap_result(
    success: bool,
    _stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    if success {
        return Ok(format!(
            "Service '{SERVICE_LABEL}' installed and started via launchd."
        ));
    }

    if stderr.contains("already bootstrapped") || stderr.contains("service already loaded") {
        return Ok(format!(
            "Service '{SERVICE_LABEL}' already installed. Run `forge service restart` to reload."
        ));
    }

    Err(format!("launchctl bootstrap failed: {stderr}"))
}

fn interpret_bootout_result(success: bool, _stdout: &str, stderr: &str) -> Result<String, String> {
    if success {
        return Ok("Service stopped.".to_string());
    }

    if launchctl_reports_missing_service(stderr) {
        return Ok("Service is not running.".to_string());
    }

    Err(format!("launchctl bootout failed: {stderr}"))
}

fn interpret_print_result(success: bool, stdout: &str, stderr: &str) -> Result<String, String> {
    if success && stdout.contains("state = running") {
        return Ok(format!("Service '{SERVICE_LABEL}' is running."));
    }

    if launchctl_reports_missing_service(stderr) || launchctl_reports_missing_service(stdout) {
        return Ok(format!("Service '{SERVICE_LABEL}' is not installed."));
    }

    let detail = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    Ok(format!(
        "Service '{SERVICE_LABEL}' status unknown: {detail}"
    ))
}

fn service_status_from_parts(
    supported: bool,
    installed: bool,
    print_success: bool,
    stdout: &str,
    stderr: &str,
) -> LaunchdServiceStatus {
    if !supported {
        return unsupported_service_status();
    }

    let status_message = interpret_print_result(print_success, stdout, stderr)
        .unwrap_or_else(|error| format!("Gateway service status unavailable: {error}"));
    let running = installed && status_message.contains(" is running.");
    let message = match (installed, running) {
        (true, true) => "Gateway service is installed and running.".to_string(),
        (true, false) => "Gateway service is installed but not running.".to_string(),
        (false, _) => "Gateway service is not installed.".to_string(),
    };

    LaunchdServiceStatus {
        supported: true,
        installed,
        running,
        message,
        label: SERVICE_LABEL.to_string(),
        launch_domain: launchctl_domain(),
        plist_path: plist_path().display().to_string(),
        log_path: gateway_log_path().display().to_string(),
        error_log_path: gateway_error_log_path().display().to_string(),
        status_message,
    }
}

fn unsupported_service_status() -> LaunchdServiceStatus {
    LaunchdServiceStatus {
        supported: false,
        installed: false,
        running: false,
        message: "Service management is only supported on macOS.".to_string(),
        label: SERVICE_LABEL.to_string(),
        launch_domain: "unsupported".to_string(),
        plist_path: String::new(),
        log_path: String::new(),
        error_log_path: String::new(),
        status_message: "Service management is only supported on macOS.".to_string(),
    }
}

fn launchctl_reports_missing_service(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("not found")
        || lower.contains("no such process")
        || lower.contains("could not find service")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── plist generation ──────────────────────────────────────────────────

    #[test]
    fn generate_plist_contains_label() {
        let plist = generate_plist();
        assert!(plist.contains("<key>Label</key>"));
        assert!(plist.contains(SERVICE_LABEL));
    }

    #[test]
    fn generate_plist_contains_keep_alive_and_run_at_load() {
        let plist = generate_plist();
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<true/>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
    }

    #[test]
    fn generate_plist_contains_log_paths() {
        let plist = generate_plist();
        assert!(plist.contains("<key>StandardOutPath</key>"));
        assert!(plist.contains("<key>StandardErrorPath</key>"));
        assert!(plist.contains("gateway.log"));
        assert!(plist.contains("gateway-error.log"));
    }

    #[test]
    fn generate_plist_is_valid_xml() {
        let plist = generate_plist();
        // Basic XML validity check: starts with <?xml and ends with </plist>
        assert!(plist.starts_with("<?xml"));
        assert!(plist.ends_with("</plist>\n") || plist.ends_with("</plist>"));
    }

    // ── plist_path ───────────────────────────────────────────────────────

    #[test]
    fn plist_path_ends_with_label() {
        let path = plist_path();
        let display = path.to_string_lossy();
        assert!(display.ends_with("com.forge.gateway.plist"));
        assert!(display.contains("LaunchAgents"));
    }

    // ── gateway_binary_path ─────────────────────────────────────────────

    #[test]
    fn gateway_binary_path_env_override() {
        std::env::set_var("FORGE_GATEWAY_BIN", "/custom/path/gateway");
        let path = gateway_binary_path();
        assert_eq!(path, PathBuf::from("/custom/path/gateway"));
        std::env::remove_var("FORGE_GATEWAY_BIN");
    }

    // ── home_dir ────────────────────────────────────────────────────────

    #[test]
    fn home_dir_returns_something_reasonable() {
        let home = home_dir();
        assert!(!home.to_string_lossy().is_empty());
    }

    #[test]
    fn launchctl_domain_uses_gui_domain() {
        assert!(launchctl_domain().starts_with("gui/"));
    }

    #[test]
    fn service_management_api_exports_start_stop_restart() {
        let _: fn() -> Result<String, String> = start;
        let _: fn() -> Result<String, String> = stop;
        let _: fn() -> Result<String, String> = restart;
    }

    #[test]
    fn bootstrap_result_treats_already_loaded_as_installed() {
        let message = interpret_bootstrap_result(false, "", "service already loaded")
            .expect("already loaded");

        assert!(message.contains("already installed"));
    }

    #[test]
    fn bootout_result_treats_not_found_as_not_running() {
        let message = interpret_bootout_result(false, "", "service not found").expect("not found");

        assert_eq!(message, "Service is not running.");
    }

    #[test]
    fn print_result_detects_running_service() {
        let message = interpret_print_result(true, "state = running\n", "").expect("running");

        assert!(message.contains("is running"));
    }

    #[test]
    fn print_result_treats_launchctl_could_not_find_service_as_not_installed() {
        let message =
            interpret_print_result(false, "", "Could not find service \"com.forge.gateway\"")
                .expect("not installed");

        assert!(message.contains("not installed"));
    }

    #[test]
    fn service_status_from_parts_returns_structured_running_status() {
        let status = service_status_from_parts(true, true, true, "state = running\n", "");

        assert!(status.supported);
        assert!(status.installed);
        assert!(status.running);
        assert!(status.message.contains("installed and running"));
        assert_eq!(status.label, SERVICE_LABEL);
        assert!(status.plist_path.ends_with("com.forge.gateway.plist"));
        assert!(status.log_path.ends_with("gateway.log"));
        assert!(status.error_log_path.ends_with("gateway-error.log"));
    }

    #[test]
    fn service_status_from_parts_reports_installed_but_not_running() {
        let status = service_status_from_parts(true, true, false, "", "Could not find service");

        assert!(status.supported);
        assert!(status.installed);
        assert!(!status.running);
        assert!(status.message.contains("installed but not running"));
    }

    #[test]
    fn unsupported_service_status_keeps_paths_empty() {
        let status = unsupported_service_status();

        assert!(!status.supported);
        assert!(!status.installed);
        assert!(!status.running);
        assert_eq!(status.launch_domain, "unsupported");
        assert!(status.plist_path.is_empty());
    }

    #[test]
    fn install_with_runner_writes_plist_and_bootstraps() {
        let root = temp_root("launchd-install-runner");
        let plist = root.join("LaunchAgents").join("com.forge.gateway.plist");
        let log_dir = root.join(".forge").join("logs");
        let mut calls = Vec::new();

        let message = install_with_runner(
            &plist,
            &log_dir,
            "<plist>ok</plist>".to_string(),
            "gui/501".to_string(),
            |args| {
                calls.push(args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>());
                Ok(LaunchctlCommandOutput {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                })
            },
        )
        .expect("install with runner");

        assert!(message.contains("installed and started via launchd"));
        assert_eq!(
            std::fs::read_to_string(&plist).expect("plist"),
            "<plist>ok</plist>"
        );
        assert!(log_dir.exists());
        assert_eq!(
            calls,
            vec![vec![
                "bootstrap".to_string(),
                "gui/501".to_string(),
                plist.display().to_string(),
            ]]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn uninstall_with_runner_boots_out_and_removes_plist() {
        let root = temp_root("launchd-uninstall-runner");
        let plist = root.join("LaunchAgents").join("com.forge.gateway.plist");
        std::fs::create_dir_all(plist.parent().expect("plist parent")).expect("plist parent");
        std::fs::write(&plist, "<plist>ok</plist>").expect("plist");
        let mut calls = Vec::new();

        let message = uninstall_with_runner(&plist, "gui/501".to_string(), |args| {
            calls.push(args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>());
            Ok(LaunchctlCommandOutput {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        })
        .expect("uninstall with runner");

        assert!(message.contains("uninstalled"));
        assert!(!plist.exists());
        assert_eq!(
            calls,
            vec![vec![
                "bootout".to_string(),
                "gui/501".to_string(),
                plist.display().to_string(),
            ]]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("forge-{name}-{nanos}"))
    }
}
