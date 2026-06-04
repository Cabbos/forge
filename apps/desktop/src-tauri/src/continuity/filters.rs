use crate::agent::turn_state::AgentToolCategory;

use super::episode::Episode;

/// Check whether a shell tool failure is actually a false positive (the command
/// succeeded but the tool wrapper reported it as failed).
pub(crate) fn shell_failure_is_false_positive(
    command: Option<&str>,
    result_summary: Option<&str>,
) -> bool {
    let Some(summary) = result_summary else {
        return false;
    };
    if shell_failure_summary_looks_successful(summary) {
        return true;
    }
    if let Some(cmd) = command {
        if shell_failure_looks_like_successful_inspection(cmd, summary) {
            return true;
        }
    }
    false
}

fn shell_failure_summary_looks_successful(summary: &str) -> bool {
    let lower = summary.to_lowercase();
    let has_error_marker = shell_summary_has_error_marker(summary);

    let looks_like_vite_build_success = lower.contains("vite ")
        && lower.contains("building")
        && lower.contains("rendering chunks")
        && lower.contains("computing gzip size")
        && lower.contains("✓ built in");

    let has_explicit_success_exit = shell_summary_has_success_exit_marker(&lower);
    let has_explicit_test_success = lower.contains("all tests passed")
        || (summary.contains("通过") && summary.contains("0 失败"));

    let looks_like_npm_test_success = lower.contains("npm test")
        || lower.contains("npx tsx")
        || lower.contains("npx jest")
        || lower.contains("npx vitest");
    let npm_output_shows_passing =
        looks_like_npm_test_success && lower.contains('✓') && lower.contains('=');

    (looks_like_vite_build_success
        || has_explicit_success_exit
        || has_explicit_test_success
        || npm_output_shows_passing)
        && !has_error_marker
}

fn shell_summary_has_success_exit_marker(lower_summary: &str) -> bool {
    let compact = lower_summary.split_whitespace().collect::<String>();
    lower_summary.contains("tsc_exit: 0")
        || lower_summary.contains("test_exit: 0")
        || lower_summary.contains("exit: 0")
        || lower_summary.contains("exit code: 0")
        || compact.contains("tsc_exit:0")
        || compact.contains("test_exit:0")
        || compact.contains("exit:0")
        || compact.contains("exit=0")
        || compact.contains("exitcode:0")
}

fn shell_failure_looks_like_successful_inspection(command: &str, summary: &str) -> bool {
    shell_command_is_read_only_inspection(command)
        && shell_summary_has_stdout(summary)
        && !shell_summary_has_error_marker(summary)
}

fn shell_command_is_read_only_inspection(command: &str) -> bool {
    let lower = command.to_lowercase();
    if shell_command_has_mutation_signal(&lower) {
        return false;
    }

    let has_filesystem_probe = contains_shell_word(&lower, "ls")
        || contains_shell_word(&lower, "file")
        || contains_shell_word(&lower, "wc")
        || contains_shell_word(&lower, "realpath")
        || contains_shell_word(&lower, "pwd")
        || contains_shell_word(&lower, "find")
        || contains_shell_word(&lower, "which");
    let has_git_probe =
        lower.contains("git status") || lower.contains("git diff") || lower.contains("git log");
    let has_sqlite_probe = lower.contains("sqlite3 ")
        && (lower.contains(".tables")
            || lower.contains(".schema")
            || lower.contains("select ")
            || lower.contains("pragma "));

    has_filesystem_probe || has_git_probe || has_sqlite_probe
}

fn shell_command_has_mutation_signal(lower_command: &str) -> bool {
    let mutating_words = [
        "rm", "mv", "cp", "mkdir", "touch", "tee", "chmod", "chown", "npm", "pnpm", "yarn",
        "cargo", "git",
    ];
    if mutating_words
        .iter()
        .any(|word| contains_shell_word(lower_command, word))
    {
        return !(lower_command.contains("git status")
            || lower_command.contains("git diff")
            || lower_command.contains("git log"));
    }

    lower_command.contains("sed -i")
        || lower_command.contains(" insert ")
        || lower_command.contains(" update ")
        || lower_command.contains(" delete ")
        || lower_command.contains(" drop ")
        || lower_command.contains(" create ")
        || lower_command.contains(" alter ")
}

fn contains_shell_word(command: &str, word: &str) -> bool {
    command
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .any(|part| part == word)
}

fn shell_summary_has_stdout(summary: &str) -> bool {
    let lower = summary.to_lowercase();
    let Some(stdout_index) = lower.find("stdout:") else {
        return false;
    };
    let stdout_start = stdout_index + "stdout:".len();
    let stderr_index = lower[stdout_start..]
        .find("stderr:")
        .map(|index| stdout_start + index)
        .unwrap_or(summary.len());
    summary[stdout_start..stderr_index].trim().len() >= 3
}

fn shell_summary_has_error_marker(summary: &str) -> bool {
    let lower = summary.to_lowercase();
    lower.contains("error:")
        || lower.contains(" failed")
        || lower.contains("failed ")
        || lower.contains("panic")
        || lower.contains("not found")
        || lower.contains("cannot find")
        || lower.contains("no such file")
        || lower.contains("permission denied")
}

/// Heuristic: is this episode a debugging-only interaction that should not
/// produce reusable experiences?
pub(crate) fn is_debugging_only_episode(episode: &Episode) -> bool {
    // If the only file changes are to .forge/continuity.db or test fixtures,
    // and there are no real code changes, this is likely a debugging interaction.
    let has_real_code_change = episode.file_changes.iter().any(|fc| {
        let p = fc.path.to_lowercase();
        !p.contains("continuity.db")
            && !p.contains(".forge/")
            && !p.ends_with("_test.rs")
            && !p.ends_with(".test.ts")
            && !p.ends_with(".test.tsx")
            && !p.ends_with(".spec.ts")
            && !p.ends_with(".spec.tsx")
    });

    if !has_real_code_change && !episode.changed_files.is_empty() {
        return true;
    }

    // If user goal looks like a debugging instruction
    let goal_lower = episode.user_goal_summary.to_lowercase();
    let debug_markers = [
        "debug",
        "print log",
        "console.log",
        "trace",
        "inspect",
        "看看",
        "检查一下",
        "查一下",
        "观察",
        "验证一下",
    ];
    if debug_markers.iter().any(|m| goal_lower.contains(m))
        && episode.changed_files.len() <= 1
        && episode.tool_count <= 2
    {
        return true;
    }

    false
}

/// Check whether a shell tool trace that looks failed is actually a false
/// positive, taking the tool category into account.
pub(crate) fn shell_tool_trace_failure_is_false_positive(
    category: &AgentToolCategory,
    command: Option<&str>,
    result_summary: Option<&str>,
) -> bool {
    if !matches!(category, AgentToolCategory::Shell) {
        return false;
    }
    shell_failure_is_false_positive(command, result_summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_zero_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npm test"),
            Some("Exit code: -1 Stdout: tests passed EXIT: 0 Stderr:")
        ));
    }

    #[test]
    fn tsc_exit_zero_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npx tsc --noEmit"),
            Some("TSC_EXIT: 0")
        ));
    }

    #[test]
    fn test_exit_zero_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npm test"),
            Some("TEST_EXIT: 0")
        ));
    }

    #[test]
    fn exit_code_word_zero_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npx tsc --noEmit 2>&1"),
            Some("Exit code: -1 Stdout: TypeScript clean\nEXIT CODE: 0 Stderr:")
        ));
    }

    #[test]
    fn compact_exit_zero_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npm test"),
            Some("Exit code: -1 Stdout: tests passed\nEXIT:0 Stderr:")
        ));
    }

    #[test]
    fn equals_exit_zero_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npx tsc --noEmit 2>&1"),
            Some("Exit code: -1 Stdout: EXIT=0 Stderr:")
        ));
    }

    #[test]
    fn chinese_test_summary_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npm test"),
            Some("Exit code: -1 Stdout: 结果: 24 通过, 0 失败 Stderr:")
        ));
    }

    #[test]
    fn all_tests_passed_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npm test"),
            Some("Exit code: -1 Stdout: ALL TESTS PASSED Stderr:")
        ));
    }

    #[test]
    fn real_error_is_not_false_positive() {
        assert!(!shell_failure_is_false_positive(
            Some("cargo test"),
            Some("error: unresolved import")
        ));
    }

    #[test]
    fn sqlite_tables_inspection_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("sqlite3 .forge/continuity.db .tables"),
            Some("Exit code: -1 Stdout: continuity_events continuity_experiences Stderr:")
        ));
    }

    #[test]
    fn ls_inspection_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("ls -la"),
            Some("Exit code: -1 Stdout: total 312 drwxr-xr-x Stderr:")
        ));
    }

    #[test]
    fn git_status_inspection_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("git status"),
            Some("Exit code: -1 Stdout: On branch main Stderr:")
        ));
    }

    #[test]
    fn git_push_is_not_inspection() {
        assert!(!shell_failure_is_false_positive(
            Some("git push origin main"),
            Some("Exit code: -1 Stdout: Stderr: remote rejected")
        ));
    }

    #[test]
    fn shared_helper_behaves_same_for_episode_and_turn_paths() {
        // Both episode.rs (via collect_notable_failures) and turn_adapters.rs
        // (via continuity_tool_failure_is_actionable) now use the same shared
        // shell_failure_is_false_positive helper. This test documents the
        // equivalence for the most common false-positive patterns.
        let cases = [
            ("npm test", "EXIT: 0", true),
            ("npx tsc --noEmit", "TSC_EXIT: 0", true),
            ("npm test", "TEST_EXIT: 0", true),
            (
                "sqlite3 .forge/continuity.db .tables",
                "Stdout: continuity_events continuity_experiences",
                true,
            ),
            ("ls -la", "Stdout: total 312", true),
            ("git status", "Stdout: On branch main", true),
            ("cargo test", "error: unresolved import", false),
            ("git push", "remote rejected", false),
        ];
        for (command, summary, expected_false_positive) in cases {
            let actual = shell_failure_is_false_positive(Some(command), Some(summary));
            assert_eq!(
                actual, expected_false_positive,
                "command={command}, summary={summary}"
            );
        }
    }
}
