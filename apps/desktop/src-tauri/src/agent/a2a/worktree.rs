use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// Maximum diff size to return to the parent model (chars). Larger diffs are
/// truncated with a marker so they do not explode the context window.
const MAX_DIFF_CHARS: usize = 50_000;

/// Process-level registry that prevents concurrent creation of worktrees with
/// the same branch name. Each lease holds a guard that releases the entry on
/// drop so the path remains diagnostic-friendly.
#[derive(Debug)]
pub(crate) struct WorktreeLeaseRegistry {
    active: Mutex<HashSet<String>>,
}

impl WorktreeLeaseRegistry {
    pub(crate) fn global() -> &'static Self {
        static INSTANCE: OnceLock<WorktreeLeaseRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            active: Mutex::new(HashSet::new()),
        })
    }

    /// Try to reserve a branch name. Returns `None` if already held.
    pub(crate) fn try_acquire(&self, branch_name: &str) -> Option<WorktreeLeaseGuard> {
        let mut active = self.active.lock().expect("worktree registry lock poisoned");
        if active.contains(branch_name) {
            return None;
        }
        active.insert(branch_name.to_string());
        Some(WorktreeLeaseGuard {
            branch_name: branch_name.to_string(),
        })
    }

    fn release(&self, branch_name: &str) {
        let mut active = self.active.lock().expect("worktree registry lock poisoned");
        active.remove(branch_name);
    }
}

/// Guard that releases the branch name from the global registry when dropped.
#[derive(Debug)]
pub(crate) struct WorktreeLeaseGuard {
    branch_name: String,
}

impl Drop for WorktreeLeaseGuard {
    fn drop(&mut self) {
        WorktreeLeaseRegistry::global().release(&self.branch_name);
    }
}

/// Lease for an isolated git worktree used by a WorktreeWorker A2A child agent.
///
/// The worktree is created at a predictable path under `<repo-root>/.claude/worktrees/`
/// and removed when the lease is dropped, unless `preserve` is set.
#[derive(Debug)]
pub(crate) struct WorktreeLease {
    repo_root: PathBuf,
    worktree_path: PathBuf,
    branch_name: String,
    preserve: bool,
    cleaned_up: bool,
    _registry_guard: WorktreeLeaseGuard,
}

/// Outcome of attempting to create a worktree lease.
#[derive(Debug)]
pub(crate) enum LeaseResult {
    Ok(WorktreeLease),
    NotAGitRepo { path: PathBuf },
    GitError { message: String },
    AlreadyInUse { branch_name: String },
}

/// Structured test report extracted from the worker result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestReport {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub exit_code: Option<i32>,
    pub summary: String,
}

/// Summary of what a WorktreeWorker produced, returned to the parent model.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorktreeWorkerSummary {
    pub result: String,
    pub diff: Option<String>,
    pub diff_available: bool,
    pub diff_truncated: bool,
    pub test_report: Option<String>,
    pub tests_passed: Option<bool>,
    pub needs_human_review: bool,
    pub suggested_action: String,
    pub worktree_path: String,
    pub cleaned_up: bool,
}

impl WorktreeLease {
    /// Attempt to create a worktree for the given task.
    ///
    /// * `working_dir` — the agent's current working directory (need not be repo root).
    /// * `task_id` — used to build a unique, predictable worktree path.
    pub(crate) fn create(working_dir: &Path, task_id: &str) -> LeaseResult {
        let repo_root = match git_repo_root(working_dir) {
            Some(root) => root,
            None => {
                return LeaseResult::NotAGitRepo {
                    path: working_dir.to_path_buf(),
                };
            }
        };

        let sanitized = sanitize_task_id(task_id);
        let branch_name = format!("a2a-worktree-{sanitized}");
        let worktree_path = repo_root
            .join(".claude")
            .join("worktrees")
            .join(&branch_name);

        // Acquire process-level lock to prevent concurrent creation of the same branch.
        let registry = WorktreeLeaseRegistry::global();
        let Some(registry_guard) = registry.try_acquire(&branch_name) else {
            return LeaseResult::AlreadyInUse {
                branch_name: branch_name.clone(),
            };
        };

        // Safety: ensure the resolved path stays inside the repo.
        if !is_path_inside(&worktree_path, &repo_root) {
            return LeaseResult::GitError {
                message: format!(
                    "worktree path {} escapes repo root {}",
                    worktree_path.display(),
                    repo_root.display()
                ),
            };
        }

        // Clean up any stale worktree at the same path before creating.
        let _ = remove_worktree(&repo_root, &worktree_path);
        let _ = std::fs::remove_dir_all(&worktree_path);
        let _ = delete_branch(&repo_root, &branch_name);

        // Create the worktree on a new detached branch so the worker has a clean HEAD.
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &branch_name,
                worktree_path.to_str().unwrap_or(""),
            ])
            .current_dir(&repo_root)
            .output();

        match output {
            Ok(out) if out.status.success() => LeaseResult::Ok(WorktreeLease {
                repo_root,
                worktree_path,
                branch_name,
                preserve: false,
                cleaned_up: false,
                _registry_guard: registry_guard,
            }),
            Ok(out) => LeaseResult::GitError {
                message: format!(
                    "git worktree add failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            },
            Err(e) => LeaseResult::GitError {
                message: format!("git worktree add failed: {e}"),
            },
        }
    }

    /// Path to the isolated worktree directory.
    pub(crate) fn path(&self) -> &Path {
        &self.worktree_path
    }

    /// Path to the repository root.
    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Mark the worktree to be preserved on drop (for diagnostics).
    pub(crate) fn preserve(&mut self) {
        self.preserve = true;
    }

    /// Returns `true` if the worktree will be preserved after drop.
    pub(crate) fn is_preserved(&self) -> bool {
        self.preserve
    }

    /// Compute a git diff of all changes inside the worktree, including
    /// modifications to tracked files and untracked files.
    pub(crate) fn diff(&self) -> Result<String, String> {
        // Diff for tracked modifications.
        let tracked = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(&self.worktree_path)
            .output()
            .map_err(|e| format!("git diff failed: {e}"))?;

        if !tracked.status.success() {
            return Err(format!(
                "git diff failed: {}",
                String::from_utf8_lossy(&tracked.stderr)
            ));
        }

        let mut result = String::from_utf8_lossy(&tracked.stdout).to_string();

        // List untracked files.
        let untracked = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&self.worktree_path)
            .output()
            .map_err(|e| format!("git ls-files failed: {e}"))?;

        if untracked.status.success() {
            let untracked_files = String::from_utf8_lossy(&untracked.stdout);
            if !untracked_files.trim().is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str("\n## Untracked files:\n");
                for line in untracked_files.lines() {
                    result.push_str(&format!("  {line}\n"));
                }
            }
        }

        Ok(result)
    }

    /// Compute a diff and truncate if it exceeds the context-window-safe limit.
    /// The truncation marker includes the original size so the caller knows
    /// how much was elided.
    pub(crate) fn diff_truncated(&self) -> Result<String, String> {
        let full = self.diff()?;
        if full.len() <= MAX_DIFF_CHARS {
            Ok(full)
        } else {
            let mut truncated = full.chars().take(MAX_DIFF_CHARS).collect::<String>();
            truncated.push_str(&format!(
                "\n\n[diff truncated: {} total chars, {} shown]",
                full.len(),
                MAX_DIFF_CHARS
            ));
            Ok(truncated)
        }
    }

    /// Remove the worktree and the temporary branch.
    pub(crate) fn cleanup(&mut self) -> Result<(), String> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;

        remove_worktree(&self.repo_root, &self.worktree_path)?;

        // Delete the branch from the main repo.
        let _ = delete_branch(&self.repo_root, &self.branch_name);

        Ok(())
    }
}

impl Drop for WorktreeLease {
    fn drop(&mut self) {
        if !self.preserve && !self.cleaned_up {
            let _ = self.cleanup();
        }
    }
}

fn git_repo_root(working_dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(working_dir)
        .output()
        .ok()?;

    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(PathBuf::from(root))
    } else {
        None
    }
}

fn remove_worktree(repo_root: &Path, worktree_path: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            worktree_path.to_str().unwrap_or(""),
        ])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("git worktree remove failed: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        // Worktree may already be gone; that's fine.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("is not a working tree") || stderr.contains("No such file") {
            Ok(())
        } else {
            Err(format!("git worktree remove failed: {stderr}"))
        }
    }
}

fn sanitize_task_id(task_id: &str) -> String {
    let sanitized = task_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(64)
        .collect::<String>();
    if sanitized.is_empty() {
        "task".to_string()
    } else {
        sanitized
    }
}

fn delete_branch(repo_root: &Path, branch_name: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(["branch", "-D", branch_name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("git branch delete failed: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("branch") && stderr.contains("not found") {
            Ok(())
        } else {
            Err(format!("git branch delete failed: {stderr}"))
        }
    }
}

fn is_path_inside(path: &Path, base: &Path) -> bool {
    let canonical_path = path.canonicalize();
    let canonical_base = base.canonicalize();
    match (canonical_path, canonical_base) {
        (Ok(p), Ok(b)) => p.starts_with(&b),
        _ => {
            // Fallback: compare components
            let path_components: Vec<_> = path.components().collect();
            let base_components: Vec<_> = base.components().collect();
            path_components.len() >= base_components.len()
                && path_components[..base_components.len()] == base_components[..]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_special_chars() {
        assert_eq!(sanitize_task_id("a2a-task-1"), "a2a-task-1");
        assert_eq!(sanitize_task_id("task/1"), "task-1");
        assert_eq!(sanitize_task_id("task.1"), "task-1");
        assert_eq!(sanitize_task_id("task__1"), "task--1");
        assert_eq!(sanitize_task_id("中文 task"), "task");
        assert_eq!(sanitize_task_id("///"), "task");
    }

    #[test]
    fn sanitize_limits_branch_component_length() {
        let long = "a".repeat(200);
        assert_eq!(sanitize_task_id(&long).len(), 64);
    }

    #[test]
    fn is_path_inside_detects_escapes() {
        let base = PathBuf::from("/home/user/project");
        assert!(is_path_inside(&base.join(".claude/worktrees/foo"), &base));
        assert!(!is_path_inside(&PathBuf::from("/home/user/other"), &base));
        assert!(!is_path_inside(&PathBuf::from("/tmp"), &base));
    }

    #[test]
    fn lease_not_a_git_repo() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-test-not-git-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let result = WorktreeLease::create(&tmp, "task-1");
        assert!(
            matches!(result, LeaseResult::NotAGitRepo { .. }),
            "expected NotAGitRepo, got {:?}",
            result
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn worktree_lease_full_lifecycle() {
        // This test requires git to be installed and will create a real repo + worktree.
        let tmp = std::env::temp_dir().join(format!(
            "forge-test-worktree-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        // Init a git repo.
        let init = Command::new("git")
            .args(["init"])
            .current_dir(&tmp)
            .output()
            .expect("git init");
        assert!(init.status.success(), "git init failed");

        // Create an initial commit so HEAD exists.
        std::fs::write(tmp.join("README.md"), "# test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&tmp)
            .output();
        let commit = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&tmp)
            .output()
            .expect("git commit");
        assert!(
            commit.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );

        // Create the lease.
        let mut lease = match WorktreeLease::create(&tmp, "task-1") {
            LeaseResult::Ok(l) => l,
            other => panic!("expected Ok lease, got {:?}", other),
        };

        assert!(lease.path().exists(), "worktree path should exist");
        assert!(
            is_path_inside(lease.path(), &tmp),
            "worktree should be inside repo"
        );

        // Make a change in the worktree.
        std::fs::write(lease.path().join("new_file.rs"), "fn main() {}").unwrap();

        // Diff should reflect the change.
        let diff = lease.diff().expect("diff should succeed");
        assert!(
            diff.contains("new_file.rs"),
            "diff should mention new_file.rs"
        );

        // Clean up explicitly.
        lease.cleanup().expect("cleanup should succeed");
        assert!(
            !lease.path().exists(),
            "worktree path should be removed after cleanup"
        );

        // Second cleanup should be a no-op.
        assert!(lease.cleanup().is_ok());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn worktree_lease_drop_cleans_up() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-test-worktree-drop-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let init = Command::new("git")
            .args(["init"])
            .current_dir(&tmp)
            .output()
            .expect("git init");
        assert!(init.status.success());

        std::fs::write(tmp.join("README.md"), "# test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&tmp)
            .output();
        let commit = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&tmp)
            .output()
            .expect("git commit");
        assert!(commit.status.success());

        let path = {
            let lease = match WorktreeLease::create(&tmp, "drop-task") {
                LeaseResult::Ok(l) => l,
                other => panic!("expected Ok lease, got {:?}", other),
            };
            let p = lease.path().to_path_buf();
            assert!(p.exists());
            p
        };

        // After drop, the path should be gone.
        assert!(!path.exists(), "worktree should be cleaned up on drop");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn worktree_lease_preserve_prevents_cleanup() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-test-worktree-preserve-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let init = Command::new("git")
            .args(["init"])
            .current_dir(&tmp)
            .output()
            .expect("git init");
        assert!(init.status.success());

        std::fs::write(tmp.join("README.md"), "# test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&tmp)
            .output();
        let commit = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&tmp)
            .output()
            .expect("git commit");
        assert!(commit.status.success());

        let path = {
            let mut lease = match WorktreeLease::create(&tmp, "preserve-task") {
                LeaseResult::Ok(l) => l,
                other => panic!("expected Ok lease, got {:?}", other),
            };
            lease.preserve();
            let p = lease.path().to_path_buf();
            p
        };

        // After drop with preserve, the path should still exist.
        assert!(path.exists(), "worktree should be preserved");

        // Clean up manually.
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn diff_truncated_when_too_large() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-test-worktree-trunc-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let init = Command::new("git")
            .args(["init"])
            .current_dir(&tmp)
            .output()
            .expect("git init");
        assert!(init.status.success());

        std::fs::write(tmp.join("README.md"), "# test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&tmp)
            .output();
        let commit = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&tmp)
            .output()
            .expect("git commit");
        assert!(commit.status.success());

        let mut lease = match WorktreeLease::create(&tmp, "trunc-task") {
            LeaseResult::Ok(l) => l,
            other => panic!("expected Ok lease, got {:?}", other),
        };

        // Modify an existing tracked file with very large content so the diff exceeds the limit.
        let big_content = "a\n".repeat(MAX_DIFF_CHARS + 1000);
        std::fs::write(lease.path().join("README.md"), big_content).unwrap();

        let diff = lease
            .diff_truncated()
            .expect("diff_truncated should succeed");
        assert!(
            diff.contains("[diff truncated:"),
            "diff should contain truncation marker, got {} chars",
            diff.len()
        );
        assert!(
            diff.len() <= MAX_DIFF_CHARS + 200,
            "diff should be near the limit, got {} chars",
            diff.len()
        );

        // Non-truncated diff should still return full content.
        let full = lease.diff().expect("diff should succeed");
        assert!(
            full.len() > MAX_DIFF_CHARS,
            "full diff should exceed limit, got {} chars",
            full.len()
        );

        let _ = lease.cleanup();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lease_registry_prevents_duplicate_branch() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-test-worktree-registry-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let init = Command::new("git")
            .args(["init"])
            .current_dir(&tmp)
            .output()
            .expect("git init");
        assert!(init.status.success());

        std::fs::write(tmp.join("README.md"), "# test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&tmp)
            .output();
        let commit = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&tmp)
            .output()
            .expect("git commit");
        assert!(commit.status.success());

        // Create first lease.
        let lease1 = match WorktreeLease::create(&tmp, "registry-task") {
            LeaseResult::Ok(l) => l,
            other => panic!("expected Ok lease, got {:?}", other),
        };
        assert!(lease1.path().exists());

        // Second creation with the same task id should fail with AlreadyInUse.
        let result2 = WorktreeLease::create(&tmp, "registry-task");
        assert!(
            matches!(result2, LeaseResult::AlreadyInUse { .. }),
            "expected AlreadyInUse, got {:?}",
            result2
        );

        // After dropping the first lease, creation should succeed again.
        let path1 = lease1.path().to_path_buf();
        drop(lease1);
        assert!(!path1.exists(), "worktree should be cleaned up on drop");

        let mut lease2 = match WorktreeLease::create(&tmp, "registry-task") {
            LeaseResult::Ok(l) => l,
            other => panic!("expected Ok lease after drop, got {:?}", other),
        };
        assert!(lease2.path().exists());

        let _ = lease2.cleanup();
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
