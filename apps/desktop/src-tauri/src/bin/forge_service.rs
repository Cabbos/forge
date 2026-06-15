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
        "start" => forge::service::launchd::start(),
        "stop" => forge::service::launchd::stop(),
        "restart" => forge::service::launchd::restart(),
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
