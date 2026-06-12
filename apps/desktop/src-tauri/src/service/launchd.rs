//! macOS launchd integration — plist generation, install, uninstall, and
//! status checks for the Forge Gateway background service.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Label used in the launchd plist and `launchctl` commands.
pub const SERVICE_LABEL: &str = "com.forge.gateway";

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

    // Ensure parent dir exists.
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }

    // Ensure log dir exists.
    let log_dir = gateway_log_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| home_dir().join(".forge").join("logs"));
    fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {e}"))?;

    // Write plist.
    let plist_content = generate_plist();
    fs::write(&plist_path, plist_content.as_bytes()).map_err(|e| format!("write plist: {e}"))?;

    // Bootstrap with launchctl.
    let domain = launchctl_domain();
    let output = Command::new("launchctl")
        .args([
            "bootstrap",
            domain.as_str(),
            plist_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;

    if output.status.success() {
        Ok(format!(
            "Service '{SERVICE_LABEL}' installed and started via launchd."
        ))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Bootstrap may fail with "already bootstrapped" — that's OK.
        if stderr.contains("already bootstrapped") || stderr.contains("service already loaded") {
            Ok(format!(
                "Service '{SERVICE_LABEL}' already installed. Run `forge service restart` to reload."
            ))
        } else {
            Err(format!("launchctl bootstrap failed: {stderr}"))
        }
    }
}

/// Uninstall the launchd service: run `launchctl bootout`, remove plist.
pub fn uninstall() -> Result<String, String> {
    let plist_path = plist_path();

    // Bootout.
    let domain = launchctl_domain();
    let output = Command::new("launchctl")
        .args([
            "bootout",
            domain.as_str(),
            plist_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "not found" means already uninstalled — fine.
        if !stderr.contains("not found") && !stderr.contains("No such process") {
            return Err(format!("launchctl bootout failed: {stderr}"));
        }
    }

    // Remove plist.
    if plist_path.exists() {
        fs::remove_file(&plist_path).map_err(|e| format!("remove plist: {e}"))?;
    }

    Ok(format!("Service '{SERVICE_LABEL}' uninstalled."))
}

/// Check if the launchd service is currently running.
pub fn status() -> Result<String, String> {
    let service_target = format!("{}/{}", launchctl_domain(), SERVICE_LABEL);
    let output = Command::new("launchctl")
        .args(["print", service_target.as_str()])
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() && stdout.contains("state = running") {
        Ok(format!("Service '{SERVICE_LABEL}' is running."))
    } else if stderr.contains("not found") || stdout.contains("not found") {
        Ok(format!("Service '{SERVICE_LABEL}' is not installed."))
    } else {
        Ok(format!(
            "Service '{SERVICE_LABEL}' status unknown: {stdout}"
        ))
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

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
}
