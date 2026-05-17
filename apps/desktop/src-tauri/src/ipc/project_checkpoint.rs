use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use crate::state::AppState;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct StoredProjectCheckpoint {
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) head: String,
    pub(crate) status: String,
    pub(crate) diff_patch: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ProjectCheckpointMetadata {
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) head: String,
    pub(crate) status: String,
}

impl From<StoredProjectCheckpoint> for ProjectCheckpointMetadata {
    fn from(checkpoint: StoredProjectCheckpoint) -> Self {
        Self {
            id: checkpoint.id,
            created_at: checkpoint.created_at,
            head: checkpoint.head,
            status: checkpoint.status,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ProjectCheckpointStatus {
    pub(crate) working_dir: String,
    pub(crate) is_git_repo: bool,
    pub(crate) dirty: bool,
    pub(crate) last_checkpoint: Option<ProjectCheckpointMetadata>,
    pub(crate) message: String,
}

#[tauri::command]
pub async fn get_project_checkpoint_status(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    project_checkpoint_status_for_session(&state, session_id.as_deref()).await
}

pub(crate) async fn project_checkpoint_status_for_session(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir = checkpoint_working_dir(state, session_id).await;
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
    let checkpoint = StoredProjectCheckpoint {
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
    let has_checkpoint = last_checkpoint.is_some();
    let message = if !is_git_repo {
        "当前项目不是 Git 仓库，检查点不可用"
    } else if has_checkpoint {
        "已保存修改前检查点，可按需回退 tracked 文件"
    } else {
        "还没有检查点，发送任务前会自动创建"
    };

    Ok(ProjectCheckpointStatus {
        working_dir: working_dir.to_string_lossy().to_string(),
        is_git_repo,
        dirty,
        last_checkpoint: last_checkpoint.map(ProjectCheckpointMetadata::from),
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

fn checkpoint_dir(working_dir: &std::path::Path) -> std::path::PathBuf {
    working_dir.join(".forge").join("checkpoints")
}

fn save_checkpoint(
    working_dir: &std::path::Path,
    checkpoint: &StoredProjectCheckpoint,
) -> Result<(), String> {
    let path = prepare_checkpoint_path(working_dir, true)?;
    let json =
        serde_json::to_string_pretty(checkpoint).map_err(|e| format!("序列化检查点失败: {}", e))?;
    let temp_path = path.with_extension(format!("json.tmp-{}", uuid::Uuid::now_v7()));

    let write_result = (|| {
        reject_symlink_path(&temp_path, "临时检查点文件")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|e| format!("创建临时检查点文件失败: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("写入临时检查点失败: {}", e))?;
        file.sync_all()
            .map_err(|e| format!("同步临时检查点失败: {}", e))?;
        reject_symlink_path(&path, "检查点文件")?;
        fs::rename(&temp_path, &path).map_err(|e| format!("替换检查点失败: {}", e))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn load_checkpoint(
    working_dir: &std::path::Path,
) -> Result<Option<StoredProjectCheckpoint>, String> {
    let path = prepare_checkpoint_path(working_dir, false)?;
    if !path.exists() {
        return Ok(None);
    }
    reject_symlink_path(&path, "检查点文件")?;
    let content = fs::read_to_string(path).map_err(|e| format!("读取检查点失败: {}", e))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| format!("检查点文件损坏: {}", e))
}

fn prepare_checkpoint_path(
    working_dir: &std::path::Path,
    create_dir: bool,
) -> Result<std::path::PathBuf, String> {
    let workspace = working_dir
        .canonicalize()
        .map_err(|e| format!("无法解析当前项目路径: {}", e))?;
    let forge_dir = working_dir.join(".forge");
    let checkpoint_dir = checkpoint_dir(working_dir);

    reject_symlink_path(&forge_dir, "Forge 数据目录")?;
    reject_symlink_path(&checkpoint_dir, "检查点目录")?;
    if create_dir {
        fs::create_dir_all(&checkpoint_dir).map_err(|e| format!("创建检查点目录失败: {}", e))?;
        reject_symlink_path(&forge_dir, "Forge 数据目录")?;
        reject_symlink_path(&checkpoint_dir, "检查点目录")?;
    }
    if checkpoint_dir.exists() {
        let canonical_dir = checkpoint_dir
            .canonicalize()
            .map_err(|e| format!("无法解析检查点目录: {}", e))?;
        if !canonical_dir.starts_with(&workspace) {
            return Err("检查点目录不能离开当前项目".to_string());
        }
    }

    let path = checkpoint_file(working_dir);
    reject_symlink_path(&path, "检查点文件")?;
    Ok(path)
}

fn reject_symlink_path(path: &std::path::Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!("{label}不能是符号链接")),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("无法检查{label}: {err}")),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_status_serialization_excludes_diff_patch() {
        let status = ProjectCheckpointStatus {
            working_dir: "/workspace".to_string(),
            is_git_repo: true,
            dirty: true,
            last_checkpoint: Some(ProjectCheckpointMetadata {
                id: "checkpoint-1".to_string(),
                created_at: 123,
                head: "abc123".to_string(),
                status: " M src/App.tsx".to_string(),
            }),
            message: "已保存修改前检查点，可按需回退 tracked 文件".to_string(),
        };

        let json = serde_json::to_string(&status).expect("serialize status");

        assert!(json.contains("checkpoint-1"));
        assert!(!json.contains("diff_patch"));
    }

    #[cfg(unix)]
    #[test]
    fn save_checkpoint_rejects_symlinked_latest_file() {
        use std::os::unix::fs as unix_fs;

        let project = temp_project("checkpoint-save-symlink");
        let external = temp_project("checkpoint-save-symlink-external");
        let checkpoint_dir = checkpoint_dir(&project);
        fs::create_dir_all(&checkpoint_dir).expect("create checkpoint dir");
        fs::create_dir_all(&external).expect("create external dir");
        let external_file = external.join("latest.json");
        fs::write(&external_file, "outside").expect("write external file");
        unix_fs::symlink(&external_file, checkpoint_dir.join("latest.json"))
            .expect("create latest symlink");

        let checkpoint = sample_checkpoint();
        let error = save_checkpoint(&project, &checkpoint)
            .expect_err("symlinked checkpoint file should be rejected");

        assert!(
            error.contains("符号链接"),
            "expected symlink rejection, got {error}"
        );
        assert_eq!(
            fs::read_to_string(external_file).expect("read external file"),
            "outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_checkpoint_rejects_symlinked_latest_file() {
        use std::os::unix::fs as unix_fs;

        let project = temp_project("checkpoint-load-symlink");
        let external = temp_project("checkpoint-load-symlink-external");
        let checkpoint_dir = checkpoint_dir(&project);
        fs::create_dir_all(&checkpoint_dir).expect("create checkpoint dir");
        fs::create_dir_all(&external).expect("create external dir");
        let external_file = external.join("latest.json");
        fs::write(&external_file, "{}").expect("write external file");
        unix_fs::symlink(&external_file, checkpoint_dir.join("latest.json"))
            .expect("create latest symlink");

        let error =
            load_checkpoint(&project).expect_err("symlinked checkpoint file should be rejected");

        assert!(
            error.contains("符号链接"),
            "expected symlink rejection, got {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_checkpoint_rejects_symlinked_checkpoint_dir() {
        use std::os::unix::fs as unix_fs;

        let project = temp_project("checkpoint-dir-symlink");
        let external = temp_project("checkpoint-dir-symlink-external");
        let forge_dir = project.join(".forge");
        fs::create_dir_all(&forge_dir).expect("create forge dir");
        fs::create_dir_all(&external).expect("create external dir");
        unix_fs::symlink(&external, forge_dir.join("checkpoints"))
            .expect("create checkpoint dir symlink");

        let error = save_checkpoint(&project, &sample_checkpoint())
            .expect_err("symlinked checkpoint dir should be rejected");

        assert!(
            error.contains("符号链接"),
            "expected symlink rejection, got {error}"
        );
    }

    fn sample_checkpoint() -> StoredProjectCheckpoint {
        StoredProjectCheckpoint {
            id: "checkpoint-1".to_string(),
            created_at: 123,
            head: "abc123".to_string(),
            status: " M src/App.tsx".to_string(),
            diff_patch: "diff --git a/src/App.tsx b/src/App.tsx".to_string(),
        }
    }

    fn temp_project(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("forge-{name}-{}", uuid::Uuid::now_v7()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp project");
        path
    }
}
