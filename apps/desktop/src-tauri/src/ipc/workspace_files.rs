use std::path::PathBuf;
use std::sync::Arc;

use crate::ipc::file_search::find_files;
use crate::ipc::open_file::open_file_macos;
use crate::ipc::open_file::resolve_workspace_file_path;
use crate::ipc::workspace::resolve_bound_working_dir;
use crate::state::AppState;

#[derive(serde::Serialize)]
pub struct FilePreviewLine {
    pub(crate) number: usize,
    pub(crate) content: String,
    pub(crate) is_target: bool,
}

#[derive(serde::Serialize)]
pub struct FilePreview {
    pub(crate) path: String,
    pub(crate) display_path: String,
    pub(crate) requested_line: Option<u32>,
    pub(crate) start_line: usize,
    pub(crate) total_lines: usize,
    pub(crate) lines: Vec<FilePreviewLine>,
}

pub(crate) async fn working_dir_for_request_or_explicit(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<PathBuf, String> {
    resolve_bound_working_dir(state, session_id, working_dir).await
}

pub(crate) async fn search_workspace_files_for_request(
    state: &Arc<AppState>,
    query: &str,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<Vec<String>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let results = find_files(&working_dir, query, 20);
    Ok(results)
}

#[tauri::command]
pub async fn search_workspace_files(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<Vec<String>, String> {
    search_workspace_files_for_request(
        &state,
        &query,
        session_id.as_deref(),
        working_dir.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_default_working_dir(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<String, String> {
    Ok(state.harness.working_dir.to_string_lossy().to_string())
}

pub(crate) async fn preview_file_for_request(
    state: &Arc<AppState>,
    path: &str,
    line: Option<u32>,
    context: Option<u32>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<FilePreview, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let full_path = resolve_workspace_file_path(&working_dir, path)?;

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

/// Preview a small slice of a file around a target line inside the app.
#[tauri::command]
pub async fn preview_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    line: Option<u32>,
    context: Option<u32>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<FilePreview, String> {
    preview_file_for_request(
        &state,
        &path,
        line,
        context,
        session_id.as_deref(),
        working_dir.as_deref(),
    )
    .await
}

pub(crate) async fn open_file_target_for_request(
    state: &Arc<AppState>,
    path: &str,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<PathBuf, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let full_path = resolve_workspace_file_path(&working_dir, path)?;
    if !full_path.exists() {
        let message = format!("File not found: {}", full_path.display());
        crate::app_log!("WARN", "[open_file] {}", message);
        return Err(message);
    }
    Ok(full_path)
}

/// Open a file in the system's default editor at a specific line.
#[tauri::command]
pub async fn open_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    line: Option<u32>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<(), String> {
    let full_path =
        open_file_target_for_request(&state, &path, session_id.as_deref(), working_dir.as_deref())
            .await?;

    crate::app_log!(
        "INFO",
        "[open_file] request path={} line={:?} resolved={}",
        path,
        line,
        full_path.display()
    );

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
