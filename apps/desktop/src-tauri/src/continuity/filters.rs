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
        if shell_failure_looks_like_silent_successful_typecheck(cmd, summary) {
            return true;
        }
        if shell_failure_looks_like_successful_build_wrapper(cmd, summary) {
            return true;
        }
        if shell_failure_looks_like_successful_cleanup(cmd, summary) {
            return true;
        }
        if shell_failure_looks_like_true_guarded_probe(cmd, summary) {
            return true;
        }
        if shell_failure_looks_like_successful_node_probe(cmd, summary) {
            return true;
        }
        if shell_failure_looks_like_successful_version_probe(cmd, summary) {
            return true;
        }
        if shell_failure_looks_like_successful_inspection(cmd, summary) {
            return true;
        }
    }
    false
}

fn shell_failure_summary_looks_successful(summary: &str) -> bool {
    let lower = summary.to_lowercase();

    let looks_like_vite_build_success = lower.contains("vite ")
        && lower.contains("building")
        && lower.contains("rendering chunks")
        && lower.contains("computing gzip size")
        && lower.contains("✓ built in");

    let has_explicit_success_exit = shell_summary_has_success_exit_marker(&lower);
    let has_explicit_test_success = lower.contains("all tests passed")
        || summary.contains("所有测试通过")
        || (summary.contains("通过") && summary.contains("0 失败"));
    let has_tap_test_success = shell_summary_has_tap_success(&lower);
    let has_error_marker = shell_summary_has_error_marker(summary) && !has_tap_test_success;

    let looks_like_npm_test_success = lower.contains("npm test")
        || lower.contains("npx tsx")
        || lower.contains("npx jest")
        || lower.contains("npx vitest");
    let npm_output_shows_passing =
        looks_like_npm_test_success && lower.contains('✓') && lower.contains('=');

    (looks_like_vite_build_success
        || has_explicit_success_exit
        || has_explicit_test_success
        || has_tap_test_success
        || npm_output_shows_passing)
        && !has_error_marker
}

fn shell_summary_has_tap_success(lower_summary: &str) -> bool {
    let compact = lower_summary.split_whitespace().collect::<String>();
    let has_failure = lower_summary.contains("not ok")
        || (1..=9).any(|count| compact.contains(&format!("#fail{count}")));
    let has_explicit_pass = (lower_summary.contains("# fail 0") || compact.contains("#fail0"))
        && (lower_summary.contains("# pass") || compact.contains("#pass"));
    let has_ok_lines = lower_summary.contains("\nok ")
        || lower_summary.contains(" ok ")
        || compact.contains("ok1-");

    lower_summary.contains("tap version") && !has_failure && (has_explicit_pass || has_ok_lines)
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
        && shell_summary_has_output(summary)
        && !shell_summary_has_error_marker(summary)
}

fn shell_failure_looks_like_successful_version_probe(command: &str, summary: &str) -> bool {
    let lower = command.to_lowercase();
    if !(lower.contains("--version") || contains_shell_word(&lower, "-v")) {
        return false;
    }
    if shell_summary_has_error_marker(summary) {
        return false;
    }
    summary_looks_like_version_output(summary)
}

fn shell_failure_looks_like_silent_successful_typecheck(command: &str, summary: &str) -> bool {
    let lower = command.to_lowercase();
    if !(contains_shell_word(&lower, "tsc") || lower.contains(" tsc ")) {
        return false;
    }
    if shell_summary_has_error_marker(summary) {
        return false;
    }
    shell_summary_payload_is_empty(summary)
}

fn shell_failure_looks_like_successful_build_wrapper(command: &str, summary: &str) -> bool {
    let lower_command = command.to_lowercase();
    if !(lower_command.contains("npm run build")
        || lower_command.contains("npm build")
        || lower_command.contains("pnpm build")
        || lower_command.contains("yarn build"))
    {
        return false;
    }
    if shell_summary_has_error_marker(summary) {
        return false;
    }
    let lower_summary = summary.to_lowercase();
    lower_summary.contains("tsc --noemit")
        && shell_stderr_section(summary)
            .unwrap_or_default()
            .trim()
            .is_empty()
}

fn shell_failure_looks_like_successful_cleanup(command: &str, summary: &str) -> bool {
    let lower_command = command.to_lowercase();
    if !contains_shell_word(&lower_command, "rm") {
        return false;
    }
    if shell_summary_has_error_marker(summary) {
        return false;
    }
    shell_summary_payload_is_empty(summary)
}

fn shell_failure_looks_like_true_guarded_probe(command: &str, summary: &str) -> bool {
    let lower_command = command.to_lowercase();
    if !lower_command.contains("|| true") {
        return false;
    }
    shell_summary_has_output(summary) && !shell_summary_has_error_marker(summary)
}

fn shell_failure_looks_like_successful_node_probe(command: &str, summary: &str) -> bool {
    let lower_command = command.to_lowercase();
    if !(lower_command.contains("node -e") && lower_command.contains("process.argv")) {
        return false;
    }
    shell_summary_has_output(summary) && !shell_summary_has_error_marker(summary)
}

fn shell_summary_payload_is_empty(summary: &str) -> bool {
    if let Some(stdout) = shell_stdout_section(summary) {
        let stderr = shell_stderr_section(summary).unwrap_or_default();
        return stdout.trim().is_empty() && stderr.trim().is_empty();
    }
    summary.trim().is_empty()
}

fn summary_looks_like_version_output(summary: &str) -> bool {
    let output = shell_stdout_section(summary).unwrap_or(summary).trim();
    if output.is_empty() {
        return false;
    }

    output.lines().all(|line| {
        let line = line.trim().to_lowercase();
        if line.is_empty() {
            return true;
        }
        let starts_with_v_number = line
            .strip_prefix('v')
            .and_then(|tail| tail.chars().next())
            .is_some_and(|ch| ch.is_ascii_digit());
        starts_with_v_number || line.contains(" v") || line.contains("version")
    })
}

fn shell_stdout_section(summary: &str) -> Option<&str> {
    let lower = summary.to_lowercase();
    let stdout_index = lower.find("stdout:")?;
    let stdout_start = stdout_index + "stdout:".len();
    let stderr_index = lower[stdout_start..]
        .find("stderr:")
        .map(|index| stdout_start + index)
        .unwrap_or(summary.len());
    Some(&summary[stdout_start..stderr_index])
}

fn shell_stderr_section(summary: &str) -> Option<&str> {
    let lower = summary.to_lowercase();
    let stderr_index = lower.find("stderr:")?;
    let stderr_start = stderr_index + "stderr:".len();
    Some(&summary[stderr_start..])
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
    shell_stdout_section(summary).is_some_and(|stdout| stdout.trim().len() >= 3)
}

fn shell_summary_has_output(summary: &str) -> bool {
    shell_summary_has_stdout(summary) || summary.trim().len() >= 3
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
    let has_real_code_change = if episode.file_changes.is_empty() {
        episode
            .changed_files
            .iter()
            .any(|path| path_counts_as_real_code_change(path))
    } else {
        episode
            .file_changes
            .iter()
            .any(|fc| path_counts_as_real_code_change(&fc.path))
    };

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

fn path_counts_as_real_code_change(path: &str) -> bool {
    let p = path.to_lowercase();
    !p.contains("continuity.db")
        && !p.contains(".forge/")
        && !p.ends_with("_test.rs")
        && !p.ends_with(".test.ts")
        && !p.ends_with(".test.tsx")
        && !p.ends_with(".spec.ts")
        && !p.ends_with(".spec.tsx")
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
        assert!(shell_failure_is_false_positive(
            Some("npm test"),
            Some("✓ 空列表\n✓ 只有 todo\n所有测试通过 ✅")
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
    fn tap_zero_failures_are_false_positive_for_test_commands() {
        assert!(shell_failure_is_false_positive(
            Some("npm run test"),
            Some("Exit code: -1 Stdout: TAP version 13 # tests 5 # pass 5 # fail 0 Stderr:")
        ));
        assert!(shell_failure_is_false_positive(
            Some("npx tsx --test src/task-summary.test.ts"),
            Some("TAP version 13\n# tests 5\n# pass 5\n# fail 0")
        ));
        assert!(shell_failure_is_false_positive(
            Some("chmod +x run_checks.sh && ./run_checks.sh ."),
            Some(
                "Exit code: -1 Stdout: TAP version 13 # tests 5 # pass 5 # fail 0 failed=0 Stderr:"
            )
        ));
        assert!(shell_failure_is_false_positive(
            Some("node --import tsx/esm --test src/task-summary.test.ts"),
            Some("Exit code: -1 Stdout: TAP version 13 # Subtest: summarizeTasks ok 1 - returns all zeros ok 2 - counts tasks Stderr:")
        ));
    }

    #[test]
    fn silent_tsc_no_emit_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npx tsc --noEmit"),
            Some("Exit code: -1\nStdout:\n\nStderr:\n")
        ));
        assert!(!shell_failure_is_false_positive(
            Some("npx tsc --noEmit"),
            Some("Exit code: -1\nStdout:\nsrc/app.ts(1,1): error TS2322\nStderr:\n")
        ));
    }

    #[test]
    fn npm_build_wrapping_tsc_success_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("npm run build"),
            Some("Exit code: -1 Stdout: > fixture@0.1.0 build > tsc --noEmit Stderr:")
        ));
    }

    #[test]
    fn silent_rm_cleanup_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("rm run_checks.sh"),
            Some("Exit code: -1 Stdout: Stderr:")
        ));
        assert!(!shell_failure_is_false_positive(
            Some("rm run_checks.sh"),
            Some("Exit code: -1 Stdout: Stderr: No such file or directory")
        ));
    }

    #[test]
    fn true_guarded_probe_with_output_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("node --help | grep -- --test || true"),
            Some("Exit code: -1 Stdout: --test launch test runner on startup Stderr:")
        ));
    }

    #[test]
    fn node_process_argv_probe_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("node -e \"console.log(process.argv)\" tsx src/storage.test.ts 2>&1"),
            Some("Exit code: -1 Stdout: [ '/node', 'tsx', 'src/storage.test.ts' ] Stderr:")
        ));
    }

    #[test]
    fn version_probe_with_stdout_is_false_positive() {
        assert!(shell_failure_is_false_positive(
            Some("node --version"),
            Some("v20.20.2")
        ));
        assert!(shell_failure_is_false_positive(
            Some("npx tsx --version"),
            Some("tsx v4.22.4\nnode v20.20.2")
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
        assert!(shell_failure_is_false_positive(
            Some("pwd"),
            Some("/private/var/folders/project/workspace")
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
