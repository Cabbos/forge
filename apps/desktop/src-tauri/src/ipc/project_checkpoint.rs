use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use super::checkpoint_snapshot::{
    capture_snapshot, restore_snapshot_transactional, WorktreeSnapshot, CHECKPOINT_SCHEMA_VERSION,
};
use crate::ipc::workspace::resolve_bound_working_dir;
use crate::state::AppState;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct LegacyStoredCheckpointFile {
    pub(crate) path: String,
    pub(crate) content_base64: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct LegacyStoredProjectCheckpoint {
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) head: String,
    pub(crate) status: String,
    pub(crate) diff_patch: String,
    #[serde(default)]
    pub(crate) untracked_files: Vec<LegacyStoredCheckpointFile>,
    #[serde(default)]
    pub(crate) skipped_untracked_files: Vec<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
struct StoredProjectCheckpoint {
    pub(crate) schema_version: u32,
    pub(crate) id: String,
    pub(crate) created_at: u64,
    pub(crate) snapshot: WorktreeSnapshot,
}

#[derive(Debug, Clone)]
enum LoadedProjectCheckpoint {
    V2(StoredProjectCheckpoint),
    LegacyV1(LegacyStoredProjectCheckpoint),
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
    pub(crate) schema_version: u32,
}

impl From<LoadedProjectCheckpoint> for ProjectCheckpointMetadata {
    fn from(checkpoint: LoadedProjectCheckpoint) -> Self {
        let restorable = checkpoint_is_restorable(&checkpoint);
        match checkpoint {
            LoadedProjectCheckpoint::V2(checkpoint) => Self {
                id: checkpoint.id,
                created_at: checkpoint.created_at,
                head: checkpoint
                    .snapshot
                    .head_oid
                    .clone()
                    .unwrap_or_else(|| "unborn".to_string()),
                status: checkpoint.snapshot.status_porcelain_v2.clone(),
                restorable,
                untracked_file_count: checkpoint.snapshot.untracked_files.len(),
                skipped_untracked_file_count: checkpoint.snapshot.unsupported_paths.len(),
                schema_version: checkpoint.schema_version,
            },
            LoadedProjectCheckpoint::LegacyV1(checkpoint) => Self {
                id: checkpoint.id,
                created_at: checkpoint.created_at,
                head: checkpoint.head,
                status: checkpoint.status,
                restorable,
                untracked_file_count: checkpoint.untracked_files.len(),
                skipped_untracked_file_count: checkpoint.skipped_untracked_files.len(),
                schema_version: 1,
            },
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

    let snapshot = capture_snapshot(&working_dir)?;
    let checkpoint = StoredProjectCheckpoint {
        schema_version: CHECKPOINT_SCHEMA_VERSION,
        id: uuid::Uuid::now_v7().to_string(),
        created_at: now_secs(),
        snapshot,
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
    let legacy_checkpoint = last_checkpoint
        .as_ref()
        .is_some_and(|checkpoint| checkpoint.schema_version < CHECKPOINT_SCHEMA_VERSION);
    let snapshot_warning = if legacy_checkpoint {
        Some("旧版检查点不能安全恢复，请重新创建检查点".to_string())
    } else if skipped_untracked_file_count > 0 {
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
    } else if legacy_checkpoint {
        "旧版检查点必须重新创建后才能恢复".to_string()
    } else if !restorable {
        "检查点包含不支持的路径，必须重新创建".to_string()
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
    checkpoint: &LoadedProjectCheckpoint,
) -> Result<(), String> {
    match checkpoint {
        LoadedProjectCheckpoint::V2(checkpoint) => {
            restore_snapshot_transactional(working_dir, &checkpoint.snapshot)
        }
        LoadedProjectCheckpoint::LegacyV1(_) => {
            Err("旧版检查点必须重新创建；Forge 未修改当前工作区".to_string())
        }
    }
}

fn checkpoint_is_restorable(checkpoint: &LoadedProjectCheckpoint) -> bool {
    match checkpoint {
        LoadedProjectCheckpoint::V2(checkpoint) => checkpoint.snapshot.is_restorable(),
        LoadedProjectCheckpoint::LegacyV1(_) => false,
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
) -> Result<Option<LoadedProjectCheckpoint>, String> {
    let path = prepare_checkpoint_path(working_dir, false)?;
    if !path.exists() {
        return Ok(None);
    }
    reject_symlink_path(&path, "检查点文件")?;
    let content = fs::read_to_string(path).map_err(|e| format!("读取检查点失败: {}", e))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("检查点文件损坏: {}", e))?;
    if value.get("schema_version").and_then(|value| value.as_u64())
        == Some(u64::from(CHECKPOINT_SCHEMA_VERSION))
    {
        serde_json::from_value(value)
            .map(LoadedProjectCheckpoint::V2)
            .map(Some)
            .map_err(|e| format!("检查点文件损坏: {}", e))
    } else {
        serde_json::from_value(value)
            .map(LoadedProjectCheckpoint::LegacyV1)
            .map(Some)
            .map_err(|e| format!("旧版检查点文件损坏: {}", e))
    }
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
                schema_version: CHECKPOINT_SCHEMA_VERSION,
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
    fn checkpoint_status_marks_legacy_empty_checkpoint_as_not_restorable() {
        let project = temp_project("checkpoint-empty-legacy");
        init_git_repo(&project);
        let checkpoint = LegacyStoredProjectCheckpoint {
            id: "legacy-checkpoint".to_string(),
            created_at: 123,
            head: "unknown".to_string(),
            status: String::new(),
            diff_patch: String::new(),
            untracked_files: Vec::new(),
            skipped_untracked_files: Vec::new(),
        };
        fs::create_dir_all(checkpoint_dir(&project)).expect("create checkpoint dir");
        fs::write(
            checkpoint_file(&project),
            serde_json::to_vec_pretty(&checkpoint).expect("serialize legacy"),
        )
        .expect("write legacy checkpoint");

        let status = checkpoint_status(&project).expect("checkpoint status");

        assert!(!status.restorable);
        assert_eq!(status.message, "旧版检查点必须重新创建后才能恢复");
        let before = run_git(&project, &["status", "--porcelain=v2"]).expect("status before");
        let loaded = load_checkpoint(&project)
            .expect("load legacy")
            .expect("legacy checkpoint");
        let error = restore_checkpoint(&project, &loaded).expect_err("legacy restore refused");
        assert!(error.contains("旧版检查点"));
        assert_eq!(
            run_git(&project, &["status", "--porcelain=v2"]).expect("status after"),
            before
        );

        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn checkpoint_status_reports_untracked_snapshot_for_no_commit_repo() {
        let project = temp_project("checkpoint-untracked-status");
        init_git_repo(&project);
        fs::write(project.join("package.json"), "{\"name\":\"before\"}\n").expect("write package");
        let snapshot = capture_snapshot(&project).expect("snapshot worktree");
        let checkpoint = StoredProjectCheckpoint {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            id: "checkpoint-untracked".to_string(),
            created_at: 123,
            snapshot,
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

    fn sample_checkpoint() -> StoredProjectCheckpoint {
        StoredProjectCheckpoint {
            schema_version: CHECKPOINT_SCHEMA_VERSION,
            id: "checkpoint-1".to_string(),
            created_at: 123,
            snapshot: WorktreeSnapshot {
                schema_version: CHECKPOINT_SCHEMA_VERSION,
                head_oid: Some("a".repeat(40)),
                status_porcelain_v2: "1 .M src/App.tsx".to_string(),
                staged_patch: String::new(),
                unstaged_patch: String::new(),
                untracked_files: Vec::new(),
                unsupported_paths: Vec::new(),
            },
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
