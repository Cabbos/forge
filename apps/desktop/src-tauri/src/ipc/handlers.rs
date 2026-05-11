use std::sync::Arc;
use tauri::Emitter;

use crate::adapters::openai_compatible::OpenAiCompatibleAdapter;
use crate::adapters::base::AiAdapter;
use crate::agent::session::AgentSession;
use crate::protocol::commands::SessionCreated;
use crate::state::AppState;
use crate::settings;

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";
const DEFAULT_MODEL: &str = "deepseek-v4-flash";

fn build_adapter(api_key: &str, model: Option<&str>) -> Result<Box<dyn AiAdapter>, String> {
    let mut a = OpenAiCompatibleAdapter::new(api_key.to_string())
        .map_err(|e| format!("API key error: {e}"))?;
    a = a.with_base_url(DEEPSEEK_BASE_URL);
    a = a.with_model(model.unwrap_or(DEFAULT_MODEL));
    Ok(Box::new(a))
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

    // Build system prompt from harness (active skills merged in)
    let system_prompt = state.harness.build_system_prompt("deepseek").await;

    let harness = state.harness.clone();
    let session = AgentSession::new(
        session_id.clone(),
        "deepseek".to_string(),
        adapter,
        harness,
        system_prompt,
    );

    let _ = app_handle.emit("session-output",
        crate::protocol::events::StreamEvent::SessionStarted {
            session_id: session_id.clone(),
            agent_type: "deepseek".to_string(),
            model: model_str,
        },
    );

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
pub async fn get_api_key_status() -> Result<Vec<settings::KeyStatus>, String> {
    Ok(settings::Settings::load().key_status())
}

#[tauri::command]
pub async fn set_api_key(provider: String, key: String) -> Result<(), String> {
    settings::Settings::load().set_api_key(&provider, &key)
}
