#![allow(dead_code)]

pub mod adapters;
mod agent;
mod app_metadata;
mod consts;
pub mod continuity;
mod executor;
mod forge_wiki;
pub mod harness;
mod ipc;
mod logger;
mod memory;
mod parser;
mod process_runner;
mod protocol;
pub mod settings;
mod state;
mod transcript;
mod workflow;
mod workspace_safety;

use harness::Harness;
use state::AppState;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    logger::setup_panic_hook();
    logger::log(
        "INFO",
        &format!("App starting, log at {}", logger::log_path_str()),
    );

    // Detect project root: handle both "npm run tauri dev" (cwd = project root)
    // and "cargo run" (cwd = src-tauri/) cases by checking for Cargo.toml
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_root = if cwd.join("src-tauri").join("Cargo.toml").exists() {
        // cwd is the project root (npm run tauri dev)
        cwd
    } else if cwd.join("Cargo.toml").exists() && cwd.file_name().is_some_and(|n| n == "src-tauri") {
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
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|_app| {
            crate::app_log!("INFO", "DeepSeek Agent started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::handlers::create_session,
            ipc::mcp_context::list_mcp_context_sources,
            ipc::handlers::resume_session,
            ipc::handlers::send_input,
            ipc::session_lifecycle::kill_session,
            ipc::session_lifecycle::delete_session,
            ipc::session_lifecycle::list_sessions,
            app_metadata::load_app_metadata,
            app_metadata::save_app_metadata,
            ipc::confirmations::confirm_response,
            ipc::continuity_experiences::list_continuity_experiences,
            ipc::continuity_experiences::search_continuity_experiences,
            ipc::workspace_files::search_workspace_files,
            ipc::workspace_files::get_default_working_dir,
            ipc::settings_handlers::get_api_key_status,
            ipc::settings_handlers::set_api_key,
            ipc::capability_handlers::list_capabilities,
            ipc::capability_handlers::toggle_capability,
            ipc::capability_handlers::install_skill,
            ipc::workspace_files::open_file,
            ipc::workspace_files::preview_file,
            ipc::project_runtime::get_project_runtime_status,
            ipc::project_runtime::start_project_dev_server,
            ipc::project_runtime::stop_project_dev_server,
            ipc::project_runtime::open_project_preview,
            ipc::project_checkpoint::get_project_checkpoint_status,
            ipc::project_checkpoint::create_project_checkpoint,
            ipc::project_checkpoint::restore_project_checkpoint,
            ipc::memory_handlers::list_memories,
            ipc::memory_handlers::update_memory,
            ipc::memory_handlers::forget_memory,
            ipc::memory_handlers::pin_memory,
            ipc::memory_handlers::select_context_memories,
            ipc::workflow_handlers::get_workflow_state,
            ipc::workflow_handlers::override_workflow_route,
            ipc::forge_wiki_handlers::get_forge_wiki_state,
            ipc::forge_wiki_handlers::init_forge_wiki,
            ipc::forge_wiki_handlers::list_forge_wiki_pages,
            ipc::forge_wiki_handlers::read_forge_wiki_page,
            ipc::forge_wiki_handlers::select_forge_wiki_context,
            ipc::forge_wiki_handlers::create_forge_wiki_update_proposal,
            ipc::forge_wiki_handlers::accept_forge_wiki_update_proposal,
            ipc::forge_wiki_handlers::discard_forge_wiki_update_proposal,
            transcript::load_session_transcript,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
