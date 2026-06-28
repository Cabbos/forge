use std::path::Path;
use std::time::Duration;

use crate::agent::turn_state::AgentVerificationStatus;
use crate::process_runner::{run_captured, ProcessRunOptions, ProcessSpec};
use crate::protocol::events::StreamEvent;

use super::types::{
    EvalHeadlessTask, HeadlessValidationResult, HEADLESS_DEFAULT_MAX_MODEL_ROUNDS,
    HEADLESS_DEFAULT_REPAIR_ATTEMPTS, HEADLESS_DEFAULT_TIMEOUT_SECS, HEADLESS_MAX_REPAIR_ATTEMPTS,
    HEADLESS_VALIDATION_OUTPUT_LIMIT, HEADLESS_VALIDATION_TIMEOUT_SECS,
};

pub(crate) fn validation_commands_from_task(task: Option<&EvalHeadlessTask>) -> Vec<String> {
    let Some(task) = task else {
        return Vec::new();
    };
    let commands = task
        .validation_commands
        .iter()
        .map(|command| command.trim().to_string())
        .filter(|command| !command.is_empty())
        .collect::<Vec<_>>();
    if !commands.is_empty() {
        return commands;
    }

    task.verification_command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(|command| vec![command.to_string()])
        .unwrap_or_default()
}

pub(crate) fn max_repair_attempts_from_task(task: Option<&EvalHeadlessTask>) -> usize {
    task.and_then(|task| task.max_repair_attempts)
        .unwrap_or(HEADLESS_DEFAULT_REPAIR_ATTEMPTS)
        .min(HEADLESS_MAX_REPAIR_ATTEMPTS)
}

pub(crate) fn resolve_timeout_secs(task: Option<&EvalHeadlessTask>) -> u64 {
    task.and_then(|task| task.timeout_secs)
        .unwrap_or(HEADLESS_DEFAULT_TIMEOUT_SECS)
}

pub(crate) fn resolve_max_model_rounds(task: Option<&EvalHeadlessTask>) -> usize {
    task.and_then(|task| task.max_model_rounds)
        .unwrap_or(HEADLESS_DEFAULT_MAX_MODEL_ROUNDS)
}

pub(crate) async fn run_headless_validation_commands(
    commands: &[String],
    workspace_path: &Path,
) -> Result<HeadlessValidationResult, String> {
    let mut last_result = None;
    for command in commands {
        let result = run_headless_validation_command(command, workspace_path).await?;
        if !result.passed() {
            return Ok(result);
        }
        last_result = Some(result);
    }

    last_result.ok_or_else(|| "Forge headless validation has no commands.".to_string())
}

pub(crate) async fn run_headless_validation_command(
    command: &str,
    workspace_path: &Path,
) -> Result<HeadlessValidationResult, String> {
    let started = std::time::Instant::now();
    let output = run_captured(
        ProcessSpec::shell(command, workspace_path.to_path_buf()),
        ProcessRunOptions {
            timeout: Duration::from_secs(HEADLESS_VALIDATION_TIMEOUT_SECS),
            cancel: None,
            output_limit: HEADLESS_VALIDATION_OUTPUT_LIMIT,
        },
    )
    .await?;
    let duration_ms = started.elapsed().as_millis() as u64;
    let status = if output.timed_out || output.cancelled {
        AgentVerificationStatus::Error
    } else if output.exit_code == Some(0) {
        AgentVerificationStatus::Passed
    } else {
        AgentVerificationStatus::Failed
    };

    Ok(HeadlessValidationResult {
        command: command.to_string(),
        status,
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
        duration_ms,
    })
}

pub(crate) fn repair_prompt_from_validation_failure(
    original_prompt: &str,
    attempt: usize,
    validation: &HeadlessValidationResult,
) -> String {
    let output = combined_validation_output(validation);
    format!(
        "The eval validation failed after attempt {attempt}. Continue in the same workspace and make the smallest fix needed.\n\nOriginal task:\n{original_prompt}\n\nFailed validation command:\n{command}\n\nExit code: {exit_code}\n\nValidation output:\n{output}\n\nAfter fixing, rerun the relevant check if needed. Do not change files outside the task scope.",
        command = validation.command,
        exit_code = validation
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    )
}

pub(crate) fn validation_events(
    session_id: &str,
    block_id: &str,
    validation: &HeadlessValidationResult,
) -> Vec<StreamEvent> {
    let mut events = vec![StreamEvent::ShellStart {
        session_id: session_id.to_string(),
        block_id: block_id.to_string(),
        command: validation.command.clone(),
    }];
    let output = combined_validation_output(validation);
    if !output.is_empty() {
        events.push(StreamEvent::ShellOutput {
            session_id: session_id.to_string(),
            block_id: block_id.to_string(),
            content: output,
        });
    }
    events.push(StreamEvent::ShellEnd {
        session_id: session_id.to_string(),
        block_id: block_id.to_string(),
        exit_code: validation.exit_code.unwrap_or(1),
    });
    events
}

pub(crate) fn combined_validation_output(validation: &HeadlessValidationResult) -> String {
    [validation.stdout.trim_end(), validation.stderr.trim_end()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn optional_text(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}
