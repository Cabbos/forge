#![allow(dead_code)]

pub mod adapters;
mod agent;
mod executor;
mod ipc;
mod logger;
mod mcp_runtime;
mod parser;
mod plugin_manager;
mod protocol;
mod pty;
pub mod settings;
mod state;

use state::AppState;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    logger::setup_panic_hook();
    logger::log("INFO", &format!("App starting, log at {}", logger::log_path_str()));

    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|_app| {
            log::info!("TUI-to-GUI application started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::handlers::create_session,
            ipc::handlers::send_input,
            ipc::handlers::send_signal,
            ipc::handlers::resize_session,
            ipc::handlers::kill_session,
            ipc::handlers::list_sessions,
            ipc::handlers::list_plugins,
            ipc::handlers::discover_plugins,
            ipc::handlers::install_plugin,
            ipc::handlers::uninstall_plugin,
            ipc::handlers::toggle_plugin,
            ipc::handlers::confirm_response,
            ipc::handlers::get_api_key_status,
            ipc::handlers::set_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
