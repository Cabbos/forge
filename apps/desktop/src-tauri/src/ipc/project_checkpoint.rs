use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use crate::state::AppState;

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct ProjectCheckpoint {
    id: String,
    created_at: u64,
    head: String,
    status: String,
    diff_patch: String,
}

#[derive(serde::Serialize)]
pub struct ProjectCheckpointStatus {
    working_dir: String,
    is_git_repo: bool,
    dirty: bool,
    last_checkpoint: Option<ProjectCheckpoint>,
    message: String,
}

#[tauri::command]
pub async fn get_project_checkpoint_status(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir = checkpoint_working_dir(&state, session_id.as_deref()).await;
    checkpoint_status(&working_dir)
}

#[tauri::command]
pub async fn create_project_checkpoint(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir = checkpoint_working_dir(&state, session_id.as_deref()).await;
    if !is_git_repo(&working_dir) {
        return Ok(ProjectCheckpointStatus {
            working_dir: working_dir.to_string_lossy().to_string(),
            is_git_repo: false,
            dirty: false,
            last_checkpoint: None,
            message: "当前项目不是 Git 仓库，暂时不能创建检查点".into(),
        });
    }

    let status = run_git(&working_dir, &["status", "--porcelain"])?;
    let head = run_git(&working_dir, &["rev-parse", "--short", "HEAD"])
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();
    let diff_patch = run_git(&working_dir, &["diff", "--binary"])?;
    let checkpoint = ProjectCheckpoint {
        id: uuid::Uuid::now_v7().to_string(),
        created_at: now_secs(),
        head,
        status,
        diff_patch,
    };

    save_checkpoint(&working_dir, &checkpoint)?;
    checkpoint_status(&working_dir)
}

#[tauri::command]
pub async fn restore_project_checkpoint(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir = checkpoint_working_dir(&state, session_id.as_deref()).await;
    let checkpoint =
        load_checkpoint(&working_dir)?.ok_or_else(|| "还没有可回退的检查点".to_string())?;

    run_git(&working_dir, &["reset", "--hard", "HEAD"])?;
    if !checkpoint.diff_patch.trim().is_empty() {
        apply_patch(&working_dir, &checkpoint.diff_patch)?;
    }

    checkpoint_status(&working_dir)
}

async fn checkpoint_working_dir(
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

fn checkpoint_status(working_dir: &std::path::Path) -> Result<ProjectCheckpointStatus, String> {
    let is_git_repo = is_git_repo(working_dir);
    let status = if is_git_repo {
        run_git(working_dir, &["status", "--porcelain"]).unwrap_or_default()
    } else {
        String::new()
    };
    let dirty = !status.trim().is_empty();
    let last_checkpoint = load_checkpoint(working_dir)?;
    let message = if !is_git_repo {
        "当前项目不是 Git 仓库，检查点不可用"
    } else if last_checkpoint.is_some() {
        "已保存修改前检查点，可按需回退 tracked 文件"
    } else {
        "还没有检查点，发送任务前会自动创建"
    };

    Ok(ProjectCheckpointStatus {
        working_dir: working_dir.to_string_lossy().to_string(),
        is_git_repo,
        dirty,
        last_checkpoint,
        message: message.into(),
    })
}

fn is_git_repo(working_dir: &std::path::Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(working_dir)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_git(working_dir: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("git {} 失败: {}", args.join(" "), e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("git {} 失败", args.join(" "))
        } else {
            stderr
        })
    }
}

fn apply_patch(working_dir: &std::path::Path, patch: &str) -> Result<(), String> {
    let mut child = Command::new("git")
        .args(["apply", "--binary", "--whitespace=nowarn", "-"])
        .current_dir(working_dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git apply 失败: {}", e))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(patch.as_bytes())
            .map_err(|e| format!("写入检查点 patch 失败: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待 git apply 失败: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "回退检查点失败".into()
        } else {
            stderr
        })
    }
}

fn checkpoint_file(working_dir: &std::path::Path) -> std::path::PathBuf {
    working_dir
        .join(".forge")
        .join("checkpoints")
        .join("latest.json")
}

fn save_checkpoint(
    working_dir: &std::path::Path,
    checkpoint: &ProjectCheckpoint,
) -> Result<(), String> {
    let path = checkpoint_file(working_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建检查点目录失败: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(checkpoint).map_err(|e| format!("序列化检查点失败: {}", e))?;
    std::fs::write(path, json).map_err(|e| format!("写入检查点失败: {}", e))
}

fn load_checkpoint(working_dir: &std::path::Path) -> Result<Option<ProjectCheckpoint>, String> {
    let path = checkpoint_file(working_dir);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|e| format!("读取检查点失败: {}", e))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| format!("检查点文件损坏: {}", e))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
