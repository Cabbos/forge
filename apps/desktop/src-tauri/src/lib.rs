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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    logger::setup_panic_hook();
    logger::log("INFO", &format!("App starting, log at {}", logger::log_path_str()));

    // Detect project root: handle both "npm run tauri dev" (cwd = project root)
    // and "cargo run" (cwd = src-tauri/) cases by checking for Cargo.toml
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_root = if cwd.join("src-tauri").join("Cargo.toml").exists() {
        // cwd is the project root (npm run tauri dev)
        cwd
    } else if cwd.join("Cargo.toml").exists() && cwd.file_name().map_or(false, |n| n == "src-tauri") {
        // cwd is src-tauri/ (cargo run), use parent
        cwd.parent().map(|p| p.to_path_buf()).unwrap_or(cwd)
    } else {
        cwd
    };
    crate::app_log!("INFO", "Working directory: {}", project_root.display());

    // Create harness with default working directory
    let harness = Arc::new(Harness::new(project_root));
    let app_state = Arc::new(AppState::new(harness));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|_app| {
            crate::app_log!("INFO", "DeepSeek Agent started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::handlers::create_session,
            ipc::handlers::send_input,
            ipc::handlers::kill_session,
            ipc::handlers::list_sessions,
            ipc::handlers::confirm_response,
            ipc::handlers::search_workspace_files,
            ipc::handlers::get_default_working_dir,
            ipc::handlers::get_api_key_status,
            ipc::handlers::set_api_key,
            ipc::capability_handlers::list_capabilities,
            ipc::capability_handlers::toggle_capability,
            ipc::capability_handlers::install_skill,
            ipc::handlers::open_file,
            ipc::handlers::preview_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
