use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::agent::turn_state::{
    AgentToolCategory, AgentToolStatus, AgentTurnState, AgentVerificationStatus,
    AgentVerificationTrace,
};
use crate::process_runner::{run_captured, ProcessRunOptions, ProcessSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerificationStep {
    pub display_command: String,
    pub cwd: PathBuf,
    pub program: String,
    pub args: Vec<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerificationPlan {
    pub display_command: String,
    pub cwd: PathBuf,
    pub program: String,
    pub args: Vec<String>,
    pub timeout_secs: u64,
    pub extra_steps: Vec<VerificationStep>,
}

impl VerificationPlan {
    fn from_steps(mut steps: Vec<VerificationStep>) -> Option<Self> {
        if steps.is_empty() {
            return None;
        }
        let display_command = steps
            .iter()
            .map(|step| step.display_command.as_str())
            .collect::<Vec<_>>()
            .join(" && ");
        let first = steps.remove(0);
        Some(Self {
            display_command,
            cwd: first.cwd,
            program: first.program,
            args: first.args,
            timeout_secs: first.timeout_secs,
            extra_steps: steps,
        })
    }

    fn into_steps(self) -> Vec<VerificationStep> {
        let mut steps = vec![VerificationStep {
            display_command: if self.extra_steps.is_empty() {
                self.display_command
            } else {
                self.program_and_args_display()
            },
            cwd: self.cwd,
            program: self.program,
            args: self.args,
            timeout_secs: self.timeout_secs,
        }];
        steps.extend(self.extra_steps);
        steps
    }

    fn program_and_args_display(&self) -> String {
        if self.args.is_empty() {
            return display_program(&self.program);
        }
        format!("{} {}", display_program(&self.program), self.args.join(" "))
    }
}

fn display_program(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(program)
        .to_string()
}

pub(crate) fn needs_verification(turn: &AgentTurnState) -> bool {
    turn.tools.iter().any(is_successful_mutation)
}

pub(crate) fn already_verified_after_last_mutation(
    turn: &AgentTurnState,
) -> Option<AgentVerificationTrace> {
    let last_mutation_index = turn.tools.iter().rposition(is_successful_mutation)?;
    let completed_shell_commands = turn
        .tools
        .iter()
        .skip(last_mutation_index + 1)
        .filter_map(|trace| {
            if trace.category == AgentToolCategory::Shell
                && trace.status == AgentToolStatus::Completed
                && !trace.is_error
                && trace
                    .command
                    .as_deref()
                    .is_some_and(is_verification_command)
            {
                trace.command.clone()
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let rust_required = turn_affects_rust(turn);
    let node_required = turn_affects_node(turn);
    let rust_verified = !rust_required
        || completed_shell_commands
            .iter()
            .any(|command| is_rust_verification_command(command));
    let node_verified = !node_required
        || completed_shell_commands
            .iter()
            .any(|command| is_node_verification_command(command));
    let generic_verified = rust_required
        || node_required
        || completed_shell_commands
            .iter()
            .any(|command| is_verification_command(command));

    if rust_verified && node_verified && generic_verified {
        Some(AgentVerificationTrace {
            status: AgentVerificationStatus::Skipped,
            command: Some(completed_shell_commands.join(" && ")),
            exit_code: Some(0),
            stdout_preview: Some("already verified by agent tool".to_string()),
            stderr_preview: None,
            duration_ms: Some(0),
            completed_at_ms: Some(now_ms()),
        })
    } else {
        None
    }
}

pub(crate) fn select_verification_plan(
    working_dir: &Path,
    turn: &AgentTurnState,
) -> Option<VerificationPlan> {
    let affects_rust = turn_affects_rust(turn);
    let affects_node = turn_affects_node(turn);
    match (affects_rust, affects_node) {
        (true, true) => VerificationPlan::from_steps(vec![
            select_rust_step(working_dir)?,
            select_node_step(working_dir)?,
        ]),
        (true, false) => VerificationPlan::from_steps(vec![select_rust_step(working_dir)?]),
        (false, true) => VerificationPlan::from_steps(vec![select_node_step(working_dir)?]),
        (false, false) => select_node_step(working_dir)
            .or_else(|| select_rust_step(working_dir))
            .and_then(|step| VerificationPlan::from_steps(vec![step])),
    }
}

pub(crate) async fn run_verification(plan: VerificationPlan) -> AgentVerificationTrace {
    let started_at = Instant::now();
    let display_command = plan.display_command.clone();
    let steps = plan.into_steps();
    let mut stdout_all = Vec::new();
    let mut stderr_all = Vec::new();
    let mut total_duration_ms = 0;

    for step in steps {
        let trace = run_verification_step(step).await;
        total_duration_ms += trace.duration_ms.unwrap_or(0);
        if let Some(stdout) = trace.stdout_preview.as_deref() {
            stdout_all.push(stdout.to_string());
        }
        if let Some(stderr) = trace.stderr_preview.as_deref() {
            stderr_all.push(stderr.to_string());
        }
        if matches!(
            trace.status,
            AgentVerificationStatus::Failed | AgentVerificationStatus::Error
        ) {
            return AgentVerificationTrace {
                status: trace.status,
                command: Some(display_command),
                exit_code: trace.exit_code,
                stdout_preview: optional_preview(&stdout_all.join("\n")),
                stderr_preview: optional_preview(&stderr_all.join("\n")).or(trace.stderr_preview),
                duration_ms: Some(total_duration_ms),
                completed_at_ms: Some(now_ms()),
            };
        }
    }

    AgentVerificationTrace {
        status: AgentVerificationStatus::Passed,
        command: Some(display_command),
        exit_code: Some(0),
        stdout_preview: optional_preview(&stdout_all.join("\n")),
        stderr_preview: optional_preview(&stderr_all.join("\n")),
        duration_ms: Some(elapsed_ms(started_at)),
        completed_at_ms: Some(now_ms()),
    }
}

async fn run_verification_step(step: VerificationStep) -> AgentVerificationTrace {
    let started_at = Instant::now();
    let command = step.display_command.clone();
    let output = match run_captured(
        ProcessSpec::new(step.program, step.args, step.cwd),
        ProcessRunOptions {
            timeout: Duration::from_secs(step.timeout_secs.max(1)),
            cancel: None,
            output_limit: 120_000,
        },
    )
    .await
    {
        Ok(output) => output,
        Err(error) => {
            return AgentVerificationTrace {
                status: AgentVerificationStatus::Error,
                command: Some(command),
                exit_code: None,
                stdout_preview: None,
                stderr_preview: Some(preview(&error.to_string())),
                duration_ms: Some(elapsed_ms(started_at)),
                completed_at_ms: Some(now_ms()),
            };
        }
    };

    if output.timed_out {
        return AgentVerificationTrace {
            status: AgentVerificationStatus::Error,
            command: Some(command),
            exit_code: None,
            stdout_preview: optional_preview(&output.stdout),
            stderr_preview: Some(preview(&format!(
                "verification timed out after {}s{}",
                step.timeout_secs,
                if output.stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!("\n{}", output.stderr)
                }
            ))),
            duration_ms: Some(elapsed_ms(started_at)),
            completed_at_ms: Some(now_ms()),
        };
    }

    let exit_code = output.exit_code;
    AgentVerificationTrace {
        status: if exit_code == Some(0) {
            AgentVerificationStatus::Passed
        } else {
            AgentVerificationStatus::Failed
        },
        command: Some(command),
        exit_code,
        stdout_preview: optional_preview(&output.stdout),
        stderr_preview: optional_preview(&output.stderr),
        duration_ms: Some(elapsed_ms(started_at)),
        completed_at_ms: Some(now_ms()),
    }
}

fn is_successful_write(trace: &crate::agent::turn_state::AgentToolTrace) -> bool {
    trace.category == AgentToolCategory::Write
        && trace.status == AgentToolStatus::Completed
        && !trace.is_error
}

fn is_successful_mutation(trace: &crate::agent::turn_state::AgentToolTrace) -> bool {
    is_successful_write(trace) || is_successful_mutating_shell(trace)
}

fn is_successful_mutating_shell(trace: &crate::agent::turn_state::AgentToolTrace) -> bool {
    trace.category == AgentToolCategory::Shell
        && trace.status == AgentToolStatus::Completed
        && !trace.is_error
        && trace.command.as_deref().is_some_and(|command| {
            !is_verification_command(command) && !is_read_only_shell_command(command)
        })
}

fn select_node_step(working_dir: &Path) -> Option<VerificationStep> {
    let package_json = working_dir.join("package.json");
    let package = std::fs::read_to_string(package_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&package).ok()?;
    let scripts = json.get("scripts")?.as_object()?;
    ["build", "test", "typecheck", "check"]
        .into_iter()
        .find_map(|script| {
            let value = scripts.get(script)?.as_str()?;
            if has_lifecycle_script(scripts, script) {
                return None;
            }
            let command = safe_script_segments(script, value)?.into_iter().next()?;
            let display_command = command.join(" ");
            let mut parts = command.into_iter();
            let program = parts.next()?;
            Some(VerificationStep {
                display_command,
                cwd: working_dir.to_path_buf(),
                program: resolve_node_tool(working_dir, &program),
                args: parts.collect(),
                timeout_secs: 120,
            })
        })
}

fn has_lifecycle_script(
    scripts: &serde_json::Map<String, serde_json::Value>,
    script: &str,
) -> bool {
    scripts.contains_key(&format!("pre{script}")) || scripts.contains_key(&format!("post{script}"))
}

fn resolve_node_tool(working_dir: &Path, command: &str) -> String {
    let local_bin = working_dir.join("node_modules").join(".bin").join(command);
    if local_bin.exists() {
        local_bin.to_string_lossy().to_string()
    } else {
        command.to_string()
    }
}

fn safe_script_segments(script_name: &str, script: &str) -> Option<Vec<Vec<String>>> {
    let normalized = script.to_ascii_lowercase();
    let has_forbidden = [
        "watch", "dev", "serve", "preview", "start", "publish", "install", "rm", "curl", "wget",
        "sudo", "chmod", "chown", "kill", "pkill", "scp", "ssh", "rsync", ">", "<", "|", ";", "`",
        "$(",
    ]
    .iter()
    .any(|forbidden| normalized.contains(forbidden));
    if has_forbidden {
        return None;
    }

    let segments = normalized
        .split("&&")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| allowed_script_segment_parts(script_name, part))
        .collect::<Option<Vec<_>>>()?;

    (!segments.is_empty()).then_some(segments)
}

fn allowed_script_segment_parts(script_name: &str, segment: &str) -> Option<Vec<String>> {
    let parts = segment
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let command = parts.first().map(String::as_str).unwrap_or_default();
    if allowed_script_commands(script_name).contains(&command)
        && !has_unsafe_verification_args(&parts[1..])
    {
        Some(parts)
    } else {
        None
    }
}

fn has_unsafe_verification_args(args: &[String]) -> bool {
    args.iter().any(|arg| is_unsafe_verification_arg(arg))
}

fn is_unsafe_verification_arg(arg: &str) -> bool {
    let normalized = arg.to_ascii_lowercase();
    if let Some((_, value)) = normalized.split_once('=') {
        if is_unsafe_verification_path_value(value) {
            return true;
        }
    }

    matches!(
        normalized.as_str(),
        "--fix"
            | "--write"
            | "--watch"
            | "-w"
            | "--update"
            | "-u"
            | "--cache-location"
            | "--output-file"
            | "--out-file"
            | "--out-dir"
            | "--outdir"
            | "--outfile"
            | "-o"
    ) || normalized.starts_with("--cache-location=")
        || normalized.starts_with("--output-file=")
        || normalized.starts_with("--out-file=")
        || normalized.starts_with("--out-dir=")
        || normalized.starts_with("--outdir=")
        || normalized.starts_with("--outfile=")
        || is_unsafe_verification_path_value(&normalized)
}

fn is_unsafe_verification_path_value(value: &str) -> bool {
    let value = value.trim_matches(|ch| matches!(ch, '"' | '\''));
    let normalized = value.replace('\\', "/");
    let bytes = normalized.as_bytes();
    let has_windows_drive_root =
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/';

    normalized.starts_with('/')
        || normalized.starts_with('~')
        || has_windows_drive_root
        || normalized == ".."
        || normalized.starts_with("../")
        || normalized.contains("/../")
}

fn select_rust_step(working_dir: &Path) -> Option<VerificationStep> {
    [working_dir.to_path_buf(), working_dir.join("src-tauri")]
        .into_iter()
        .find(|dir| dir.join("Cargo.toml").exists())
        .map(|dir| VerificationStep {
            display_command: "cargo check".to_string(),
            cwd: dir,
            program: "cargo".to_string(),
            args: vec!["check".to_string()],
            timeout_secs: 120,
        })
}

fn turn_affects_rust(turn: &AgentTurnState) -> bool {
    turn.tools
        .iter()
        .flat_map(|trace| trace.affected_files.iter())
        .any(|path| path.ends_with(".rs") || path.contains("src-tauri/"))
}

fn turn_affects_node(turn: &AgentTurnState) -> bool {
    turn.tools
        .iter()
        .flat_map(|trace| trace.affected_files.iter())
        .any(|path| {
            path.ends_with(".ts")
                || path.ends_with(".tsx")
                || path.ends_with(".js")
                || path.ends_with(".jsx")
                || path.ends_with(".mjs")
                || path.ends_with(".cjs")
                || path.ends_with(".css")
                || path.ends_with(".scss")
                || path.ends_with(".json")
                || path == "package.json"
                || path.starts_with("src/")
        })
}

fn allowed_script_commands(script_name: &str) -> &'static [&'static str] {
    match script_name {
        "build" => [
            "astro",
            "next",
            "ng",
            "nuxt",
            "parcel",
            "react-scripts",
            "rollup",
            "tsc",
            "tsup",
            "turbo",
            "vite",
            "vue-tsc",
            "webpack",
        ]
        .as_slice(),
        "test" => ["jest", "mocha", "vitest"].as_slice(),
        "typecheck" | "check" => ["svelte-check", "tsc", "vue-tsc"].as_slice(),
        _ => [].as_slice(),
    }
}

fn is_read_only_shell_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    if normalized.contains("&&")
        || normalized.contains(';')
        || normalized.contains('|')
        || normalized.contains('>')
        || normalized.contains('<')
        || normalized.contains("`")
        || normalized.contains("$(")
    {
        return false;
    }
    [
        "cat ",
        "find ",
        "git diff",
        "git show",
        "git status",
        "grep ",
        "ls",
        "pwd",
        "rg ",
        "sed -n",
        "tree",
    ]
    .iter()
    .any(|prefix| normalized == *prefix || normalized.starts_with(prefix))
}

fn is_rust_verification_command(command: &str) -> bool {
    if has_shell_control_operator(command) {
        return false;
    }
    matches!(
        command.split_whitespace().collect::<Vec<_>>().as_slice(),
        ["cargo", "check" | "test", ..]
    )
}

fn is_node_verification_command(command: &str) -> bool {
    if has_shell_control_operator(command) {
        return false;
    }
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let allowed_scripts = ["build", "test", "typecheck", "check"];
    if parts
        .iter()
        .skip(1)
        .any(|part| is_unsafe_verification_arg(part))
    {
        return false;
    }
    match parts.as_slice() {
        ["npm", "test", ..] => true,
        ["npm", "run", script, ..] => allowed_scripts.contains(script),
        ["pnpm", "test", ..] => true,
        ["pnpm", "run", script, ..] => allowed_scripts.contains(script),
        ["yarn", "run", script, ..] => allowed_scripts.contains(script),
        ["yarn", script, ..] => allowed_scripts.contains(script),
        ["bun", "test", ..] => true,
        ["bun", "run", script, ..] => allowed_scripts.contains(script),
        ["tsc", ..]
        | ["vite", "build", ..]
        | ["vitest", ..]
        | ["jest", ..]
        | ["svelte-check", ..]
        | ["vue-tsc", ..] => true,
        _ => false,
    }
}

fn is_verification_command(command: &str) -> bool {
    if has_shell_control_operator(command) {
        return false;
    }
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let allowed_scripts = ["build", "test", "typecheck", "check"];
    if parts
        .iter()
        .skip(1)
        .any(|part| is_unsafe_verification_arg(part))
    {
        return false;
    }
    match parts.as_slice() {
        ["cargo", "check" | "test", ..] => true,
        ["npm", "test", ..] => true,
        ["npm", "run", script, ..] => allowed_scripts.contains(script),
        ["pnpm", "test", ..] => true,
        ["pnpm", "run", script, ..] => allowed_scripts.contains(script),
        ["yarn", "run", script, ..] => allowed_scripts.contains(script),
        ["yarn", script, ..] => allowed_scripts.contains(script),
        ["bun", "test", ..] => true,
        ["bun", "run", script, ..] => allowed_scripts.contains(script),
        ["tsc", ..] | ["vite", "build", ..] | ["vitest", ..] | ["jest", ..] | ["vue-tsc", ..] => {
            true
        }
        _ => false,
    }
}

fn has_shell_control_operator(command: &str) -> bool {
    let normalized = command.to_ascii_lowercase();
    ["&&", "||", ";", "|", ">", "<", "`", "$(", "\n", "\r"]
        .iter()
        .any(|operator| normalized.contains(operator))
}

fn optional_preview(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(preview(value))
    }
}

fn preview(value: &str) -> String {
    value.chars().take(2000).collect()
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::turn_state::{
        AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentVerificationStatus,
    };
    use std::fs;
    use std::path::Path;

    fn sample_turn() -> AgentTurnState {
        AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "agent-core".to_string(),
            "verification".to_string(),
            "verify changes".to_string(),
        )
    }

    fn trace(
        category: AgentToolCategory,
        status: AgentToolStatus,
        affected_files: Vec<&str>,
        command: Option<&str>,
        is_error: bool,
    ) -> AgentToolTrace {
        AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: match category {
                AgentToolCategory::Read => "read_file",
                AgentToolCategory::Write => "write_file",
                AgentToolCategory::Shell => "bash",
                AgentToolCategory::Delegate => "delegate_task",
                AgentToolCategory::Mcp => "mcp__tool",
                AgentToolCategory::Other => "other",
            }
            .to_string(),
            category,
            status,
            started_at_ms: 1,
            ended_at_ms: Some(2),
            result_summary: None,
            is_error,
            affected_files: affected_files.into_iter().map(str::to_string).collect(),
            command: command.map(str::to_string),
        }
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-verification-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_package_json(dir: &Path, scripts: &str) {
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"scripts":{{{scripts}}}}}"#),
        )
        .expect("write package.json");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn verification_timeout_kills_process_group() {
        let dir = temp_dir("timeout-process-group");
        let marker = dir.join("marker");
        let trace = run_verification_step(VerificationStep {
            display_command: "timeout marker script".to_string(),
            cwd: dir.clone(),
            program: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "(sleep 2; echo should-not-run > marker) & wait".to_string(),
            ],
            timeout_secs: 1,
        })
        .await;

        assert_eq!(trace.status, AgentVerificationStatus::Error);
        assert!(trace
            .stderr_preview
            .as_deref()
            .unwrap_or_default()
            .contains("verification timed out"));

        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!marker.exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_trace_needs_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(needs_verification(&turn));
    }

    #[test]
    fn read_only_trace_does_not_need_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Read,
            AgentToolStatus::Completed,
            vec!["src/lib.rs"],
            None,
            false,
        ));

        assert!(!needs_verification(&turn));
    }

    #[test]
    fn mutating_shell_trace_needs_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("node scripts/generate.js"),
            false,
        ));

        assert!(needs_verification(&turn));
    }

    #[test]
    fn read_only_shell_trace_does_not_need_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("git diff -- src/App.tsx"),
            false,
        ));

        assert!(!needs_verification(&turn));
    }

    #[test]
    fn compound_shell_trace_needs_verification_even_if_first_command_is_read_only() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("git status && node scripts/generate.js"),
            false,
        ));

        assert!(needs_verification(&turn));
    }

    #[test]
    fn existing_successful_npm_build_after_write_avoids_duplicate_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("npm run build"),
            false,
        ));

        let trace = already_verified_after_last_mutation(&turn)
            .expect("successful verification command should be reused");

        assert_eq!(trace.status, AgentVerificationStatus::Skipped);
        assert_eq!(trace.command.as_deref(), Some("npm run build"));
        assert_eq!(
            trace.stdout_preview.as_deref(),
            Some("already verified by agent tool")
        );
    }

    #[test]
    fn existing_successful_filtered_cargo_test_after_write_avoids_duplicate_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/agent/verification.rs"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("cargo test agent::verification"),
            false,
        ));

        let trace = already_verified_after_last_mutation(&turn)
            .expect("filtered cargo test should be reused");

        assert_eq!(trace.status, AgentVerificationStatus::Skipped);
        assert_eq!(
            trace.command.as_deref(),
            Some("cargo test agent::verification")
        );
    }

    #[test]
    fn rust_change_is_not_satisfied_by_node_build() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/lib.rs"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("npm run build"),
            false,
        ));

        assert!(already_verified_after_last_mutation(&turn).is_none());
    }

    #[test]
    fn mixed_change_is_not_satisfied_by_cargo_only() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/protocol/events.rs", "src/lib/protocol.ts"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("cargo check"),
            false,
        ));

        assert!(already_verified_after_last_mutation(&turn).is_none());
    }

    #[test]
    fn mixed_change_reuses_cargo_and_node_verification_after_mutation() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/protocol/events.rs", "src/lib/protocol.ts"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("cargo check"),
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("npm run build"),
            false,
        ));

        let trace = already_verified_after_last_mutation(&turn)
            .expect("mixed stack should be covered by cargo and node checks");

        assert_eq!(trace.status, AgentVerificationStatus::Skipped);
        assert_eq!(
            trace.command.as_deref(),
            Some("cargo check && npm run build")
        );
    }

    #[test]
    fn compound_npm_build_is_not_reused_as_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("npm run build || true"),
            false,
        ));

        assert!(already_verified_after_last_mutation(&turn).is_none());
    }

    #[test]
    fn compound_cargo_check_is_not_reused_as_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/lib.rs"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("cargo check; echo ok"),
            false,
        ));

        assert!(already_verified_after_last_mutation(&turn).is_none());
    }

    #[test]
    fn unsafe_watch_script_is_ignored() {
        let dir = temp_dir("unsafe-watch");
        write_package_json(&dir, r#""test":"vitest --watch""#);
        fs::write(dir.join("package-lock.json"), "").expect("write lockfile");
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_arbitrary_node_script_is_ignored() {
        let dir = temp_dir("unsafe-node");
        write_package_json(&dir, r#""build":"node scripts/build.js""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_lint_fix_script_is_ignored() {
        let dir = temp_dir("unsafe-lint-fix");
        write_package_json(&dir, r#""lint":"eslint --fix .""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_prettier_write_script_is_ignored() {
        let dir = temp_dir("unsafe-prettier-write");
        write_package_json(&dir, r#""lint":"prettier --write .""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_check_write_args_are_ignored() {
        let dir = temp_dir("unsafe-check-write");
        write_package_json(&dir, r#""check":"tsc --outDir ../outside""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_check_equal_outdir_args_are_ignored() {
        let dir = temp_dir("unsafe-check-equal-outdir");
        write_package_json(&dir, r#""check":"tsc --outDir=../outside""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_equal_path_arg_is_ignored() {
        let dir = temp_dir("unsafe-equal-path");
        write_package_json(&dir, r#""build":"vite build --config=../evil.config.js""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_windows_equal_path_args_are_ignored() {
        let dir = temp_dir("unsafe-windows-equal-path");
        write_package_json(&dir, r#""build":"vite build --config=..\\evil.config.js""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_windows_absolute_path_args_are_ignored() {
        let dir = temp_dir("unsafe-windows-absolute-path");
        write_package_json(
            &dir,
            r#""build":"vite build --config=C:\\tmp\\evil.config.js""#,
        );
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_unc_path_args_are_ignored() {
        let dir = temp_dir("unsafe-unc-path");
        write_package_json(
            &dir,
            r#""build":"vite build --config=\\\\server\\share\\evil.config.js""#,
        );
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn unsafe_lint_shell_trace_still_needs_verification() {
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));
        turn.record_tool(trace(
            AgentToolCategory::Shell,
            AgentToolStatus::Completed,
            vec![],
            Some("npm run lint -- --fix"),
            false,
        ));

        assert!(needs_verification(&turn));
        assert!(already_verified_after_last_mutation(&turn).is_none());
    }

    #[test]
    fn unsafe_lifecycle_script_blocks_node_plan() {
        let dir = temp_dir("unsafe-lifecycle");
        write_package_json(&dir, r#""prebuild":"node scripts/mutate.js","build":"tsc""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }

    #[test]
    fn node_plan_runs_direct_tool_command_not_package_manager_script() {
        let dir = temp_dir("node-direct");
        write_package_json(&dir, r#""build":"tsc && vite build""#);
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        let plan = select_verification_plan(&dir, &turn).expect("node check should be selected");

        assert_eq!(plan.display_command, "tsc");
        assert_eq!(plan.cwd, dir);
        assert_eq!(plan.program, "tsc");
        assert_eq!(plan.args, Vec::<String>::new());
    }

    #[test]
    fn package_json_build_selects_direct_node_command() {
        let dir = temp_dir("node-build");
        write_package_json(&dir, r#""lint":"eslint .","build":"tsc && vite build""#);
        fs::write(dir.join("pnpm-lock.yaml"), "").expect("write lockfile");
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src/App.tsx"],
            None,
            false,
        ));

        let plan = select_verification_plan(&dir, &turn).expect("node check should be selected");

        assert_eq!(plan.display_command, "tsc");
        assert_eq!(plan.cwd, dir);
        assert_eq!(plan.program, "tsc");
        assert_eq!(plan.args, Vec::<String>::new());
    }

    #[test]
    fn src_tauri_cargo_toml_with_rs_affected_selects_cargo_check() {
        let dir = temp_dir("rust-cargo");
        fs::create_dir_all(dir.join("src-tauri")).expect("create src-tauri");
        fs::write(
            dir.join("src-tauri/Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2021'\n",
        )
        .expect("write Cargo.toml");
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/lib.rs"],
            None,
            false,
        ));

        let plan = select_verification_plan(&dir, &turn).expect("cargo check should be selected");

        assert_eq!(plan.display_command, "cargo check");
        assert_eq!(plan.cwd, dir.join("src-tauri"));
        assert_eq!(plan.program, "cargo");
        assert_eq!(plan.args, vec!["check".to_string()]);
    }

    #[test]
    fn rust_changes_prefer_cargo_check_over_root_node_build() {
        let dir = temp_dir("rust-over-node");
        write_package_json(&dir, r#""build":"tsc && vite build""#);
        fs::create_dir_all(dir.join("src-tauri")).expect("create src-tauri");
        fs::write(
            dir.join("src-tauri/Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2021'\n",
        )
        .expect("write Cargo.toml");
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/lib.rs"],
            None,
            false,
        ));

        let plan = select_verification_plan(&dir, &turn).expect("cargo check should be selected");

        assert_eq!(plan.display_command, "cargo check");
        assert_eq!(plan.cwd, dir.join("src-tauri"));
    }

    #[test]
    fn mixed_rust_and_node_changes_select_combined_verification() {
        let dir = temp_dir("mixed-stack");
        write_package_json(&dir, r#""build":"tsc && vite build""#);
        fs::create_dir_all(dir.join("src-tauri")).expect("create src-tauri");
        fs::write(
            dir.join("src-tauri/Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2021'\n",
        )
        .expect("write Cargo.toml");
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/protocol/events.rs", "src/lib/protocol.ts"],
            None,
            false,
        ));

        let plan =
            select_verification_plan(&dir, &turn).expect("combined check should be selected");

        assert_eq!(plan.display_command, "cargo check && tsc");
        assert_eq!(plan.extra_steps.len(), 1);
        assert_eq!(plan.program, "cargo");
        assert_eq!(plan.extra_steps[0].program, "tsc");
    }

    #[test]
    fn mixed_rust_and_node_changes_require_both_verification_plans() {
        let dir = temp_dir("mixed-stack-missing-node");
        fs::create_dir_all(dir.join("src-tauri")).expect("create src-tauri");
        fs::write(
            dir.join("src-tauri/Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2021'\n",
        )
        .expect("write Cargo.toml");
        let mut turn = sample_turn();
        turn.record_tool(trace(
            AgentToolCategory::Write,
            AgentToolStatus::Completed,
            vec!["src-tauri/src/protocol/events.rs", "src/lib/protocol.ts"],
            None,
            false,
        ));

        assert!(select_verification_plan(&dir, &turn).is_none());
    }
}
