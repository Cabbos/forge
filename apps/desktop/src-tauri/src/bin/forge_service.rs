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

    let result = forge::service::run_service_command(&args[1]);

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
