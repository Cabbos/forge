use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};

pub(crate) const CHECKPOINT_SCHEMA_VERSION: u32 = 2;
pub(crate) const MAX_SNAPSHOT_FILE_BYTES: u64 = 512 * 1024;
pub(crate) const MAX_SNAPSHOT_TOTAL_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WorktreeSnapshot {
    pub(crate) schema_version: u32,
    pub(crate) head_oid: Option<String>,
    pub(crate) status_porcelain_v2: String,
    pub(crate) staged_patch: String,
    pub(crate) unstaged_patch: String,
    pub(crate) untracked_files: Vec<SnapshotFile>,
    pub(crate) unsupported_paths: Vec<UnsupportedCheckpointPath>,
}

impl WorktreeSnapshot {
    pub(crate) fn is_restorable(&self) -> bool {
        self.schema_version == CHECKPOINT_SCHEMA_VERSION && self.unsupported_paths.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SnapshotFile {
    pub(crate) relative_path: String,
    pub(crate) bytes_base64: String,
    pub(crate) executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct UnsupportedCheckpointPath {
    pub(crate) relative_path: String,
    pub(crate) reason: String,
}

pub(crate) fn capture_snapshot(working_dir: &Path) -> Result<WorktreeSnapshot, String> {
    ensure_git_repo(working_dir)?;
    let head_oid = git_head_oid(working_dir)?;
    let status_porcelain_v2 = run_git_utf8(
        working_dir,
        &[
            "status",
            "--porcelain=v2",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).forge/**",
        ],
    )?;
    let staged_patch = run_git_utf8(
        working_dir,
        &[
            "diff",
            "--cached",
            "--binary",
            "--full-index",
            "--",
            ".",
            ":(exclude).forge/**",
        ],
    )?;
    let unstaged_patch = run_git_utf8(
        working_dir,
        &[
            "diff",
            "--binary",
            "--full-index",
            "--",
            ".",
            ":(exclude).forge/**",
        ],
    )?;
    let (untracked_files, unsupported_paths) = capture_untracked_files(working_dir)?;

    Ok(WorktreeSnapshot {
        schema_version: CHECKPOINT_SCHEMA_VERSION,
        head_oid,
        status_porcelain_v2,
        staged_patch,
        unstaged_patch,
        untracked_files,
        unsupported_paths,
    })
}

fn ensure_git_repo(working_dir: &Path) -> Result<(), String> {
    let inside = run_git_utf8(working_dir, &["rev-parse", "--is-inside-work-tree"])?;
    if inside.trim() == "true" {
        Ok(())
    } else {
        Err("checkpoint workspace is not a Git worktree".to_string())
    }
}

fn git_head_oid(working_dir: &Path) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(working_dir)
        .output()
        .map_err(|_| "failed to inspect checkpoint HEAD".to_string())?;
    if output.status.success() {
        let oid = String::from_utf8(output.stdout)
            .map_err(|_| "checkpoint HEAD is not valid UTF-8".to_string())?;
        Ok(Some(oid.trim().to_string()))
    } else {
        Ok(None)
    }
}

fn capture_untracked_files(
    working_dir: &Path,
) -> Result<(Vec<SnapshotFile>, Vec<UnsupportedCheckpointPath>), String> {
    let output = run_git_bytes(
        working_dir,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            ".",
            ":(exclude).forge/**",
        ],
    )?;
    let mut files = Vec::new();
    let mut unsupported = Vec::new();
    let mut total_bytes = 0_u64;

    for raw_path in output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let relative_path = match String::from_utf8(raw_path.to_vec()) {
            Ok(path) => path,
            Err(_) => {
                unsupported.push(UnsupportedCheckpointPath {
                    relative_path: "[non-utf8-path]".to_string(),
                    reason: "non_utf8_path".to_string(),
                });
                continue;
            }
        };
        if let Err(reason) = validate_relative_path(&relative_path) {
            unsupported.push(UnsupportedCheckpointPath {
                relative_path,
                reason,
            });
            continue;
        }
        let path = working_dir.join(&relative_path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                unsupported.push(UnsupportedCheckpointPath {
                    relative_path,
                    reason: "metadata_unavailable".to_string(),
                });
                continue;
            }
        };
        let reason = if metadata.file_type().is_symlink() {
            Some("symlink")
        } else if !metadata.is_file() {
            Some("not_regular_file")
        } else if metadata.len() > MAX_SNAPSHOT_FILE_BYTES {
            Some("file_too_large")
        } else if total_bytes.saturating_add(metadata.len()) > MAX_SNAPSHOT_TOTAL_BYTES {
            Some("snapshot_too_large")
        } else {
            None
        };
        if let Some(reason) = reason {
            unsupported.push(UnsupportedCheckpointPath {
                relative_path,
                reason: reason.to_string(),
            });
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                unsupported.push(UnsupportedCheckpointPath {
                    relative_path,
                    reason: "read_failed".to_string(),
                });
                continue;
            }
        };
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        files.push(SnapshotFile {
            relative_path,
            bytes_base64: general_purpose::STANDARD.encode(bytes),
            executable: is_executable(&metadata),
        });
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    unsupported.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok((files, unsupported))
}

fn validate_relative_path(relative_path: &str) -> Result<(), String> {
    let path = Path::new(relative_path);
    if relative_path.is_empty() || path.is_absolute() {
        return Err("invalid_path".to_string());
    }
    if relative_path == ".forge" || relative_path.starts_with(".forge/") {
        return Err("internal_path".to_string());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("path_escape".to_string());
    }
    Ok(())
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

fn run_git_utf8(working_dir: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(run_git_bytes(working_dir, args)?)
        .map_err(|_| format!("git {} returned non-UTF-8 output", args.join(" ")))
}

fn run_git_bytes(working_dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .map_err(|_| format!("failed to run git {}", args.join(" ")))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(if stderr.trim().is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            stderr.trim().to_string()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestoreFailurePoint {
    None,
    AfterStagedPatch,
    BeforeFirstUntrackedWrite,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestoreFailureInjection {
    AfterStagedPatch,
    BeforeFirstUntrackedWrite,
}

pub(crate) fn restore_snapshot_transactional(
    working_dir: &Path,
    checkpoint: &WorktreeSnapshot,
) -> Result<(), String> {
    restore_snapshot_transactional_internal(working_dir, checkpoint, RestoreFailurePoint::None)
}

#[cfg(test)]
pub(crate) fn restore_snapshot_transactional_with_injection(
    working_dir: &Path,
    checkpoint: &WorktreeSnapshot,
    injection: RestoreFailureInjection,
) -> Result<(), String> {
    let injection = match injection {
        RestoreFailureInjection::AfterStagedPatch => RestoreFailurePoint::AfterStagedPatch,
        RestoreFailureInjection::BeforeFirstUntrackedWrite => {
            RestoreFailurePoint::BeforeFirstUntrackedWrite
        }
    };
    restore_snapshot_transactional_internal(working_dir, checkpoint, injection)
}

fn restore_snapshot_transactional_internal(
    working_dir: &Path,
    checkpoint: &WorktreeSnapshot,
    injection: RestoreFailurePoint,
) -> Result<(), String> {
    validate_snapshot(checkpoint)?;
    if !checkpoint.unsupported_paths.is_empty() {
        return Err(
            "checkpoint contains unsupported paths; recreate it before restore".to_string(),
        );
    }
    let current_head = git_head_oid(working_dir)?;
    if current_head != checkpoint.head_oid {
        return Err("checkpoint HEAD does not match the current worktree HEAD".to_string());
    }

    let rollback = capture_snapshot(working_dir)?;
    validate_snapshot(&rollback)?;
    if !rollback.unsupported_paths.is_empty() {
        return Err("current worktree contains unsupported paths; restore refused".to_string());
    }

    match materialize_snapshot(working_dir, checkpoint, &[&rollback], injection) {
        Ok(()) => Ok(()),
        Err(original_error) => {
            let rollback_result = materialize_snapshot(
                working_dir,
                &rollback,
                &[checkpoint, &rollback],
                RestoreFailurePoint::None,
            );
            match rollback_result {
                Ok(()) => Err(format!("{original_error}; rollback=restored")),
                Err(rollback_error) => Err(format!(
                    "{original_error}; rollback=failed ({rollback_error})"
                )),
            }
        }
    }
}

fn validate_snapshot(snapshot: &WorktreeSnapshot) -> Result<(), String> {
    if snapshot.schema_version != CHECKPOINT_SCHEMA_VERSION {
        return Err("legacy checkpoint must be recreated before restore".to_string());
    }
    if let Some(head_oid) = &snapshot.head_oid {
        if !(40..=64).contains(&head_oid.len())
            || !head_oid.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("checkpoint HEAD is invalid".to_string());
        }
    }
    let mut seen = HashSet::new();
    let mut total_bytes = 0_u64;
    for file in &snapshot.untracked_files {
        validate_relative_path(&file.relative_path)
            .map_err(|_| "checkpoint contains an invalid path".to_string())?;
        if !seen.insert(file.relative_path.as_str()) {
            return Err("checkpoint contains duplicate untracked paths".to_string());
        }
        let bytes = general_purpose::STANDARD
            .decode(&file.bytes_base64)
            .map_err(|_| "checkpoint contains invalid base64".to_string())?;
        if bytes.len() as u64 > MAX_SNAPSHOT_FILE_BYTES {
            return Err("checkpoint untracked file exceeds the size limit".to_string());
        }
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        if total_bytes > MAX_SNAPSHOT_TOTAL_BYTES {
            return Err("checkpoint untracked files exceed the total size limit".to_string());
        }
    }
    for unsupported in &snapshot.unsupported_paths {
        validate_relative_path(&unsupported.relative_path)
            .map_err(|_| "checkpoint contains an invalid unsupported path".to_string())?;
    }
    Ok(())
}

fn materialize_snapshot(
    working_dir: &Path,
    target: &WorktreeSnapshot,
    removal_snapshots: &[&WorktreeSnapshot],
    injection: RestoreFailurePoint,
) -> Result<(), String> {
    reset_tracked_state(working_dir, target.head_oid.is_some())?;
    remove_captured_untracked_paths(working_dir, removal_snapshots)?;

    apply_snapshot_patch(working_dir, &target.staged_patch, true)?;
    if injection == RestoreFailurePoint::AfterStagedPatch {
        return Err("injected failure after staged patch".to_string());
    }
    apply_snapshot_patch(working_dir, &target.unstaged_patch, false)?;

    let mut first_untracked = true;
    for file in &target.untracked_files {
        if first_untracked && injection == RestoreFailurePoint::BeforeFirstUntrackedWrite {
            return Err("injected failure before untracked restore".to_string());
        }
        first_untracked = false;
        restore_snapshot_file(working_dir, file)?;
    }

    let restored = capture_snapshot(working_dir)?;
    if restored != *target {
        return Err("restored checkpoint does not match captured state".to_string());
    }
    Ok(())
}

fn reset_tracked_state(working_dir: &Path, has_head: bool) -> Result<(), String> {
    if has_head {
        run_git_utf8(working_dir, &["reset", "--hard", "HEAD"])?;
        return Ok(());
    }

    let indexed = run_git_bytes(working_dir, &["ls-files", "-z"])?;
    let indexed_paths = indexed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map_err(|_| "indexed checkpoint path is not UTF-8".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    run_git_utf8(working_dir, &["read-tree", "--empty"])?;
    for relative_path in indexed_paths {
        remove_captured_path(working_dir, &relative_path)?;
    }
    Ok(())
}

fn remove_captured_untracked_paths(
    working_dir: &Path,
    snapshots: &[&WorktreeSnapshot],
) -> Result<(), String> {
    let mut paths = HashSet::new();
    for snapshot in snapshots {
        for file in &snapshot.untracked_files {
            paths.insert(file.relative_path.as_str());
        }
    }
    for relative_path in paths {
        remove_captured_path(working_dir, relative_path)?;
    }
    Ok(())
}

fn remove_captured_path(working_dir: &Path, relative_path: &str) -> Result<(), String> {
    validate_relative_path(relative_path)
        .map_err(|_| "refused to remove an invalid checkpoint path".to_string())?;
    let path = working_dir.join(relative_path);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(path)
                .map_err(|_| format!("failed to remove captured path {relative_path}"))
        }
        Ok(_) => Err(format!("captured path is not a file: {relative_path}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(format!("failed to inspect captured path {relative_path}")),
    }
}

fn apply_snapshot_patch(working_dir: &Path, patch: &str, staged: bool) -> Result<(), String> {
    if patch.trim().is_empty() {
        return Ok(());
    }
    let mut args = vec!["apply", "--binary"];
    if staged {
        args.push("--index");
    }
    args.extend(["--whitespace=nowarn", "-"]);
    let mut child = Command::new("git")
        .args(&args)
        .current_dir(working_dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "failed to start git apply".to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "git apply stdin unavailable".to_string())?
        .write_all(patch.as_bytes())
        .map_err(|_| "failed to write checkpoint patch".to_string())?;
    let output = child
        .wait_with_output()
        .map_err(|_| "failed to wait for git apply".to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(if stderr.trim().is_empty() {
            "git apply failed".to_string()
        } else {
            format!("git apply failed: {}", stderr.trim())
        })
    }
}

fn restore_snapshot_file(working_dir: &Path, file: &SnapshotFile) -> Result<(), String> {
    validate_relative_path(&file.relative_path)
        .map_err(|_| "checkpoint file path is invalid".to_string())?;
    let path = working_dir.join(&file.relative_path);
    ensure_safe_parent_directories(working_dir, &file.relative_path)?;
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!(
            "checkpoint file target is a symlink: {}",
            file.relative_path
        ));
    }
    let bytes = general_purpose::STANDARD
        .decode(&file.bytes_base64)
        .map_err(|_| "checkpoint file base64 is invalid".to_string())?;
    fs::write(&path, bytes)
        .map_err(|_| format!("failed to restore checkpoint file {}", file.relative_path))?;
    set_executable(&path, file.executable)?;
    Ok(())
}

fn ensure_safe_parent_directories(working_dir: &Path, relative_path: &str) -> Result<(), String> {
    let mut current = working_dir.to_path_buf();
    let components = Path::new(relative_path).components().collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        let Component::Normal(component) = component else {
            return Err("checkpoint parent path is invalid".to_string());
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("checkpoint parent path is a symlink".to_string());
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err("checkpoint parent path is not a directory".to_string());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|_| "failed to create checkpoint parent directory".to_string())?;
            }
            Err(_) => return Err("failed to inspect checkpoint parent directory".to_string()),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::metadata(path).map_err(|_| "failed to inspect restored mode".to_string())?;
    let mut mode = metadata.permissions().mode();
    if executable {
        mode |= 0o111;
    } else {
        mode &= !0o111;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| "failed to restore executable mode".to_string())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn captures_staged_only_text_modification() {
        let repo = committed_repo("staged-only");
        fs::write(repo.join("app.txt"), "staged\n").expect("write staged");
        git(&repo, &["add", "app.txt"]);

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert!(snapshot.staged_patch.contains("+staged"));
        assert!(snapshot.unstaged_patch.is_empty());
        assert!(snapshot.status_porcelain_v2.contains("1 M."));
        cleanup(&repo);
    }

    #[test]
    fn captures_unstaged_only_text_modification() {
        let repo = committed_repo("unstaged-only");
        fs::write(repo.join("app.txt"), "unstaged\n").expect("write unstaged");

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert!(snapshot.staged_patch.is_empty());
        assert!(snapshot.unstaged_patch.contains("+unstaged"));
        assert!(snapshot.status_porcelain_v2.contains("1 .M"));
        cleanup(&repo);
    }

    #[test]
    fn captures_staged_and_unstaged_changes_to_same_file() {
        let repo = committed_repo("staged-and-unstaged");
        fs::write(repo.join("app.txt"), "staged\n").expect("write staged");
        git(&repo, &["add", "app.txt"]);
        fs::write(repo.join("app.txt"), "staged\nunstaged\n").expect("write unstaged");

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert!(snapshot.staged_patch.contains("+staged"));
        assert!(snapshot.unstaged_patch.contains("+unstaged"));
        assert!(snapshot.status_porcelain_v2.contains("1 MM"));
        cleanup(&repo);
    }

    #[test]
    fn captures_staged_rename_and_deletion() {
        let repo = committed_repo("rename-delete");
        fs::write(repo.join("remove.txt"), "remove\n").expect("write remove fixture");
        git(&repo, &["add", "remove.txt"]);
        git(&repo, &["commit", "-m", "add remove fixture"]);
        git(&repo, &["mv", "app.txt", "renamed.txt"]);
        fs::remove_file(repo.join("remove.txt")).expect("remove tracked file");
        git(&repo, &["add", "-A"]);

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert!(snapshot.staged_patch.contains("renamed.txt"));
        assert!(snapshot.staged_patch.contains("deleted file mode"));
        assert!(snapshot.status_porcelain_v2.contains("2 R."));
        assert!(snapshot.status_porcelain_v2.contains("1 D."));
        cleanup(&repo);
    }

    #[test]
    fn captures_tracked_binary_modification_with_binary_patch() {
        let repo = temp_repo("tracked-binary");
        init_repo(&repo);
        fs::write(repo.join("image.bin"), [0, 1, 2, 3]).expect("write binary base");
        git(&repo, &["add", "image.bin"]);
        git(&repo, &["commit", "-m", "binary base"]);
        fs::write(repo.join("image.bin"), [0, 9, 8, 7, 6]).expect("modify binary");

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert!(snapshot.unstaged_patch.contains("GIT binary patch"));
        cleanup(&repo);
    }

    #[cfg(unix)]
    #[test]
    fn captures_untracked_binary_bytes_and_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let repo = committed_repo("untracked-binary");
        let bytes = [0, 159, 146, 150, 255];
        let path = repo.join("tool.bin");
        fs::write(&path, bytes).expect("write untracked binary");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("set executable");

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");
        let file = snapshot
            .untracked_files
            .iter()
            .find(|file| file.relative_path == "tool.bin")
            .expect("untracked snapshot");

        assert_eq!(
            general_purpose::STANDARD
                .decode(&file.bytes_base64)
                .expect("decode snapshot"),
            bytes
        );
        assert!(file.executable);
        cleanup(&repo);
    }

    #[test]
    fn captures_unborn_repository_without_head() {
        let repo = temp_repo("unborn");
        init_repo(&repo);
        fs::write(repo.join("new.txt"), "new\n").expect("write untracked");

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert_eq!(snapshot.schema_version, CHECKPOINT_SCHEMA_VERSION);
        assert_eq!(snapshot.head_oid, None);
        assert_eq!(snapshot.untracked_files.len(), 1);
        cleanup(&repo);
    }

    #[cfg(unix)]
    #[test]
    fn records_symlink_and_oversized_untracked_file_as_unsupported() {
        use std::os::unix::fs as unix_fs;

        let repo = committed_repo("unsupported");
        fs::write(
            repo.join("large.bin"),
            vec![0_u8; MAX_SNAPSHOT_FILE_BYTES as usize + 1],
        )
        .expect("write large file");
        unix_fs::symlink("app.txt", repo.join("link.txt")).expect("create symlink");

        let snapshot = capture_snapshot(&repo).expect("capture snapshot");

        assert!(snapshot
            .unsupported_paths
            .iter()
            .any(|item| item.relative_path == "large.bin" && item.reason == "file_too_large"));
        assert!(snapshot
            .unsupported_paths
            .iter()
            .any(|item| item.relative_path == "link.txt" && item.reason == "symlink"));
        assert!(!snapshot.is_restorable());
        cleanup(&repo);
    }

    #[cfg(unix)]
    #[test]
    fn transactional_restore_round_trips_combined_text_binary_rename_delete_and_untracked() {
        use std::os::unix::fs::PermissionsExt;

        let repo = committed_repo("roundtrip-combined");
        fs::write(repo.join("delete.txt"), "delete\n").expect("write delete base");
        fs::write(repo.join("image.bin"), [0, 1, 2]).expect("write binary base");
        git(&repo, &["add", "delete.txt", "image.bin"]);
        git(&repo, &["commit", "-m", "fixtures"]);
        git(&repo, &["mv", "app.txt", "renamed.txt"]);
        fs::remove_file(repo.join("delete.txt")).expect("delete tracked");
        git(&repo, &["add", "-A"]);
        fs::write(repo.join("renamed.txt"), "base\nunstaged\n").expect("unstaged rename");
        fs::write(repo.join("image.bin"), [0, 9, 8, 7]).expect("modify binary");
        let untracked = repo.join("tool.bin");
        fs::write(&untracked, [0, 255, 1, 254]).expect("write untracked");
        fs::set_permissions(&untracked, fs::Permissions::from_mode(0o755)).expect("set executable");
        let checkpoint = capture_snapshot(&repo).expect("capture checkpoint A");

        git(&repo, &["reset", "--hard", "HEAD"]);
        fs::remove_file(&untracked).expect("remove old untracked");
        fs::write(repo.join("app.txt"), "state-b\n").expect("write state B");
        let state_b = capture_snapshot(&repo).expect("capture B");
        assert_ne!(state_b.status_porcelain_v2, checkpoint.status_porcelain_v2);

        restore_snapshot_transactional(&repo, &checkpoint).expect("restore A");

        assert_eq!(
            capture_snapshot(&repo).expect("capture restored A"),
            checkpoint
        );
        cleanup(&repo);
    }

    #[test]
    fn transactional_restore_round_trips_unborn_repository() {
        let repo = temp_repo("roundtrip-unborn");
        init_repo(&repo);
        fs::write(repo.join("first.txt"), "first\n").expect("write first");
        git(&repo, &["add", "first.txt"]);
        fs::write(repo.join("second.txt"), "second\n").expect("write second");
        let checkpoint = capture_snapshot(&repo).expect("capture checkpoint A");
        fs::write(repo.join("first.txt"), "state-b\n").expect("mutate first");
        fs::write(repo.join("third.txt"), "third\n").expect("write third");

        restore_snapshot_transactional(&repo, &checkpoint).expect("restore unborn A");

        assert_eq!(
            capture_snapshot(&repo).expect("capture restored A"),
            checkpoint
        );
        cleanup(&repo);
    }

    #[cfg(unix)]
    #[test]
    fn restore_refuses_unsupported_snapshot_before_mutation() {
        use std::os::unix::fs as unix_fs;

        let repo = committed_repo("refuse-unsupported");
        unix_fs::symlink("app.txt", repo.join("link.txt")).expect("create symlink");
        let checkpoint = capture_snapshot(&repo).expect("capture unsupported A");
        fs::write(repo.join("app.txt"), "state-b\n").expect("write B");
        let before = capture_snapshot(&repo).expect("capture before refusal");

        let error = restore_snapshot_transactional(&repo, &checkpoint)
            .expect_err("unsupported checkpoint must be refused");

        assert!(error.contains("unsupported"));
        assert_eq!(
            capture_snapshot(&repo).expect("capture after refusal"),
            before
        );
        cleanup(&repo);
    }

    #[test]
    fn restore_refuses_head_drift_before_mutation() {
        let repo = committed_repo("refuse-head-drift");
        fs::write(repo.join("app.txt"), "checkpoint\n").expect("write checkpoint state");
        let checkpoint = capture_snapshot(&repo).expect("capture A");
        git(&repo, &["add", "app.txt"]);
        git(&repo, &["commit", "-m", "head drift"]);
        fs::write(repo.join("app.txt"), "state-b\n").expect("write B");
        let before = capture_snapshot(&repo).expect("capture before refusal");

        let error = restore_snapshot_transactional(&repo, &checkpoint)
            .expect_err("HEAD drift must be refused");

        assert!(error.contains("HEAD"));
        assert_eq!(
            capture_snapshot(&repo).expect("capture after refusal"),
            before
        );
        cleanup(&repo);
    }

    #[test]
    fn injected_staged_patch_failure_rolls_back_exact_state_b() {
        let repo = committed_repo("rollback-staged");
        fs::write(repo.join("app.txt"), "checkpoint\n").expect("write A");
        git(&repo, &["add", "app.txt"]);
        let checkpoint = capture_snapshot(&repo).expect("capture A");
        git(&repo, &["reset", "--hard", "HEAD"]);
        fs::write(repo.join("app.txt"), "state-b\n").expect("write B");
        fs::write(repo.join("b.txt"), "untracked-b\n").expect("write B untracked");
        let before = capture_snapshot(&repo).expect("capture B");

        let error = restore_snapshot_transactional_with_injection(
            &repo,
            &checkpoint,
            RestoreFailureInjection::AfterStagedPatch,
        )
        .expect_err("injected failure");

        assert!(error.contains("rollback=restored"));
        assert_eq!(capture_snapshot(&repo).expect("capture rollback B"), before);
        cleanup(&repo);
    }

    #[test]
    fn injected_untracked_restore_failure_rolls_back_exact_state_b() {
        let repo = committed_repo("rollback-untracked");
        fs::write(repo.join("a.txt"), "untracked-a\n").expect("write A untracked");
        let checkpoint = capture_snapshot(&repo).expect("capture A");
        fs::remove_file(repo.join("a.txt")).expect("remove A untracked");
        fs::write(repo.join("app.txt"), "state-b\n").expect("write B");
        fs::write(repo.join("b.txt"), "untracked-b\n").expect("write B untracked");
        let before = capture_snapshot(&repo).expect("capture B");

        let error = restore_snapshot_transactional_with_injection(
            &repo,
            &checkpoint,
            RestoreFailureInjection::BeforeFirstUntrackedWrite,
        )
        .expect_err("injected failure");

        assert!(error.contains("rollback=restored"));
        assert_eq!(capture_snapshot(&repo).expect("capture rollback B"), before);
        cleanup(&repo);
    }

    #[test]
    fn restore_never_removes_checkpoint_internal_uncaptured_path() {
        let repo = committed_repo("preserve-internal");
        fs::write(repo.join("app.txt"), "checkpoint\n").expect("write A");
        let checkpoint = capture_snapshot(&repo).expect("capture A");
        let internal = repo.join(".forge/manual/sentinel.txt");
        fs::create_dir_all(internal.parent().expect("internal parent")).expect("create internal");
        fs::write(&internal, "keep\n").expect("write internal sentinel");
        fs::write(repo.join("app.txt"), "state-b\n").expect("write B");

        restore_snapshot_transactional(&repo, &checkpoint).expect("restore A");

        assert_eq!(
            fs::read_to_string(internal).expect("read sentinel"),
            "keep\n"
        );
        cleanup(&repo);
    }

    fn committed_repo(name: &str) -> PathBuf {
        let repo = temp_repo(name);
        init_repo(&repo);
        fs::write(repo.join("app.txt"), "base\n").expect("write base");
        git(&repo, &["add", "app.txt"]);
        git(&repo, &["commit", "-m", "base"]);
        repo
    }

    fn temp_repo(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("forge-checkpoint-{name}-{}", uuid::Uuid::now_v7()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create repo");
        path
    }

    fn init_repo(path: &Path) {
        git(path, &["init"]);
        git(path, &["config", "user.email", "forge@example.test"]);
        git(path, &["config", "user.name", "Forge Test"]);
    }

    fn git(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}
