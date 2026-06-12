//! Forge Service Manager — install, uninstall, start, stop, restart, status.
//!
//! Usage: `forge_service <install|uninstall|start|stop|restart|status>`

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: forge_service <install|uninstall|start|stop|restart|status>");
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "install" => forge::service::launchd::install(),
        "uninstall" => forge::service::launchd::uninstall(),
        "status" => forge::service::launchd::status(),
        "start" => {
            // launchd services with RunAtLoad+KeepAlive are always running
            // after bootstrap.  "start" = ensure installed + bootstrapped.
            forge::service::launchd::install()
        }
        "stop" => {
            // Bootout stops the service without removing the plist.
            let plist_path = forge::service::launchd::plist_path();
            let domain = forge::service::launchd::launchctl_domain();
            let output = process::Command::new("launchctl")
                .args([
                    "bootout",
                    domain.as_str(),
                    plist_path.to_str().unwrap_or(""),
                ])
                .output();
            match output {
                Ok(o) if o.status.success() => Ok("Service stopped.".to_string()),
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if stderr.contains("not found") {
                        Ok("Service is not running.".to_string())
                    } else {
                        Err(format!("stop failed: {stderr}"))
                    }
                }
                Err(e) => Err(format!("launchctl: {e}")),
            }
        }
        "restart" => {
            // Stop then start.
            let plist_path = forge::service::launchd::plist_path();
            let domain = forge::service::launchd::launchctl_domain();
            let _ = process::Command::new("launchctl")
                .args([
                    "bootout",
                    domain.as_str(),
                    plist_path.to_str().unwrap_or(""),
                ])
                .output();
            forge::service::launchd::install()
        }
        cmd => Err(format!("Unknown service command: {cmd}")),
    };

    match result {
        Ok(msg) => {
            println!("{msg}");
        }
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    }
}
