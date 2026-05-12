use std::sync::Arc;
use tauri::Emitter;

use crate::adapters::anthropic::AnthropicAdapter;
use crate::adapters::base::AiAdapter;
use crate::agent::session::AgentSession;
use crate::protocol::commands::SessionCreated;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use crate::state::AppState;
use crate::settings;

/// DeepSeek Anthropic-compatible API (recommended by DeepSeek docs)
const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/anthropic";
const DEFAULT_MODEL: &str = "deepseek-v4-flash[1m]";

fn build_adapter(api_key: &str, model: Option<&str>) -> Result<Arc<Box<dyn AiAdapter>>, String> {
    let model = model.unwrap_or(DEFAULT_MODEL);
    let a = AnthropicAdapter::new(api_key.to_string())
        .map_err(|e| format!("API key error: {e}"))?
        .with_base_url(DEEPSEEK_BASE_URL)
        .with_model(model)
        .with_max_tokens(384_000)
        .with_thinking_budget_tokens(16_000);
    Ok(Arc::new(Box::new(a) as Box<dyn AiAdapter>))
}

#[tauri::command]
pub async fn create_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    working_dir: String,
    api_key: String,
    model: Option<String>,
) -> Result<SessionCreated, String> {
    let session_id = uuid::Uuid::now_v7().to_string();

    let key = if api_key.is_empty() {
        settings::Settings::load().get_api_key("deepseek").unwrap_or("").to_string()
    } else {
        api_key
    };
    if key.is_empty() {
        return Err("No DeepSeek API key configured. Open Settings (Cmd+,) to set one.".into());
    }

    let adapter = build_adapter(&key, model.as_deref())?;
    let model_str = model.unwrap_or(DEFAULT_MODEL.to_string());

    // Build system prompt from harness (active skills + project CLAUDE.md)
    let wd = std::path::Path::new(&working_dir);
    let system_prompt = state.harness.build_system_prompt("deepseek", wd).await;

    let harness = state.harness.clone();
    let session = AgentSession::new(
        session_id.clone(),
        "deepseek".to_string(),
        adapter,
        harness,
        system_prompt,
    );

    let _ = app_handle.emit("session-output",
        StreamEvent::SessionStarted {
            session_id: session_id.clone(),
            agent_type: "deepseek".to_string(),
            model: model_str,
        },
    );

    // Emit active skills as a visible info block in the conversation
    let skills = state.harness.skill_loader.enabled_skills().await;
    if !skills.is_empty() {
        let names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
        let info = format!("Active Skills: {}", names.join(", "));
        let bid = BlockId::new().to_string();
        let _ = app_handle.emit("session-output", StreamEvent::TextStart { session_id: session_id.clone(), block_id: bid.clone() });
        let _ = app_handle.emit("session-output", StreamEvent::TextChunk { session_id: session_id.clone(), block_id: bid.clone(), content: info });
        let _ = app_handle.emit("session-output", StreamEvent::TextEnd { session_id: session_id.clone(), block_id: bid });
    }

    state.sessions.write().await.insert(session_id.clone(), Arc::new(session));
    Ok(SessionCreated { session_id })
}

#[tauri::command]
pub async fn send_input(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    session_id: String,
    text: String,
) -> Result<(), String> {
    let session = state.sessions.read().await.get(&session_id).cloned();
    match session {
        Some(s) => s.send_message(&text, &app_handle).await,
        None => Err(format!("Session not found: {session_id}")),
    }
}

#[tauri::command]
pub async fn kill_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    if let Some(s) = state.sessions.read().await.get(&session_id).cloned() {
        s.kill(&app_handle);
    }
    state.sessions.write().await.remove(&session_id);
    Ok(())
}

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<crate::protocol::commands::SessionInfo>, String> {
    let sessions = state.sessions.read().await;
    let result: Vec<_> = sessions.iter().map(|(id, s)| {
        let status = s.status.lock().unwrap();
        crate::protocol::commands::SessionInfo {
            id: id.clone(),
            provider: "deepseek".into(),
            model: s.model.clone(),
            status: status.as_str().to_string(),
            created_at: String::new(),
        }
    }).collect();
    Ok(result)
}

#[tauri::command]
pub async fn confirm_response(
    state: tauri::State<'_, Arc<AppState>>,
    block_id: String,
    approved: bool,
) -> Result<(), String> {
    let sender = { state.pending_confirms.write().await.remove(&block_id) };
    match sender {
        Some(tx) => { let _ = tx.send(approved); Ok(()) }
        None => Err(format!("No pending confirm for: {block_id}")),
    }
}

#[tauri::command]
pub async fn search_workspace_files(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
) -> Result<Vec<String>, String> {
    // Search the harness working dir for files matching the query
    let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let results = find_files(&dir, &query, 20);
    Ok(results)
}

fn find_files(dir: &std::path::Path, query: &str, limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    let lower_query = query.to_lowercase();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if results.len() >= limit { break; }
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            // Skip hidden, node_modules, target
            if name.starts_with('.') || name == "node_modules" || name == "target" || name == "dist" { continue; }
            let rel = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().to_string();
            if name.to_lowercase().contains(&lower_query) || rel.to_lowercase().contains(&lower_query) {
                if path.is_dir() {
                    results.push(format!("{}/", rel));
                    // Also search one level deep
                    results.extend(find_files(&path, query, limit - results.len()).into_iter().map(|f| format!("{}/{}", rel, f)));
                } else {
                    results.push(rel);
                }
            }
            if results.len() >= limit { break; }
        }
    }
    results.truncate(limit);
    results
}

#[tauri::command]
pub async fn get_api_key_status() -> Result<Vec<settings::KeyStatus>, String> {
    Ok(settings::Settings::load().key_status())
}

#[tauri::command]
pub async fn set_api_key(provider: String, key: String) -> Result<(), String> {
    settings::Settings::load().set_api_key(&provider, &key)
}
