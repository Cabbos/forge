use base64::{engine::general_purpose, Engine as _};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use crate::ipc::workspace::resolve_bound_working_dir;
use crate::state::AppState;

const MAX_UNTRACKED_SNAPSHOT_FILE_BYTES: u64 = 512 * 1024;
const MAX_UNTRACKED_SNAPSHOT_TOTAL_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct StoredCheckpointFile {
    pub(crate) path: String,
    pub(crate) content_base64: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct StoredProjectCheckpoint {
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) head: String,
    pub(crate) status: String,
    pub(crate) diff_patch: String,
    #[serde(default)]
    pub(crate) untracked_files: Vec<StoredCheckpointFile>,
    #[serde(default)]
    pub(crate) skipped_untracked_files: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct ProjectCheckpointMetadata {
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) head: String,
    pub(crate) status: String,
    pub(crate) restorable: bool,
    pub(crate) untracked_file_count: usize,
    pub(crate) skipped_untracked_file_count: usize,
}

impl From<StoredProjectCheckpoint> for ProjectCheckpointMetadata {
    fn from(checkpoint: StoredProjectCheckpoint) -> Self {
        let restorable = checkpoint_is_restorable(&checkpoint);
        let untracked_file_count = checkpoint.untracked_files.len();
        let skipped_untracked_file_count = checkpoint.skipped_untracked_files.len();
        Self {
            id: checkpoint.id,
            created_at: checkpoint.created_at,
            head: checkpoint.head,
            status: checkpoint.status,
            restorable,
            untracked_file_count,
            skipped_untracked_file_count,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ProjectCheckpointStatus {
    pub(crate) working_dir: String,
    pub(crate) is_git_repo: bool,
    pub(crate) dirty: bool,
    pub(crate) last_checkpoint: Option<ProjectCheckpointMetadata>,
    pub(crate) restorable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) snapshot_warning: Option<String>,
    pub(crate) message: String,
}

#[tauri::command]
pub async fn get_project_checkpoint_status(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    project_checkpoint_status_for_request(&state, session_id.as_deref(), working_dir.as_deref())
        .await
}

pub(crate) async fn project_checkpoint_status_for_session(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir = resolve_bound_working_dir(state, session_id, None).await?;
    checkpoint_status(&working_dir)
}

pub(crate) fn project_checkpoint_status_for_path(
    working_dir: &Path,
) -> Result<ProjectCheckpointStatus, String> {
    checkpoint_status(working_dir)
}

async fn project_checkpoint_status_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir = checkpoint_working_dir_or_explicit(state, session_id, working_dir).await?;
    checkpoint_status(&working_dir)
}

#[tauri::command]
pub async fn create_project_checkpoint(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir =
        checkpoint_working_dir_or_explicit(&state, session_id.as_deref(), working_dir.as_deref())
            .await?;
    if !is_git_repo(&working_dir) {
        return Ok(ProjectCheckpointStatus {
            working_dir: working_dir.to_string_lossy().to_string(),
            is_git_repo: false,
            dirty: false,
            last_checkpoint: None,
            restorable: false,
            snapshot_warning: None,
            message: "当前项目不是 Git 仓库，暂时不能创建检查点".into(),
        });
    }

    let status = run_git(&working_dir, &["status", "--porcelain"])?;
    let head = run_git(&working_dir, &["rev-parse", "--short", "HEAD"])
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();
    let diff_patch = run_git(&working_dir, &["diff", "--binary"])?;
    let (untracked_files, skipped_untracked_files) = snapshot_untracked_files(&working_dir)?;
    let checkpoint = StoredProjectCheckpoint {
        id: uuid::Uuid::now_v7().to_string(),
        created_at: now_secs(),
        head,
        status,
        diff_patch,
        untracked_files,
        skipped_untracked_files,
    };

    save_checkpoint(&working_dir, &checkpoint)?;
    checkpoint_status(&working_dir)
}

#[tauri::command]
pub async fn restore_project_checkpoint(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectCheckpointStatus, String> {
    let working_dir =
        checkpoint_working_dir_or_explicit(&state, session_id.as_deref(), working_dir.as_deref())
            .await?;
    let checkpoint =
        load_checkpoint(&working_dir)?.ok_or_else(|| "还没有可回退的检查点".to_string())?;

    restore_checkpoint(&working_dir, &checkpoint)?;

    checkpoint_status(&working_dir)
}

async fn checkpoint_working_dir_or_explicit(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    resolve_bound_working_dir(state, session_id, working_dir).await
}

fn checkpoint_status(working_dir: &std::path::Path) -> Result<ProjectCheckpointStatus, String> {
    let is_git_repo = is_git_repo(working_dir);
    let status = if is_git_repo {
        run_git(working_dir, &["status", "--porcelain"]).unwrap_or_default()
    } else {
        String::new()
    };
    let dirty = !status.trim().is_empty();
    let stored_checkpoint = load_checkpoint(working_dir)?;
    let last_checkpoint = stored_checkpoint.map(ProjectCheckpointMetadata::from);
    let restorable = last_checkpoint
        .as_ref()
        .is_some_and(|checkpoint| checkpoint.restorable);
    let skipped_untracked_file_count = last_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.skipped_untracked_file_count)
        .unwrap_or(0);
    let untracked_file_count = last_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.untracked_file_count)
        .unwrap_or(0);
    let snapshot_warning = if skipped_untracked_file_count > 0 {
        Some(format!(
            "{} 个未跟踪文件过大或不可读取，未纳入检查点",
            skipped_untracked_file_count
        ))
    } else {
        None
    };
    let message = if !is_git_repo {
        "当前项目不是 Git 仓库，检查点不可用".to_string()
    } else if last_checkpoint.is_none() {
        "还没有检查点，发送任务前会自动创建".to_string()
    } else if !restorable {
        "检查点存在，但没有可回退内容".to_string()
    } else if skipped_untracked_file_count > 0 {
        "检查点已保存，但部分未跟踪文件未纳入".to_string()
    } else if untracked_file_count > 0 {
        "已保存修改前检查点，包含未跟踪文件快照".to_string()
    } else {
        "已保存修改前检查点，可按需回退 tracked 文件".to_string()
    };

    Ok(ProjectCheckpointStatus {
        working_dir: working_dir.to_string_lossy().to_string(),
        is_git_repo,
        dirty,
        last_checkpoint,
        restorable,
        snapshot_warning,
        message,
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

fn restore_checkpoint(
    working_dir: &Path,
    checkpoint: &StoredProjectCheckpoint,
) -> Result<(), String> {
    if !checkpoint_is_restorable(checkpoint) {
        return Err("检查点没有可回退内容".to_string());
    }

    let rollback_patch = run_git(working_dir, &["diff", "--binary"]).unwrap_or_default();
    if git_has_head(working_dir) {
        run_git(working_dir, &["reset", "--hard", "HEAD"])?;
    }
    remove_untracked_files_not_in_checkpoint(working_dir, checkpoint)?;
    restore_untracked_files(working_dir, checkpoint)?;
    if checkpoint.diff_patch.trim().is_empty() {
        return Ok(());
    }

    if let Err(apply_error) = apply_patch(working_dir, &checkpoint.diff_patch) {
        if !rollback_patch.trim().is_empty() {
            let _ = apply_patch(working_dir, &rollback_patch);
        }
        return Err(format!(
            "回退检查点失败，已尝试恢复回退前的改动: {apply_error}"
        ));
    }
    Ok(())
}

fn checkpoint_is_restorable(checkpoint: &StoredProjectCheckpoint) -> bool {
    let head = checkpoint.head.trim();
    (!head.is_empty() && head != "unknown")
        || !checkpoint.diff_patch.trim().is_empty()
        || !checkpoint.untracked_files.is_empty()
}

fn git_has_head(working_dir: &Path) -> bool {
    run_git(working_dir, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

fn snapshot_untracked_files(
    working_dir: &Path,
) -> Result<(Vec<StoredCheckpointFile>, Vec<String>), String> {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut total_bytes = 0_u64;

    for relative_path in list_untracked_relative_paths(working_dir)? {
        if is_checkpoint_internal_path(&relative_path) {
            continue;
        }
        let path = workspace_file_path(working_dir, &relative_path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| format!("读取未跟踪文件元数据失败 {relative_path}: {e}"))?;
        if metadata.file_type().is_symlink() {
            skipped.push(format!("{relative_path} (符号链接未纳入检查点)"));
            continue;
        }
        if !metadata.is_file() {
            skipped.push(format!("{relative_path} (不是普通文件)"));
            continue;
        }
        if metadata.len() > MAX_UNTRACKED_SNAPSHOT_FILE_BYTES {
            skipped.push(format!("{relative_path} (文件过大)"));
            continue;
        }
        if total_bytes.saturating_add(metadata.len()) > MAX_UNTRACKED_SNAPSHOT_TOTAL_BYTES {
            skipped.push(format!("{relative_path} (检查点容量上限)"));
            continue;
        }

        let bytes =
            fs::read(&path).map_err(|e| format!("读取未跟踪文件失败 {relative_path}: {e}"))?;
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        files.push(StoredCheckpointFile {
            path: relative_path,
            content_base64: general_purpose::STANDARD.encode(bytes),
        });
    }

    Ok((files, skipped))
}

fn list_untracked_relative_paths(working_dir: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("git ls-files 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git ls-files 失败".into()
        } else {
            stderr
        });
    }

    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect())
}

fn remove_untracked_files_not_in_checkpoint(
    working_dir: &Path,
    checkpoint: &StoredProjectCheckpoint,
) -> Result<(), String> {
    let keep_paths: HashSet<&str> = checkpoint
        .untracked_files
        .iter()
        .map(|file| file.path.as_str())
        .collect();

    for relative_path in list_untracked_relative_paths(working_dir)? {
        if is_checkpoint_internal_path(&relative_path)
            || keep_paths.contains(relative_path.as_str())
        {
            continue;
        }
        let path = workspace_file_path(working_dir, &relative_path)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
                fs::remove_file(&path)
                    .map_err(|e| format!("移除检查点后的未跟踪文件失败 {relative_path}: {e}"))?;
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(format!("检查未跟踪文件失败 {relative_path}: {err}")),
        }
    }

    Ok(())
}

fn restore_untracked_files(
    working_dir: &Path,
    checkpoint: &StoredProjectCheckpoint,
) -> Result<(), String> {
    for file in &checkpoint.untracked_files {
        let path = workspace_file_path(working_dir, &file.path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建检查点文件目录失败 {}: {e}", file.path))?;
            ensure_path_stays_in_workspace(working_dir, parent, "检查点文件目录")?;
        }
        reject_symlink_path(&path, "检查点恢复文件")?;
        let bytes = general_purpose::STANDARD
            .decode(&file.content_base64)
            .map_err(|e| format!("解析未跟踪文件快照失败 {}: {e}", file.path))?;
        fs::write(&path, bytes).map_err(|e| format!("恢复未跟踪文件失败 {}: {e}", file.path))?;
    }
    Ok(())
}

fn workspace_file_path(
    working_dir: &Path,
    relative_path: &str,
) -> Result<std::path::PathBuf, String> {
    validate_checkpoint_relative_path(relative_path)?;
    Ok(working_dir.join(relative_path))
}

fn validate_checkpoint_relative_path(relative_path: &str) -> Result<(), String> {
    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return Err("检查点文件路径不能为空".to_string());
    }
    if is_checkpoint_internal_path(trimmed) {
        return Err("检查点不能覆盖 Forge 内部数据目录".to_string());
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err("检查点文件路径必须是项目内相对路径".to_string());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err("检查点文件路径不能离开当前项目".to_string()),
        }
    }
    Ok(())
}

fn is_checkpoint_internal_path(relative_path: &str) -> bool {
    relative_path == ".forge" || relative_path.starts_with(".forge/")
}

fn ensure_path_stays_in_workspace(
    working_dir: &Path,
    path: &Path,
    label: &str,
) -> Result<(), String> {
    let workspace = working_dir
        .canonicalize()
        .map_err(|e| format!("无法解析当前项目路径: {}", e))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("无法解析{label}: {e}"))?;
    if !canonical_path.starts_with(workspace) {
        return Err(format!("{label}不能离开当前项目"));
    }
    Ok(())
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
                restorable: true,
                untracked_file_count: 0,
                skipped_untracked_file_count: 0,
            }),
            restorable: true,
            snapshot_warning: None,
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

    #[tokio::test]
    async fn checkpoint_request_requires_session_or_explicit_workspace() {
        let workspace = temp_project("missing-workspace-binding");
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::new(
            crate::harness::Harness::new(workspace.clone()),
        )));

        let error = checkpoint_working_dir_or_explicit(&state, None, None)
            .await
            .expect_err("missing workspace should fail");

        assert!(error.contains("工作空间"));

        let _ = fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn checkpoint_request_uses_session_workspace_over_explicit_workspace() {
        let session_workspace = temp_project("checkpoint-session-workspace");
        let explicit_workspace = temp_project("checkpoint-explicit-workspace");
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::new(
            crate::harness::Harness::new(explicit_workspace.clone()),
        )));
        let session = std::sync::Arc::new(crate::agent::session::AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            std::sync::Arc::new(crate::adapters::missing_key::MissingKeyAdapter::new(
                "DeepSeek",
                "deepseek-chat",
            )),
            std::sync::Arc::new(crate::harness::Harness::new(session_workspace.clone())),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;

        let resolved = checkpoint_working_dir_or_explicit(
            &state,
            Some("session-1"),
            Some(explicit_workspace.to_str().expect("utf8")),
        )
        .await
        .expect("checkpoint workspace should resolve");

        assert_eq!(
            resolved.canonicalize().expect("resolved workspace"),
            session_workspace.canonicalize().expect("session workspace")
        );
        assert_ne!(
            resolved.canonicalize().expect("resolved workspace"),
            explicit_workspace
                .canonicalize()
                .expect("explicit workspace")
        );

        let _ = fs::remove_dir_all(session_workspace);
        let _ = fs::remove_dir_all(explicit_workspace);
    }

    #[test]
    fn restore_checkpoint_restores_previous_diff_when_checkpoint_apply_fails() {
        let project = temp_project("checkpoint-restore-rollback");
        init_git_repo(&project);
        let file = project.join("app.txt");
        fs::write(&file, "base\n").expect("write base");
        run_git(&project, &["add", "app.txt"]).expect("git add");
        run_git(&project, &["commit", "-m", "base"]).expect("git commit");
        fs::write(&file, "current\n").expect("write current diff");

        let checkpoint = StoredProjectCheckpoint {
            diff_patch: "this is not a git patch".to_string(),
            ..sample_checkpoint()
        };
        let error = restore_checkpoint(&project, &checkpoint)
            .expect_err("invalid checkpoint patch should fail");

        assert!(error.contains("已尝试恢复回退前的改动"));
        assert_eq!(
            fs::read_to_string(&file).expect("read restored file"),
            "current\n"
        );

        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn checkpoint_status_marks_legacy_empty_checkpoint_as_not_restorable() {
        let project = temp_project("checkpoint-empty-legacy");
        init_git_repo(&project);
        let checkpoint = StoredProjectCheckpoint {
            head: "unknown".to_string(),
            diff_patch: String::new(),
            ..sample_checkpoint()
        };
        save_checkpoint(&project, &checkpoint).expect("save checkpoint");

        let status = checkpoint_status(&project).expect("checkpoint status");

        assert!(!status.restorable);
        assert_eq!(status.message, "检查点存在，但没有可回退内容");

        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn checkpoint_status_reports_untracked_snapshot_for_no_commit_repo() {
        let project = temp_project("checkpoint-untracked-status");
        init_git_repo(&project);
        fs::write(project.join("package.json"), "{\"name\":\"before\"}\n").expect("write package");
        let (untracked_files, skipped_untracked_files) =
            snapshot_untracked_files(&project).expect("snapshot untracked");
        let checkpoint = StoredProjectCheckpoint {
            head: "unknown".to_string(),
            diff_patch: String::new(),
            untracked_files,
            skipped_untracked_files,
            ..sample_checkpoint()
        };
        save_checkpoint(&project, &checkpoint).expect("save checkpoint");

        let status = checkpoint_status(&project).expect("checkpoint status");

        assert!(status.restorable);
        assert_eq!(status.message, "已保存修改前检查点，包含未跟踪文件快照");
        assert_eq!(
            status
                .last_checkpoint
                .as_ref()
                .expect("checkpoint metadata")
                .untracked_file_count,
            1
        );

        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn restore_checkpoint_restores_untracked_files_in_no_commit_repo() {
        let project = temp_project("checkpoint-untracked-restore");
        init_git_repo(&project);
        fs::write(project.join("package.json"), "{\"name\":\"before\"}\n").expect("write package");
        let (untracked_files, skipped_untracked_files) =
            snapshot_untracked_files(&project).expect("snapshot untracked");
        let checkpoint = StoredProjectCheckpoint {
            head: "unknown".to_string(),
            diff_patch: String::new(),
            untracked_files,
            skipped_untracked_files,
            ..sample_checkpoint()
        };

        fs::create_dir_all(project.join("src")).expect("create src");
        fs::write(project.join("package.json"), "{\"name\":\"after\"}\n").expect("edit package");
        fs::write(project.join("src/storage.ts"), "export const value = 1;\n")
            .expect("write new file");

        restore_checkpoint(&project, &checkpoint).expect("restore checkpoint");

        assert_eq!(
            fs::read_to_string(project.join("package.json")).expect("read package"),
            "{\"name\":\"before\"}\n"
        );
        assert!(
            !project.join("src/storage.ts").exists(),
            "new untracked files after the checkpoint should be removed"
        );

        let _ = fs::remove_dir_all(project);
    }

    fn sample_checkpoint() -> StoredProjectCheckpoint {
        StoredProjectCheckpoint {
            id: "checkpoint-1".to_string(),
            created_at: 123,
            head: "abc123".to_string(),
            status: " M src/App.tsx".to_string(),
            diff_patch: "diff --git a/src/App.tsx b/src/App.tsx".to_string(),
            untracked_files: Vec::new(),
            skipped_untracked_files: Vec::new(),
        }
    }

    fn temp_project(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("forge-{name}-{}", uuid::Uuid::now_v7()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn init_git_repo(path: &std::path::Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .expect("git init");
        run_git(path, &["config", "user.email", "forge@example.test"]).expect("git config email");
        run_git(path, &["config", "user.name", "Forge Test"]).expect("git config name");
    }
}
