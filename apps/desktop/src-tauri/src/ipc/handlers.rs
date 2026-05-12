use std::sync::Arc;
use tauri::Emitter;

use crate::adapters::anthropic::AnthropicAdapter;
use crate::adapters::base::AiAdapter;
use crate::agent::session::AgentSession;
use crate::harness::Harness;
use crate::protocol::commands::SessionCreated;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use crate::state::AppState;
use crate::settings;

#[derive(serde::Serialize)]
pub struct FilePreviewLine {
    number: usize,
    content: String,
    is_target: bool,
}

#[derive(serde::Serialize)]
pub struct FilePreview {
    path: String,
    display_path: String,
    requested_line: Option<u32>,
    start_line: usize,
    total_lines: usize,
    lines: Vec<FilePreviewLine>,
}

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

    let working_dir = resolve_working_dir(&working_dir)?;
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.clone(),
        state.pending_confirms.clone(),
    ));

    // Build system prompt from harness (active skills + project CLAUDE.md)
    let system_prompt = harness.build_system_prompt("deepseek", &working_dir).await;

    let session = AgentSession::new(
        session_id.clone(),
        "deepseek".to_string(),
        adapter,
        harness.clone(),
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
    let skills = harness.skill_loader.enabled_skills().await;
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

fn resolve_working_dir(working_dir: &str) -> Result<std::path::PathBuf, String> {
    let requested = working_dir.trim();
    if requested.is_empty() {
        return std::env::current_dir().map_err(|e| format!("Cannot read current directory: {}", e));
    }

    let path = std::path::PathBuf::from(requested);
    let resolved = path
        .canonicalize()
        .map_err(|e| format!("Cannot open project folder '{}': {}", requested, e))?;
    if !resolved.is_dir() {
        return Err(format!("Project folder is not a directory: {}", resolved.display()));
    }
    Ok(resolved)
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
    let results = find_files(&state.harness.working_dir, &query, 20);
    Ok(results)
}

#[tauri::command]
pub async fn get_default_working_dir(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<String, String> {
    Ok(state.harness.working_dir.to_string_lossy().to_string())
}

/// Preview a small slice of a file around a target line inside the app.
#[tauri::command]
pub async fn preview_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    line: Option<u32>,
    context: Option<u32>,
    session_id: Option<String>,
) -> Result<FilePreview, String> {
    let working_dir = working_dir_for_request(&state, session_id.as_deref()).await;
    let full_path = resolve_workspace_path(&working_dir, &path);

    crate::app_log!(
        "INFO",
        "[preview_file] request path={} line={:?} resolved={}",
        path,
        line,
        full_path.display()
    );

    if !full_path.exists() {
        return Err(format!("File not found: {}", full_path.display()));
    }
    if !full_path.is_file() {
        return Err(format!("Not a file: {}", full_path.display()));
    }

    let metadata = std::fs::metadata(&full_path)
        .map_err(|e| format!("Unable to read file metadata: {}", e))?;
    if metadata.len() > 2_000_000 {
        return Err("File is too large to preview safely.".into());
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|_| "This file is not plain text, so it cannot be previewed here.".to_string())?;

    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len().max(1);
    let target_line = line.map(|l| l.max(1) as usize);
    let context = context.unwrap_or(40).clamp(5, 160) as usize;

    let (start_line, end_line) = if let Some(target) = target_line {
        let target = target.min(total_lines);
        (
            target.saturating_sub(context).max(1),
            (target + context).min(total_lines),
        )
    } else {
        (1, (context * 2).min(total_lines))
    };

    let lines = (start_line..=end_line)
        .map(|number| FilePreviewLine {
            number,
            content: all_lines.get(number - 1).copied().unwrap_or("").to_string(),
            is_target: target_line.map(|target| target == number).unwrap_or(false),
        })
        .collect::<Vec<_>>();

    let display_path = full_path
        .strip_prefix(&working_dir)
        .unwrap_or(&full_path)
        .to_string_lossy()
        .to_string();

    Ok(FilePreview {
        path: full_path.to_string_lossy().to_string(),
        display_path,
        requested_line: line,
        start_line,
        total_lines,
        lines,
    })
}

/// Open a file in the system's default editor at a specific line.
#[tauri::command]
pub async fn open_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    line: Option<u32>,
    session_id: Option<String>,
) -> Result<(), String> {
    let working_dir = working_dir_for_request(&state, session_id.as_deref()).await;
    let full_path = resolve_workspace_path(&working_dir, &path);

    crate::app_log!(
        "INFO",
        "[open_file] request path={} line={:?} resolved={}",
        path,
        line,
        full_path.display()
    );

    if !full_path.exists() {
        let message = format!("File not found: {}", full_path.display());
        crate::app_log!("WARN", "[open_file] {}", message);
        return Err(message);
    }

    let path_str = full_path.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        open_file_macos(&path_str, line)?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path_str, line);
        return Err("open_file is only supported on macOS currently".into());
    }

    Ok(())
}

async fn working_dir_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> std::path::PathBuf {
    if let Some(session_id) = session_id {
        if let Some(session) = state.sessions.read().await.get(session_id).cloned() {
            return session.harness.working_dir.clone();
        }
    }
    state.harness.working_dir.clone()
}

fn resolve_workspace_path(working_dir: &std::path::Path, path: &str) -> std::path::PathBuf {
    let requested_path = path.trim();
    if let Some(src_path) = requested_path.strip_prefix("@/") {
        working_dir.join("src").join(src_path)
    } else if std::path::Path::new(requested_path).is_absolute() {
        std::path::PathBuf::from(requested_path)
    } else {
        working_dir.join(requested_path)
    }
}

#[cfg(target_os = "macos")]
fn open_file_macos(path_str: &str, line: Option<u32>) -> Result<(), String> {
    let location = if let Some(l) = line {
        format!("{}:{}", path_str, l)
    } else {
        path_str.to_string()
    };

    let mut attempts: Vec<(String, Vec<String>)> = Vec::new();

    for cli in [
        "code",
        "/usr/local/bin/code",
        "/opt/homebrew/bin/code",
        "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
        "cursor",
        "/usr/local/bin/cursor",
        "/opt/homebrew/bin/cursor",
        "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
    ] {
        attempts.push((cli.to_string(), vec!["-g".into(), location.clone()]));
    }

    for env_name in ["VISUAL", "EDITOR"] {
        if let Ok(editor) = std::env::var(env_name) {
            let editor = editor.trim();
            if editor.is_empty() {
                continue;
            }
            let mut parts = editor.split_whitespace();
            if let Some(program) = parts.next() {
                let mut args = parts.map(str::to_string).collect::<Vec<_>>();
                args.push("-g".into());
                args.push(location.clone());
                attempts.push((program.to_string(), args));
            }
        }
    }

    let mut app_names = vec!["Visual Studio Code".to_string(), "Code".to_string(), "Cursor".to_string()];
    if let Ok(editor) = std::env::var("EDITOR") {
        let editor = editor.trim();
        if !editor.is_empty() && !app_names.iter().any(|name| name == editor) {
            app_names.insert(0, editor.to_string());
        }
    }

    for app_name in app_names {
        attempts.push((
            "open".to_string(),
            vec!["-a".into(), app_name, "--args".into(), "-g".into(), location.clone()],
        ));
    }

    attempts.push(("open".to_string(), vec![path_str.to_string()]));

    let mut errors = Vec::new();
    for (program, args) in attempts {
        match run_open_command(&program, &args) {
            Ok(()) => {
                crate::app_log!("INFO", "[open_file] opened via {} {}", program, args.join(" "));
                return Ok(());
            }
            Err(error) => errors.push(error),
        }
    }

    let message = format!("Failed to open file: {}", errors.join(" | "));
    crate::app_log!("WARN", "[open_file] {}", message);
    Err(message)
}

#[cfg(target_os = "macos")]
fn run_open_command(program: &str, args: &[String]) -> Result<(), String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{} {} ({})", program, args.join(" "), e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("exit status {}", output.status)
    } else {
        stderr
    };
    Err(format!("{} {} ({})", program, args.join(" "), detail))
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
