#![allow(dead_code)]

pub mod adapters;
mod agent;
mod executor;
pub mod harness;
mod ipc;
mod logger;
mod parser;
mod protocol;
pub mod settings;
mod state;

use harness::Harness;
use state::AppState;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    logger::setup_panic_hook();
    logger::log("INFO", &format!("App starting, log at {}", logger::log_path_str()));

    // Create harness with default working directory
    let harness = Arc::new(Harness::new(
        std::path::PathBuf::from(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        )
    ));
    let app_state = Arc::new(AppState::new(harness));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|_app| {
            log::info!("DeepSeek Agent started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::handlers::create_session,
            ipc::handlers::send_input,
            ipc::handlers::kill_session,
            ipc::handlers::list_sessions,
            ipc::handlers::confirm_response,
            ipc::handlers::get_api_key_status,
            ipc::handlers::set_api_key,
            ipc::capability_handlers::list_capabilities,
            ipc::capability_handlers::toggle_capability,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
