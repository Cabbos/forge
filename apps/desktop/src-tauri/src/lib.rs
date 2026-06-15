#![allow(dead_code)]

pub mod adapters;
mod agent;
mod app_metadata;
mod autosave;
mod consts;
pub mod continuity;
pub mod diagnostics;
pub mod eval_headless;
mod executor;
mod forge_wiki;
pub mod gateway;
pub mod harness;
mod ipc;
mod log_store;
mod logger;
mod memory;
mod parser;
mod process_runner;
mod profile;
mod protocol;
pub mod scheduler;
pub mod service;
pub mod settings;
mod state;
mod transcript;
mod workflow;
mod workspace_safety;

use harness::Harness;
use state::AppState;
use std::sync::Arc;
use tauri::Manager;

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
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state: Arc<AppState> = handle.state::<Arc<AppState>>().inner().clone();
                ipc::session_lifecycle::startup_restore_active_session(&state, &handle).await;
            });
            // Phase 2.4: spawn the session watchdog background task.
            diagnostics::watchdog::spawn_session_watchdog(app.handle().clone());
            diagnostics::watchdog::spawn_gateway_watchdog(app.handle().clone());
            crate::app_log!("INFO", "DeepSeek Agent started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::handlers::create_session,
            ipc::a2a_handlers::get_agent_a2a_state,
            ipc::a2a_handlers::list_agent_a2a_states,
            ipc::mcp_context::list_mcp_context_sources,
            ipc::handlers::resume_session,
            ipc::handlers::send_input,
            ipc::handlers::compact_session_context,
            ipc::session_lifecycle::kill_session,
            ipc::session_lifecycle::delete_session,
            ipc::session_lifecycle::list_sessions,
            app_metadata::load_app_metadata,
            app_metadata::save_app_metadata,
            ipc::confirmations::confirm_response,
            ipc::continuity_experiences::list_continuity_experiences,
            ipc::continuity_experiences::search_continuity_experiences,
            ipc::continuity_experiences::update_continuity_experience_status,
            ipc::workspace_files::search_workspace_files,
            ipc::workspace_files::get_default_working_dir,
            ipc::settings_handlers::get_api_key_status,
            ipc::settings_handlers::set_api_key,
            ipc::capability_handlers::list_capabilities,
            ipc::capability_handlers::toggle_capability,
            ipc::capability_handlers::install_skill,
            ipc::capability_handlers::list_ecosystem_items,
            ipc::capability_handlers::set_ecosystem_enabled,
            ipc::capability_handlers::get_tool_inventory,
            ipc::capability_handlers::configure_ecosystem_item,
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
            ipc::profile_handlers::list_profiles,
            ipc::profile_handlers::upsert_profile,
            ipc::profile_handlers::delete_profile,
            ipc::profile_handlers::set_active_profile,
            ipc::memory_handlers::list_memory_facts,
            ipc::memory_handlers::upsert_memory_fact,
            ipc::memory_handlers::delete_memory_fact,
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
            ipc::diagnostics_handlers::get_diagnostics_report,
            ipc::diagnostics_handlers::get_gateway_runtime_status,
            ipc::diagnostics_handlers::enqueue_gateway_trigger,
            ipc::diagnostics_handlers::list_gateway_triggers,
            ipc::diagnostics_handlers::cancel_gateway_trigger,
            ipc::diagnostics_handlers::replay_gateway_trigger_run,
            ipc::diagnostics_handlers::get_gateway_trigger_run,
            ipc::diagnostics_handlers::get_recent_logs,
            ipc::diagnostics_handlers::run_repair_action,
            ipc::diagnostics_handlers::list_repair_actions,
            ipc::scheduler_handlers::list_scheduled_tasks,
            ipc::scheduler_handlers::upsert_scheduled_task,
            ipc::scheduler_handlers::delete_scheduled_task,
            ipc::scheduler_handlers::set_scheduled_task_enabled,
            ipc::scheduler_handlers::run_scheduled_task_now,
            ipc::service_handlers::get_service_status,
            ipc::service_handlers::set_autostart,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                crate::autosave::flush_all_sessions(app_handle);
                if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                    let state = state.inner().clone();
                    tauri::async_runtime::block_on(async move {
                        crate::ipc::session_lifecycle::unregister_all_gateway_sessions_best_effort(
                            &state,
                        )
                        .await;
                    });
                }
            }
        });
}
