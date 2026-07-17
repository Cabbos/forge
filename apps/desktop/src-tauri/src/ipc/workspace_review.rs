use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use crate::ipc::workspace::resolve_bound_working_dir;
use crate::state::AppState;

pub(crate) const MAX_REVIEW_PATCH_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceReviewFileStatus {
    Added,
    Modified,
    Renamed,
    Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct WorkspaceReviewFile {
    pub path: String,
    pub status: WorkspaceReviewFileStatus,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct WorkspaceReview {
    pub working_dir: String,
    pub patch: String,
    pub files: Vec<WorkspaceReviewFile>,
    pub truncated: bool,
}

pub(crate) fn collect_workspace_review(working_dir: &Path) -> Result<WorkspaceReview, String> {
    let output = Command::new("git")
        .args([
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--find-renames",
            "HEAD",
            "--",
        ])
        .current_dir(working_dir)
        .output()
        .map_err(|error| format!("无法读取当前改动: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "无法读取当前改动: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let truncated = output.stdout.len() > MAX_REVIEW_PATCH_BYTES;
    let patch_bytes = &output.stdout[..output.stdout.len().min(MAX_REVIEW_PATCH_BYTES)];
    let patch = String::from_utf8_lossy(patch_bytes).into_owned();
    let files = parse_review_files(&patch);

    Ok(WorkspaceReview {
        working_dir: working_dir.to_string_lossy().to_string(),
        patch,
        files,
        truncated,
    })
}

pub(crate) async fn workspace_review_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<WorkspaceReview, String> {
    let working_dir = resolve_bound_working_dir(state, session_id, working_dir).await?;
    tokio::task::spawn_blocking(move || collect_workspace_review(&working_dir))
        .await
        .map_err(|error| format!("读取当前改动的任务失败: {error}"))?
}

#[tauri::command]
pub async fn get_workspace_review(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<WorkspaceReview, String> {
    workspace_review_for_request(&state, session_id.as_deref(), working_dir.as_deref()).await
}

fn parse_review_files(patch: &str) -> Vec<WorkspaceReviewFile> {
    let mut files = Vec::new();
    let mut current: Option<WorkspaceReviewFile> = None;

    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("diff --git a/") {
            if let Some(file) = current.take() {
                files.push(file);
            }
            let path = header
                .split_once(" b/")
                .map(|(_, path)| path)
                .unwrap_or(header)
                .to_string();
            current = Some(WorkspaceReviewFile {
                path,
                status: WorkspaceReviewFileStatus::Modified,
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        let Some(file) = current.as_mut() else {
            continue;
        };

        if line.starts_with("new file mode ") {
            file.status = WorkspaceReviewFileStatus::Added;
        } else if line.starts_with("deleted file mode ") {
            file.status = WorkspaceReviewFileStatus::Deleted;
        } else if line.starts_with("rename from ") {
            file.status = WorkspaceReviewFileStatus::Renamed;
        } else if let Some(path) = line.strip_prefix("rename to ") {
            file.status = WorkspaceReviewFileStatus::Renamed;
            file.path = path.to_string();
        } else if line.starts_with('+') && !line.starts_with("+++") {
            file.additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            file.deletions += 1;
        }
    }

    if let Some(file) = current {
        files.push(file);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::session::AgentSession;
    use crate::harness::Harness;
    use crate::state::AppState;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;

    fn temp_git_repo(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-workspace-review-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&path).expect("workspace");
        git(&path, &["init"]);
        git(&path, &["config", "user.email", "forge@example.invalid"]);
        git(&path, &["config", "user.name", "Forge Test"]);
        std::fs::write(path.join("README.md"), "before\n").expect("baseline");
        git(&path, &["add", "README.md"]);
        git(&path, &["commit", "-m", "baseline"]);
        path
    }

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git command");
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn workspace_review_reports_current_file_stats_and_patch() {
        let workspace = temp_git_repo("stats");
        std::fs::write(workspace.join("README.md"), "after\nsecond\n").expect("change");

        let review = collect_workspace_review(&workspace).expect("review");

        assert_eq!(review.working_dir, workspace.to_string_lossy());
        assert_eq!(review.files.len(), 1);
        assert_eq!(review.files[0].path, "README.md");
        assert_eq!(review.files[0].additions, 2);
        assert_eq!(review.files[0].deletions, 1);
        assert!(review.patch.contains("+after"));
        assert!(!review.truncated);

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn workspace_review_caps_large_diff_output() {
        let workspace = temp_git_repo("bounded");
        std::fs::write(
            workspace.join("README.md"),
            "changed line\n".repeat(220_000),
        )
        .expect("large change");

        let review = collect_workspace_review(&workspace).expect("review");

        assert!(review.truncated);
        assert!(review.patch.len() <= MAX_REVIEW_PATCH_BYTES);

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn workspace_review_uses_session_workspace_over_explicit_workspace() {
        let session_workspace = temp_git_repo("session-bound");
        let explicit_workspace = temp_git_repo("explicit-ignored");
        std::fs::write(session_workspace.join("README.md"), "session change\n")
            .expect("session change");
        std::fs::write(explicit_workspace.join("README.md"), "explicit change\n")
            .expect("explicit change");
        let state = Arc::new(AppState::new(Arc::new(Harness::new(
            explicit_workspace.clone(),
        ))));
        let session = Arc::new(AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            Arc::new(Harness::new(session_workspace.clone())),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;

        let review =
            workspace_review_for_request(&state, Some("session-1"), explicit_workspace.to_str())
                .await
                .expect("review");

        assert_eq!(
            PathBuf::from(review.working_dir)
                .canonicalize()
                .expect("review workspace"),
            session_workspace.canonicalize().expect("session workspace")
        );
        assert!(review.patch.contains("session change"));
        assert!(!review.patch.contains("explicit change"));

        let _ = std::fs::remove_dir_all(session_workspace);
        let _ = std::fs::remove_dir_all(explicit_workspace);
    }
}
