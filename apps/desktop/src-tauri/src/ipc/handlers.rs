use std::sync::Arc;

use tauri::Emitter;
use crate::adapters::anthropic::AnthropicAdapter;
use crate::agent::session::AgentSession;
use crate::executor::ToolExecutor;
use crate::protocol::commands::{AgentType, SessionCreated, ToolType};
use crate::pty::session::CliSession;
use crate::state::{AppState, Session};
use crate::settings;

#[tauri::command]
pub async fn create_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    tool_type: String,
    working_dir: String,
    tool_path: Option<String>,
    _model: Option<String>,
) -> Result<SessionCreated, String> {
    let tool: ToolType = tool_type
        .parse()
        .map_err(|e| format!("Invalid tool type: {}", e))?;

    let session_id = uuid::Uuid::now_v7().to_string();

    match tool {
        // Bash: PTY session
        ToolType::Bash => {
            let session = CliSession::spawn(
                session_id.clone(),
                tool,
                &working_dir,
                tool_path.as_deref(),
                app_handle,
            )?;
            state.sessions.write().await
                .insert(session_id.clone(), Session::Cli(Arc::new(session)));
        }
        // Claude/Codex/Hermes: API-based agent sessions
        ToolType::Claude | ToolType::Codex | ToolType::Hermes => {
            let agent_type = match tool {
                ToolType::Claude => AgentType::Claude,
                ToolType::Codex => AgentType::Codex,
                ToolType::Hermes => AgentType::Hermes,
                _ => unreachable!(),
            };

            let creds = settings::detect_credentials("anthropic");
            let mut adapter = AnthropicAdapter::new(creds.api_key.clone())
                .map_err(|e| format!("API key error: {}. Set ANTHROPIC_API_KEY or configure in Claude Code settings.", e))?;
            if let Some(base) = creds.api_base {
                adapter = adapter.with_base_url(&base);
            }
            if let Some(ref m) = creds.model {
                adapter = adapter.with_model(m);
            }

            let executor = ToolExecutor::new(
                std::path::PathBuf::from(&working_dir),
                state.pending_confirms.clone(),
            );

            let agent_type_str = format!("{:?}", agent_type).to_lowercase();
            let session = AgentSession::new(
                session_id.clone(),
                agent_type.clone(),
                Box::new(adapter),
                Some(executor),
                &app_handle,
            );

            let model = session.model.clone();
            state.sessions.write().await
                .insert(session_id.clone(), Session::Agent(Arc::new(session)));
            let _ = app_handle.emit("session-output",
                crate::protocol::events::StreamEvent::SessionStarted {
                    session_id: session_id.clone(),
                    agent_type: agent_type_str,
                    model,
                },
            );
        }
    }

    Ok(SessionCreated { session_id })
}

#[tauri::command]
pub async fn send_input(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    session_id: String,
    text: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };

    match session {
        Some(Session::Cli(s)) => {
            let input = if text.ends_with('\n') { text } else { text + "\n" };
            s.write_input(&input)?;
            Ok(())
        }
        Some(Session::Agent(s)) => {
            s.send_message(&text, &app_handle).await
        }
        None => Err(format!("Session not found: {}", session_id)),
    }
}

#[tauri::command]
pub async fn send_signal(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    signal: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };
    match session {
        Some(Session::Cli(s)) => s.send_signal(&signal),
        Some(Session::Agent(_)) => Err("Signals not supported for agent sessions".to_string()),
        None => Err(format!("Session not found: {}", session_id)),
    }
}

#[tauri::command]
pub async fn resize_session(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };
    match session {
        Some(Session::Cli(s)) => s.resize(cols, rows),
        Some(Session::Agent(_)) => Err("Resize not supported for agent sessions".to_string()),
        None => Err(format!("Session not found: {}", session_id)),
    }
}

#[tauri::command]
pub async fn kill_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };
    match session {
        Some(Session::Cli(s)) => {
            s.kill(&app_handle)?;
        }
        Some(Session::Agent(s)) => {
            s.kill(&app_handle);
        }
        None => return Err(format!("Session not found: {}", session_id)),
    }
    state.sessions.write().await.remove(&session_id);
    Ok(())
}

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<crate::protocol::commands::SessionInfo>, String> {
    let sessions = state.sessions.read().await;
    let mut result = Vec::new();
    for (id, s) in sessions.iter() {
        let (tool_type, status) = match s {
            Session::Cli(cli) => {
                let st = cli.status.lock().unwrap();
                (format!("{:?}", cli.tool_type).to_lowercase(), format!("{:?}", *st).to_lowercase())
            }
            Session::Agent(agent) => {
                let st = agent.status.lock().unwrap();
                (format!("{:?}", agent.agent_type).to_lowercase(), format!("{:?}", *st).to_lowercase())
            }
        };
        result.push(crate::protocol::commands::SessionInfo {
            id: id.clone(), tool_type, status, created_at: String::new(),
        });
    }
    Ok(result)
}

// Plugin/persistence commands kept but simplified

#[tauri::command]
pub async fn list_plugins(agent: String) -> Result<Vec<crate::plugin_manager::PluginEntry>, String> {
    let agent: crate::plugin_manager::AgentTarget = match agent.to_lowercase().as_str() {
        "claude" => crate::plugin_manager::AgentTarget::Claude,
        "codex" => crate::plugin_manager::AgentTarget::Codex,
        "hermes" => crate::plugin_manager::AgentTarget::Hermes,
        _ => return Err(format!("Unknown agent: {}", agent)),
    };
    Ok(crate::plugin_manager::scanner::PluginScanner::scan(&agent))
}

#[tauri::command]
pub async fn discover_plugins(agent: String) -> Result<Vec<crate::plugin_manager::PluginEntry>, String> {
    let agent: crate::plugin_manager::AgentTarget = match agent.to_lowercase().as_str() {
        "claude" => crate::plugin_manager::AgentTarget::Claude,
        "codex" => crate::plugin_manager::AgentTarget::Codex,
        "hermes" => crate::plugin_manager::AgentTarget::Hermes,
        _ => return Err(format!("Unknown agent: {}", agent)),
    };
    Ok(crate::plugin_manager::registry::PluginRegistry::discover(&agent))
}

#[tauri::command]
pub async fn install_plugin(plugin_id: String, agent: String, config: Option<serde_json::Value>) -> Result<(), String> {
    let _agent_target = match agent.to_lowercase().as_str() {
        "claude" => crate::plugin_manager::AgentTarget::Claude,
        "codex" => crate::plugin_manager::AgentTarget::Codex,
        "hermes" => crate::plugin_manager::AgentTarget::Hermes,
        _ => return Err(format!("Unknown agent: {}", agent)),
    };
    let mut plugin = crate::plugin_manager::registry::PluginRegistry::get_preset(&plugin_id)
        .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;
    if let Some(cfg) = config { plugin.current_config = Some(cfg); }
    crate::plugin_manager::installer::PluginInstaller::install(&plugin)
}

#[tauri::command]
pub async fn uninstall_plugin(plugin_id: String, agent: String) -> Result<(), String> {
    let agent_target = match agent.to_lowercase().as_str() {
        "claude" => crate::plugin_manager::AgentTarget::Claude,
        "codex" => crate::plugin_manager::AgentTarget::Codex,
        "hermes" => crate::plugin_manager::AgentTarget::Hermes,
        _ => return Err(format!("Unknown agent: {}", agent)),
    };
    let plugin = crate::plugin_manager::PluginEntry {
        id: plugin_id.clone(), name: plugin_id.clone(), description: String::new(),
        plugin_type: crate::plugin_manager::PluginType::McpServer, agent: agent_target,
        category: String::new(), status: crate::plugin_manager::PluginStatus::Installed { enabled: true },
        config_schema: None, current_config: None, homepage: None, author: None,
    };
    crate::plugin_manager::installer::PluginInstaller::uninstall(&plugin)
}

#[tauri::command]
pub async fn toggle_plugin(plugin_id: String, agent: String, enabled: bool) -> Result<(), String> {
    let agent_target = match agent.to_lowercase().as_str() {
        "claude" => crate::plugin_manager::AgentTarget::Claude,
        "codex" => crate::plugin_manager::AgentTarget::Codex,
        "hermes" => crate::plugin_manager::AgentTarget::Hermes,
        _ => return Err(format!("Unknown agent: {}", agent)),
    };
    crate::plugin_manager::installer::PluginInstaller::toggle(&plugin_id, &agent_target, enabled)
}

#[tauri::command]
pub async fn get_api_key_status() -> Result<Vec<settings::KeyStatus>, String> {
    let mut status = Vec::new();
    for provider in &["anthropic", "openai"] {
        let creds = settings::detect_credentials(provider);
        let set = !creds.api_key.is_empty();
        status.push(settings::KeyStatus {
            provider: provider.to_string(), set,
            preview: if set { settings::mask_key(&creds.api_key) } else { String::new() },
        });
    }
    Ok(status)
}

#[tauri::command]
pub async fn set_api_key(provider: String, key: String) -> Result<(), String> {
    settings::Settings::load().set_api_key(&provider, &key)
}

#[tauri::command]
pub async fn confirm_response(
    state: tauri::State<'_, Arc<AppState>>,
    block_id: String,
    approved: bool,
) -> Result<(), String> {
    let sender = {
        let mut confirms = state.pending_confirms.write().await;
        confirms.remove(&block_id)
    };
    match sender {
        Some(tx) => { let _ = tx.send(approved); Ok(()) }
        None => Err(format!("No pending confirm for: {}", block_id)),
    }
}
