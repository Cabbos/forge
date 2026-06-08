use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::event_sink::{CollectingEventEmitter, EventEmitter};
use crate::agent::provider_capabilities::{default_model, normalize_provider};
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::time::now_ms;
use crate::agent::turn_state::{
    AgentToolTrace, AgentTurnState, AgentTurnStatus, AgentVerificationStatus,
    AgentVerificationTrace,
};
use crate::continuity::{
    build_episode_from_turn, build_send_input_reflection_event, continuity_events_from_turn,
    continuity_lessons_from_turn, ContinuityEvent, ContinuityService, ReflectionOutcome,
};
use crate::ipc::session_builder::{build_agent_session, BuildAgentSessionRequest};
use crate::process_runner::{run_captured, ProcessRunOptions, ProcessSpec};
use crate::protocol::events::StreamEvent;
use crate::settings;

type PendingConfirms =
    Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>;
const HEADLESS_CONFIRM_RETRY_ATTEMPTS: usize = 100;
const HEADLESS_CONFIRM_RETRY_DELAY_MS: u64 = 10;
const HEADLESS_DEFAULT_REPAIR_ATTEMPTS: usize = 1;
const HEADLESS_MAX_REPAIR_ATTEMPTS: usize = 3;
const HEADLESS_DEFAULT_TIMEOUT_SECS: u64 = 600;
const HEADLESS_DEFAULT_MAX_MODEL_ROUNDS: usize = 80;
const HEADLESS_VALIDATION_TIMEOUT_SECS: u64 = 120;
const HEADLESS_VALIDATION_OUTPUT_LIMIT: usize = 12_000;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvalHeadlessRequest {
    #[serde(default)]
    pub task: Option<EvalHeadlessTask>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub workspace_path: PathBuf,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct EvalHeadlessTask {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub validation_commands: Vec<String>,
    #[serde(default)]
    pub verification_command: Option<String>,
    #[serde(default)]
    pub max_repair_attempts: Option<usize>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub max_model_rounds: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct HeadlessFileDiff {
    pub path: String,
    pub change_type: String,
    pub diff: String,
}

#[derive(Debug, Clone)]
struct HeadlessValidationResult {
    command: String,
    status: AgentVerificationStatus,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

impl HeadlessValidationResult {
    fn passed(&self) -> bool {
        self.status == AgentVerificationStatus::Passed
    }

    fn to_trace(&self) -> AgentVerificationTrace {
        AgentVerificationTrace {
            status: self.status.clone(),
            command: Some(self.command.clone()),
            exit_code: self.exit_code,
            stdout_preview: optional_text(&self.stdout),
            stderr_preview: optional_text(&self.stderr),
            duration_ms: Some(self.duration_ms),
            completed_at_ms: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TracePayloadInput {
    pub task_id: String,
    pub prompt: String,
    pub provider: String,
    pub model: String,
    pub raw_events: Vec<StreamEvent>,
    pub latest_turn: Option<AgentTurnState>,
    pub file_diffs: Vec<HeadlessFileDiff>,
    pub changed_files: Vec<String>,
    pub final_answer: String,
    pub duration_ms: u64,
    pub continuity_formed_count: Option<usize>,
    pub continuity_error: Option<String>,
    pub repair_attempts_used: usize,
    pub validation_attempts: usize,
}

pub async fn run_stdin_json(input: &str) -> Result<serde_json::Value, String> {
    let request: EvalHeadlessRequest = serde_json::from_str(input)
        .map_err(|error| format!("failed to parse Forge eval stdin JSON: {error}"))?;
    run_request(request).await
}

pub async fn run_request(request: EvalHeadlessRequest) -> Result<serde_json::Value, String> {
    let started = Instant::now();
    let task_id = request
        .task
        .as_ref()
        .and_then(|task| task.id.clone())
        .unwrap_or_else(|| "forge-headless-task".to_string());
    let prompt = resolve_prompt(&request)?;
    let display_provider = request
        .provider
        .clone()
        .unwrap_or_else(|| "forge".to_string());
    let display_model = request
        .model
        .clone()
        .unwrap_or_else(|| "local-forge".to_string());
    let workspace_path = request.workspace_path.clone();

    let before_snapshot = snapshot_workspace(&workspace_path)?;
    let agent_provider = resolve_agent_provider(request.provider.as_deref());
    let credentials = settings::detect_credentials(&agent_provider);
    let agent_model = resolve_agent_model(
        request.model.as_deref(),
        credentials.model.as_deref(),
        &agent_provider,
    );

    if credentials.api_key.trim().is_empty() {
        return Ok(build_setup_error_payload(SetupErrorPayloadInput {
            task_id,
            prompt,
            display_provider,
            display_model,
            agent_provider,
            agent_model,
            duration_ms: started.elapsed().as_millis() as u64,
            error: "missing_api_key".to_string(),
            failure_reason:
                "Forge headless eval could not find an API key for the selected provider."
                    .to_string(),
        }));
    }

    let pending_confirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
    let session_id = uuid::Uuid::now_v7().to_string();
    let (session, missing_api_key) = build_agent_session(BuildAgentSessionRequest {
        session_id: session_id.clone(),
        provider: agent_provider.clone(),
        model: agent_model.clone(),
        api_key: &credentials.api_key,
        api_base: credentials.api_base.as_deref(),
        working_dir: &workspace_path,
        pending_confirms: pending_confirms.clone(),
        existing_context_window_tokens: None,
    })
    .await?;

    if missing_api_key {
        return Ok(build_setup_error_payload(SetupErrorPayloadInput {
            task_id,
            prompt,
            display_provider,
            display_model,
            agent_provider,
            agent_model,
            duration_ms: started.elapsed().as_millis() as u64,
            error: "missing_api_key".to_string(),
            failure_reason: "Forge headless eval built a session without usable credentials."
                .to_string(),
        }));
    }

    let session = Arc::new(session);
    let emitter = Arc::new(HeadlessEventEmitter::new(pending_confirms));
    let validation_commands = validation_commands_from_task(request.task.as_ref());
    let max_repair_attempts = max_repair_attempts_from_task(request.task.as_ref());
    let timeout_secs = resolve_timeout_secs(request.task.as_ref());
    let max_model_rounds = resolve_max_model_rounds(request.task.as_ref());
    let mut raw_events = Vec::new();
    let mut agent_error: Option<String> = None;
    let mut validation_result = None;
    let mut repair_attempts_used = 0;
    let mut validation_attempts = 0;

    // Initial agent turn
    {
        let watchdog =
            spawn_timeout_watchdog(started, timeout_secs, session.clone(), emitter.clone());
        let result = send_headless_turn(&session, &prompt, emitter.clone()).await;
        watchdog.abort();
        raw_events.extend(emitter.drain());

        if started.elapsed().as_secs() >= timeout_secs {
            agent_error = Some("timeout".to_string());
        } else if emitter.model_rounds() >= max_model_rounds {
            agent_error = Some("max_model_rounds_exceeded".to_string());
        } else if let Err(error) = result {
            agent_error = Some(error);
        }
    }

    // Validation and repair loop
    if agent_error.is_none() && !validation_commands.is_empty() {
        for attempt in 0..=max_repair_attempts {
            // Budget check before validation
            if started.elapsed().as_secs() >= timeout_secs {
                agent_error = Some("timeout".to_string());
                break;
            }
            if emitter.model_rounds() >= max_model_rounds {
                agent_error = Some("max_model_rounds_exceeded".to_string());
                break;
            }

            let validation =
                run_headless_validation_commands(&validation_commands, &workspace_path).await?;
            validation_attempts += 1;
            raw_events.extend(validation_events(
                &session_id,
                &format!("headless-validation-{attempt}"),
                &validation,
            ));
            validation_result = Some(validation.clone());

            if validation.passed() || attempt == max_repair_attempts {
                break;
            }

            // Budget check before repair turn
            if started.elapsed().as_secs() >= timeout_secs {
                agent_error = Some("timeout".to_string());
                repair_attempts_used = attempt + 1;
                break;
            }
            if emitter.model_rounds() >= max_model_rounds {
                agent_error = Some("max_model_rounds_exceeded".to_string());
                repair_attempts_used = attempt + 1;
                break;
            }

            let repair_prompt =
                repair_prompt_from_validation_failure(&prompt, attempt + 1, &validation);
            let watchdog =
                spawn_timeout_watchdog(started, timeout_secs, session.clone(), emitter.clone());
            let result = send_headless_turn(&session, &repair_prompt, emitter.clone()).await;
            watchdog.abort();
            raw_events.extend(emitter.drain());
            repair_attempts_used = attempt + 1;

            if started.elapsed().as_secs() >= timeout_secs {
                agent_error = Some("timeout".to_string());
                break;
            }
            if emitter.model_rounds() >= max_model_rounds {
                agent_error = Some("max_model_rounds_exceeded".to_string());
                break;
            }
            if let Err(error) = result {
                agent_error = Some(error);
                break;
            }
        }
    }

    let mut latest_turn = lock_unpoisoned(&session.latest_turn).clone();
    if let (Some(turn), Some(validation)) = (latest_turn.as_mut(), validation_result.as_ref()) {
        turn.set_verification(validation.to_trace());
    }
    let continuity_outcome =
        headless_reflection_outcome(agent_error.as_ref(), latest_turn.as_ref());
    let mut continuity_formed_count = None;
    let mut continuity_error = None;
    match record_headless_continuity(
        &ContinuityService::new(),
        &workspace_path,
        &session_id,
        &prompt,
        latest_turn.as_ref(),
        continuity_outcome,
        now_ms(),
    ) {
        Ok(formed_count) => {
            continuity_formed_count = Some(formed_count);
        }
        Err(error) => {
            crate::app_log!(
                "WARN",
                "[continuity] headless continuity record failed: {}",
                error
            );
            continuity_error = Some(error);
        }
    }
    let after_snapshot = snapshot_workspace(&workspace_path)?;
    let (changed_files, file_diffs) = diff_workspace_snapshots(&before_snapshot, &after_snapshot);
    let final_answer = final_answer_from_events(&raw_events);

    let mut payload = build_trace_payload(TracePayloadInput {
        task_id,
        prompt,
        provider: display_provider,
        model: display_model,
        raw_events,
        latest_turn,
        file_diffs,
        changed_files,
        final_answer,
        duration_ms: started.elapsed().as_millis() as u64,
        continuity_formed_count,
        continuity_error,
        repair_attempts_used,
        validation_attempts,
    });
    insert_agent_identity(&mut payload, &agent_provider, &agent_model);

    if let Some(ref error) = agent_error {
        let (error_code, failure_category, failure_reason) = if error == "timeout" {
            (
                "timeout",
                "timeout",
                "Forge headless eval exceeded the configured timeout.".to_string(),
            )
        } else if error == "max_model_rounds_exceeded" {
            (
                "max_model_rounds_exceeded",
                "budget_exhausted",
                "Forge headless eval exceeded the configured max model rounds.".to_string(),
            )
        } else {
            (
                "agent_error",
                "agent_error",
                format!("Forge agent turn failed: {error}"),
            )
        };
        insert_failure_fields(&mut payload, error_code, failure_category, &failure_reason);
    }

    Ok(payload)
}

struct HeadlessEventEmitter {
    collector: CollectingEventEmitter,
    pending_confirms: PendingConfirms,
    model_rounds: Arc<AtomicUsize>,
    was_calling_model: AtomicBool,
}

impl HeadlessEventEmitter {
    fn new(pending_confirms: PendingConfirms) -> Self {
        Self {
            collector: CollectingEventEmitter::new(),
            pending_confirms,
            model_rounds: Arc::new(AtomicUsize::new(0)),
            was_calling_model: AtomicBool::new(false),
        }
    }

    fn drain(&self) -> Vec<StreamEvent> {
        self.collector.drain()
    }

    fn model_rounds(&self) -> usize {
        self.model_rounds.load(Ordering::SeqCst)
    }
}

impl EventEmitter for HeadlessEventEmitter {
    fn emit(&self, event: StreamEvent) {
        if let StreamEvent::ConfirmAsk { block_id, kind, .. } = &event {
            let pending_confirms = self.pending_confirms.clone();
            let block_id = block_id.clone();
            let approve = kind != "ask_user";
            tokio::spawn(async move {
                for _ in 0..HEADLESS_CONFIRM_RETRY_ATTEMPTS {
                    if let Some(sender) = pending_confirms.write().await.remove(&block_id) {
                        let _ = sender.send(approve);
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(
                        HEADLESS_CONFIRM_RETRY_DELAY_MS,
                    ))
                    .await;
                }
            });
        }

        if let StreamEvent::AgentTurnUpdated { state, .. } = &event {
            let is_calling_model = state.status == AgentTurnStatus::CallingModel;
            let was_calling = self.was_calling_model.load(Ordering::SeqCst);
            if is_calling_model && !was_calling {
                self.model_rounds.fetch_add(1, Ordering::SeqCst);
            }
            self.was_calling_model
                .store(is_calling_model, Ordering::SeqCst);
        }

        self.collector.emit(event);
    }
}

fn spawn_timeout_watchdog(
    started: Instant,
    timeout_secs: u64,
    session: Arc<crate::agent::session::AgentSession>,
    emitter: Arc<HeadlessEventEmitter>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let elapsed = started.elapsed().as_secs();
        if elapsed >= timeout_secs {
            session.kill_with_emitter(&*emitter);
            return;
        }
        let remaining = timeout_secs - elapsed;
        tokio::time::sleep(Duration::from_secs(remaining)).await;
        if started.elapsed().as_secs() >= timeout_secs {
            session.kill_with_emitter(&*emitter);
        }
    })
}

async fn send_headless_turn(
    session: &crate::agent::session::AgentSession,
    prompt: &str,
    emitter: Arc<HeadlessEventEmitter>,
) -> Result<(), String> {
    let turn_guard = session.reserve_turn()?;
    session
        .send_message_with_shared_emitter(prompt, emitter, Vec::new(), None, None, turn_guard)
        .await
}

fn headless_reflection_outcome(
    agent_error: Option<&String>,
    latest_turn: Option<&AgentTurnState>,
) -> ReflectionOutcome {
    if agent_error.is_some() {
        return ReflectionOutcome::Failed;
    }

    let Some(turn) = latest_turn else {
        return ReflectionOutcome::Failed;
    };

    if matches!(turn.status, AgentTurnStatus::Cancelled) {
        return ReflectionOutcome::Cancelled;
    }
    if matches!(turn.status, AgentTurnStatus::Failed)
        || matches!(
            turn.verification.status,
            AgentVerificationStatus::Failed | AgentVerificationStatus::Error
        )
    {
        return ReflectionOutcome::Failed;
    }

    ReflectionOutcome::Completed
}

fn record_headless_continuity(
    service: &ContinuityService,
    project_path: &Path,
    session_id: &str,
    prompt: &str,
    latest_turn: Option<&AgentTurnState>,
    outcome: ReflectionOutcome,
    timestamp_ms: u64,
) -> Result<usize, String> {
    let project_path = project_path
        .to_str()
        .ok_or_else(|| "headless workspace path is not valid UTF-8".to_string())?;
    service.record_event(
        project_path,
        &ContinuityEvent::UserMessage {
            session_id: session_id.to_string(),
            content: prompt.to_string(),
            timestamp_ms,
        },
    )?;

    let continuity_lessons = latest_turn
        .map(continuity_lessons_from_turn)
        .unwrap_or_default();
    let episode = latest_turn.map(build_episode_from_turn);
    service.record_event(
        project_path,
        &build_send_input_reflection_event(
            session_id,
            prompt,
            outcome,
            continuity_lessons,
            episode,
            timestamp_ms,
        ),
    )?;

    if let Some(turn) = latest_turn {
        for event in continuity_events_from_turn(turn) {
            service.record_event(project_path, &event)?;
        }
    }

    let formed = service.form_experiences_for_session(project_path, session_id, timestamp_ms)?;
    Ok(formed.len())
}

fn validation_commands_from_task(task: Option<&EvalHeadlessTask>) -> Vec<String> {
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

fn max_repair_attempts_from_task(task: Option<&EvalHeadlessTask>) -> usize {
    task.and_then(|task| task.max_repair_attempts)
        .unwrap_or(HEADLESS_DEFAULT_REPAIR_ATTEMPTS)
        .min(HEADLESS_MAX_REPAIR_ATTEMPTS)
}

fn resolve_timeout_secs(task: Option<&EvalHeadlessTask>) -> u64 {
    task.and_then(|task| task.timeout_secs)
        .unwrap_or(HEADLESS_DEFAULT_TIMEOUT_SECS)
}

fn resolve_max_model_rounds(task: Option<&EvalHeadlessTask>) -> usize {
    task.and_then(|task| task.max_model_rounds)
        .unwrap_or(HEADLESS_DEFAULT_MAX_MODEL_ROUNDS)
}

async fn run_headless_validation_commands(
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

async fn run_headless_validation_command(
    command: &str,
    workspace_path: &Path,
) -> Result<HeadlessValidationResult, String> {
    let started = Instant::now();
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

fn repair_prompt_from_validation_failure(
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

fn validation_events(
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

fn combined_validation_output(validation: &HeadlessValidationResult) -> String {
    [validation.stdout.trim_end(), validation.stderr.trim_end()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn optional_text(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

pub fn build_trace_payload(input: TracePayloadInput) -> serde_json::Value {
    let mut event_summary = summarize_events(&input.raw_events);
    let verification_result = input
        .latest_turn
        .as_ref()
        .and_then(verification_result_from_turn);
    let (error, failure_reason, failure_category) =
        failure_fields(input.latest_turn.as_ref(), verification_result.as_ref());
    let mut tool_calls = std::mem::take(&mut event_summary.tool_calls);
    if let Some(turn) = input.latest_turn.as_ref() {
        tool_calls = enrich_tool_calls_with_turn_tools(tool_calls, &turn.tools);
    }

    serde_json::json!({
        "task_id": input.task_id,
        "user_prompt": input.prompt,
        "provider": input.provider,
        "model": input.model,
        "raw_events": input.raw_events
            .iter()
            .filter_map(|event| serde_json::to_value(event).ok())
            .collect::<Vec<_>>(),
        "tool_calls": tool_calls,
        "shell_outputs": event_summary.shell_outputs,
        "file_diffs": input.file_diffs
            .into_iter()
            .map(|diff| {
                serde_json::json!({
                    "path": diff.path,
                    "change_type": diff.change_type,
                    "diff": diff.diff,
                })
            })
            .collect::<Vec<_>>(),
        "changed_files": input.changed_files,
        "verification_result": verification_result,
        "headless_continuity_formed_count": input.continuity_formed_count,
        "headless_continuity_error": input.continuity_error,
        "final_answer": input.final_answer,
        "model_rounds": event_summary.model_rounds,
        "confirm_requests": event_summary.confirm_requests,
        "compact_events": event_summary.compact_events,
        "compact_count": event_summary.compact_count,
        "compact_estimated_tokens_saved": event_summary.compact_estimated_tokens_saved,
        "input_tokens": event_summary.input_tokens,
        "output_tokens": event_summary.output_tokens,
        "repair_attempts_used": input.repair_attempts_used,
        "validation_attempts": input.validation_attempts,
        "error": error,
        "failure_reason": failure_reason,
        "failure_category": failure_category,
        "duration_ms": input.duration_ms,
    })
}

struct SetupErrorPayloadInput {
    task_id: String,
    prompt: String,
    display_provider: String,
    display_model: String,
    agent_provider: String,
    agent_model: String,
    duration_ms: u64,
    error: String,
    failure_reason: String,
}

fn build_setup_error_payload(input: SetupErrorPayloadInput) -> serde_json::Value {
    let mut payload = build_trace_payload(TracePayloadInput {
        task_id: input.task_id,
        prompt: input.prompt,
        provider: input.display_provider,
        model: input.display_model,
        raw_events: Vec::new(),
        latest_turn: None,
        file_diffs: Vec::new(),
        changed_files: Vec::new(),
        final_answer: String::new(),
        duration_ms: input.duration_ms,
        continuity_formed_count: None,
        continuity_error: None,
        repair_attempts_used: 0,
        validation_attempts: 0,
    });
    insert_agent_identity(&mut payload, &input.agent_provider, &input.agent_model);
    insert_failure_fields(
        &mut payload,
        &input.error,
        "runner_error",
        &input.failure_reason,
    );
    payload
}

#[derive(Default)]
struct EventSummary {
    tool_calls: Vec<serde_json::Value>,
    shell_outputs: Vec<serde_json::Value>,
    model_rounds: u64,
    confirm_requests: u64,
    compact_events: Vec<serde_json::Value>,
    compact_count: u64,
    compact_estimated_tokens_saved: u64,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[derive(Default)]
struct PendingTool {
    name: String,
    input: serde_json::Value,
}

#[derive(Default)]
struct PendingShell {
    command: String,
    stdout: String,
}

fn summarize_events(events: &[StreamEvent]) -> EventSummary {
    let mut summary = EventSummary::default();
    let mut pending_tools: HashMap<String, PendingTool> = HashMap::new();
    let mut pending_shells: HashMap<String, PendingShell> = HashMap::new();
    let mut last_turn_was_calling_model = false;
    let mut calling_model_transitions = 0;

    for event in events {
        match event {
            StreamEvent::ToolCallStart {
                block_id,
                tool_name,
                tool_input,
                ..
            } => {
                summary.model_rounds += 1;
                pending_tools.insert(
                    block_id.clone(),
                    PendingTool {
                        name: tool_name.clone(),
                        input: tool_input.clone(),
                    },
                );
            }
            StreamEvent::ToolCallResult {
                block_id,
                result,
                is_error,
                duration_ms,
                ..
            } => {
                let pending = pending_tools.remove(block_id).unwrap_or_default();
                summary.tool_calls.push(serde_json::json!({
                    "command": format_tool_command(&pending.name, &pending.input),
                    "stdout": result,
                    "stderr": if *is_error { result.as_str() } else { "" },
                    "exit_code": if *is_error { 1 } else { 0 },
                    "duration_ms": duration_ms,
                }));
            }
            StreamEvent::ShellStart {
                block_id, command, ..
            } => {
                pending_shells.insert(
                    block_id.clone(),
                    PendingShell {
                        command: command.clone(),
                        stdout: String::new(),
                    },
                );
            }
            StreamEvent::ShellOutput {
                block_id, content, ..
            } => {
                pending_shells
                    .entry(block_id.clone())
                    .or_default()
                    .stdout
                    .push_str(content);
            }
            StreamEvent::ShellEnd {
                block_id,
                exit_code,
                ..
            } => {
                let pending = pending_shells.remove(block_id).unwrap_or_default();
                summary.shell_outputs.push(serde_json::json!({
                    "command": pending.command,
                    "stdout": pending.stdout,
                    "stderr": "",
                    "exit_code": exit_code,
                    "duration_ms": 0,
                }));
            }
            StreamEvent::ConfirmAsk { .. } => {
                summary.confirm_requests += 1;
            }
            StreamEvent::ContextCompacted {
                summary: compact_summary,
                retained_messages,
                compacted_messages,
                estimated_tokens_before,
                estimated_tokens_after,
                ..
            } => {
                let saved = estimated_tokens_before.saturating_sub(*estimated_tokens_after) as u64;
                let reduction_percent = if *estimated_tokens_before > 0 {
                    ((*estimated_tokens_before - *estimated_tokens_after) as f64
                        / *estimated_tokens_before as f64
                        * 100.0)
                        .round() as u64
                } else {
                    0
                };
                summary.compact_count += 1;
                summary.compact_estimated_tokens_saved += saved;
                summary.compact_events.push(serde_json::json!({
                    "summary": compact_summary,
                    "retained_messages": retained_messages,
                    "compacted_messages": compacted_messages,
                    "estimated_tokens_before": estimated_tokens_before,
                    "estimated_tokens_after": estimated_tokens_after,
                    "estimated_tokens_saved": saved,
                    "estimated_reduction_percent": reduction_percent,
                }));
            }
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
                ..
            } => {
                summary.input_tokens = Some(*input_tokens);
                summary.output_tokens = Some(*output_tokens);
            }
            StreamEvent::AgentTurnUpdated { state, .. } => {
                let is_calling_model = state.status == AgentTurnStatus::CallingModel;
                if is_calling_model && !last_turn_was_calling_model {
                    calling_model_transitions += 1;
                }
                last_turn_was_calling_model = is_calling_model;
            }
            _ => {}
        }
    }

    if summary.model_rounds == 0 {
        summary.model_rounds = calling_model_transitions;
    }

    summary
}

fn enrich_tool_calls_with_turn_tools(
    mut tool_calls: Vec<serde_json::Value>,
    turn_tools: &[AgentToolTrace],
) -> Vec<serde_json::Value> {
    if tool_calls.is_empty() {
        return turn_tools.iter().map(tool_call_from_turn_tool).collect();
    }

    for (index, tool_call) in tool_calls.iter_mut().enumerate() {
        let command_is_empty = tool_call
            .get("command")
            .and_then(|value| value.as_str())
            .is_none_or(|command| command.trim().is_empty());
        if !command_is_empty {
            continue;
        }

        if let (Some(object), Some(turn_tool)) = (tool_call.as_object_mut(), turn_tools.get(index))
        {
            object.insert(
                "command".to_string(),
                serde_json::Value::String(format_turn_tool_command(turn_tool)),
            );
        }
    }

    tool_calls
}

fn tool_call_from_turn_tool(tool: &AgentToolTrace) -> serde_json::Value {
    let duration_ms = tool
        .ended_at_ms
        .map(|ended| ended.saturating_sub(tool.started_at_ms))
        .unwrap_or(0);
    serde_json::json!({
        "command": format_turn_tool_command(tool),
        "stdout": tool.result_summary.clone().unwrap_or_default(),
        "stderr": if tool.is_error {
            tool.result_summary.clone().unwrap_or_default()
        } else {
            String::new()
        },
        "exit_code": if tool.is_error { 1 } else { 0 },
        "duration_ms": duration_ms,
    })
}

fn format_turn_tool_command(tool: &AgentToolTrace) -> String {
    if let Some(command) = tool
        .command
        .as_ref()
        .filter(|command| !command.trim().is_empty())
    {
        return command.to_string();
    }
    if let Some(path) = tool
        .affected_files
        .first()
        .filter(|path| !path.trim().is_empty())
    {
        return format!("{} {path}", tool.name);
    }
    tool.name.clone()
}

fn format_tool_command(tool_name: &str, input: &serde_json::Value) -> String {
    let path = input
        .get("path")
        .and_then(|value| value.as_str())
        .or_else(|| input.get("command").and_then(|value| value.as_str()));
    match path {
        Some(path) if !path.trim().is_empty() => format!("{tool_name} {path}"),
        _ => tool_name.to_string(),
    }
}

fn resolve_prompt(request: &EvalHeadlessRequest) -> Result<String, String> {
    let prompt = request
        .task
        .as_ref()
        .and_then(|task| task.prompt.as_deref())
        .filter(|prompt| !prompt.trim().is_empty())
        .unwrap_or(&request.prompt)
        .trim()
        .to_string();
    if prompt.is_empty() {
        return Err("Forge eval request did not include a prompt.".to_string());
    }
    Ok(prompt)
}

fn resolve_agent_provider(display_provider: Option<&str>) -> String {
    let env_provider = std::env::var("FORGE_HEADLESS_PROVIDER")
        .or_else(|_| std::env::var("FORGE_EVAL_AI_PROVIDER"))
        .ok();
    let provider_hint = env_provider
        .as_deref()
        .or_else(|| display_provider.filter(|provider| provider != &"forge"));
    normalize_provider(provider_hint)
}

fn resolve_agent_model(
    display_model: Option<&str>,
    credential_model: Option<&str>,
    provider: &str,
) -> String {
    if let Some(model) = std::env::var("FORGE_HEADLESS_MODEL")
        .or_else(|_| std::env::var("FORGE_EVAL_AI_MODEL"))
        .ok()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
    {
        return model;
    }

    credential_model
        .filter(|model| model_matches_provider(provider, model))
        .map(str::to_string)
        .or_else(|| {
            display_model
                .filter(|model| model != &"local-forge")
                .filter(|model| model_matches_provider(provider, model))
                .map(str::to_string)
        })
        .unwrap_or_else(|| default_headless_model(provider).to_string())
}

fn model_matches_provider(provider: &str, model: &str) -> bool {
    let model = model.trim().to_lowercase();
    if model.is_empty() {
        return false;
    }

    match provider {
        "deepseek" => model.starts_with("deepseek-"),
        "anthropic" => model.starts_with("claude"),
        "openai" => model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3"),
        "openrouter" => true,
        _ => true,
    }
}

fn default_headless_model(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "deepseek-v4-flash",
        _ => default_model(provider),
    }
}

fn final_answer_from_events(events: &[StreamEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextChunk { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn insert_agent_identity(payload: &mut serde_json::Value, provider: &str, model: &str) {
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "forge_agent_provider".to_string(),
            serde_json::Value::String(provider.to_string()),
        );
        object.insert(
            "forge_agent_model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }
}

fn insert_failure_fields(
    payload: &mut serde_json::Value,
    error: &str,
    failure_category: &str,
    failure_reason: &str,
) {
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
        object.insert(
            "failure_category".to_string(),
            serde_json::Value::String(failure_category.to_string()),
        );
        object.insert(
            "failure_reason".to_string(),
            serde_json::Value::String(failure_reason.to_string()),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotFile {
    contents: Vec<u8>,
}

type WorkspaceSnapshot = HashMap<String, SnapshotFile>;

fn snapshot_workspace(root: &Path) -> Result<WorkspaceSnapshot, String> {
    if !root.is_dir() {
        return Err(format!(
            "Forge eval workspace does not exist or is not a directory: {}",
            root.display()
        ));
    }

    let mut snapshot = WorkspaceSnapshot::new();
    snapshot_dir(root, root, &mut snapshot).map_err(|error| {
        format!(
            "failed to snapshot Forge eval workspace {}: {error}",
            root.display()
        )
    })?;
    Ok(snapshot)
}

fn snapshot_dir(root: &Path, dir: &Path, snapshot: &mut WorkspaceSnapshot) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if file_type.is_dir() {
            if is_ignored_snapshot_dir(&name) {
                continue;
            }
            snapshot_dir(root, &path, snapshot)?;
            continue;
        }

        if !file_type.is_file() || is_ignored_snapshot_file(&name) {
            continue;
        }

        let relative_path = normalize_relative_path(root, &path)?;
        let contents = fs::read(&path)?;
        snapshot.insert(relative_path, SnapshotFile { contents });
    }
    Ok(())
}

fn normalize_relative_path(root: &Path, path: &Path) -> io::Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

fn is_ignored_snapshot_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".forge" | "node_modules" | "target" | ".venv" | "__pycache__" | ".pytest_cache"
    )
}

fn is_ignored_snapshot_file(name: &str) -> bool {
    matches!(name, ".DS_Store")
}

fn is_ignored_snapshot_path(path: &str) -> bool {
    path == ".forge" || path.starts_with(".forge/")
}

fn diff_workspace_snapshots(
    before: &WorkspaceSnapshot,
    after: &WorkspaceSnapshot,
) -> (Vec<String>, Vec<HeadlessFileDiff>) {
    let paths = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changed_files = Vec::new();
    let mut file_diffs = Vec::new();

    for path in paths {
        if is_ignored_snapshot_path(&path) {
            continue;
        }
        let change_type = match (before.get(&path), after.get(&path)) {
            (None, Some(_)) => Some("added"),
            (Some(_), None) => Some("deleted"),
            (Some(before), Some(after)) if before != after => Some("modified"),
            _ => None,
        };
        if let Some(change_type) = change_type {
            changed_files.push(path.clone());
            file_diffs.push(HeadlessFileDiff {
                path: path.clone(),
                change_type: change_type.to_string(),
                diff: format!("workspace snapshot detected {change_type}: {path}"),
            });
        }
    }

    (changed_files, file_diffs)
}

fn verification_result_from_turn(turn: &AgentTurnState) -> Option<serde_json::Value> {
    let command = turn.verification.command.clone()?;
    let passed = matches!(
        turn.verification.status,
        AgentVerificationStatus::Passed | AgentVerificationStatus::Skipped
    );
    Some(serde_json::json!({
        "command": command,
        "passed": passed,
        "stdout": turn.verification.stdout_preview.clone().unwrap_or_default(),
        "stderr": turn.verification.stderr_preview.clone().unwrap_or_default(),
        "exit_code": turn.verification.exit_code.unwrap_or(if passed { 0 } else { 1 }),
        "duration_ms": turn.verification.duration_ms.unwrap_or(0),
    }))
}

fn failure_fields(
    turn: Option<&AgentTurnState>,
    verification_result: Option<&serde_json::Value>,
) -> (Option<String>, Option<String>, String) {
    if let Some(failure) = turn.and_then(|turn| turn.failure.as_ref()) {
        return (
            Some(failure.kind.clone()),
            Some(failure.message.clone()),
            failure.kind.clone(),
        );
    }

    if verification_result
        .and_then(|value| value.get("passed"))
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        return (
            Some("verification_failed".to_string()),
            Some("Forge verification failed.".to_string()),
            "verification_failed".to_string(),
        );
    }

    (None, None, "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::turn_state::{
        AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnProjection, AgentTurnState,
        AgentTurnStatus, AgentVerificationStatus,
    };
    use crate::harness::write_boundary::{WriteBoundary, WriteBoundaryRisk};
    use crate::protocol::events::StreamEvent;
    use std::sync::Mutex;
    use std::time::Duration;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(key: &str, value: Option<String>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn trace_payload_maps_forge_events_turn_state_and_diffs() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/tmp/workspace".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash[1m]".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Update calculator".to_string(),
        );
        turn.tools.push(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "edit_file".to_string(),
            category: AgentToolCategory::Write,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Edited src/calculator.py".to_string()),
            is_error: false,
            affected_files: vec!["src/calculator.py".to_string()],
            command: None,
        });
        turn.verification.status = AgentVerificationStatus::Passed;
        turn.verification.command = Some("python -m pytest tests/test_calculator.py".to_string());
        turn.verification.exit_code = Some(0);
        turn.verification.stdout_preview = Some("1 passed".to_string());
        turn.verification.duration_ms = Some(120);

        let payload = build_trace_payload(TracePayloadInput {
            task_id: "small-edit-success".to_string(),
            prompt: "Update src/calculator.py so add_one returns value + 1".to_string(),
            provider: "forge".to_string(),
            model: "local-forge".to_string(),
            raw_events: vec![
                StreamEvent::ToolCallStart {
                    session_id: "session-1".to_string(),
                    block_id: "tool-1".to_string(),
                    tool_name: "edit_file".to_string(),
                    tool_input: serde_json::json!({
                        "path": "src/calculator.py",
                        "old_string": "return value",
                        "new_string": "return value + 1"
                    }),
                },
                StreamEvent::ToolCallResult {
                    session_id: "session-1".to_string(),
                    block_id: "tool-1".to_string(),
                    result: "Edited src/calculator.py".to_string(),
                    is_error: false,
                    duration_ms: 10,
                },
                StreamEvent::ShellStart {
                    session_id: "session-1".to_string(),
                    block_id: "shell-1".to_string(),
                    command: "python -m pytest tests/test_calculator.py".to_string(),
                },
                StreamEvent::ShellOutput {
                    session_id: "session-1".to_string(),
                    block_id: "shell-1".to_string(),
                    content: "1 passed\n".to_string(),
                },
                StreamEvent::ShellEnd {
                    session_id: "session-1".to_string(),
                    block_id: "shell-1".to_string(),
                    exit_code: 0,
                },
                StreamEvent::ContextCompacted {
                    session_id: "session-1".to_string(),
                    block_id: "compact-1".to_string(),
                    summary: "Kept the important setup and validation details.".to_string(),
                    retained_messages: 16,
                    compacted_messages: 48,
                    estimated_tokens_before: 120_000,
                    estimated_tokens_after: 42_000,
                },
                StreamEvent::Usage {
                    session_id: "session-1".to_string(),
                    input_tokens: 120,
                    output_tokens: 40,
                    estimated_cost_usd: 0.001,
                },
            ],
            latest_turn: Some(turn),
            file_diffs: vec![HeadlessFileDiff {
                path: "src/calculator.py".to_string(),
                change_type: "modified".to_string(),
                diff: "diff --git a/src/calculator.py b/src/calculator.py".to_string(),
            }],
            changed_files: vec!["src/calculator.py".to_string()],
            final_answer: "Completed.".to_string(),
            duration_ms: 250,
            continuity_formed_count: Some(1),
            continuity_error: None,
            repair_attempts_used: 0,
            validation_attempts: 1,
        });

        assert_eq!(payload["task_id"], "small-edit-success");
        assert_eq!(payload["provider"], "forge");
        assert_eq!(payload["model"], "local-forge");
        assert_eq!(
            payload["changed_files"],
            serde_json::json!(["src/calculator.py"])
        );
        assert_eq!(
            payload["tool_calls"][0]["command"],
            "edit_file src/calculator.py"
        );
        assert_eq!(
            payload["tool_calls"][0]["stdout"],
            "Edited src/calculator.py"
        );
        assert_eq!(
            payload["shell_outputs"][0]["command"],
            "python -m pytest tests/test_calculator.py"
        );
        assert_eq!(payload["shell_outputs"][0]["stdout"], "1 passed\n");
        assert_eq!(payload["verification_result"]["passed"], true);
        assert_eq!(payload["model_rounds"], 1);
        assert_eq!(payload["confirm_requests"], 0);
        assert_eq!(payload["input_tokens"], 120);
        assert_eq!(payload["output_tokens"], 40);
        assert_eq!(payload["compact_count"], 1);
        assert_eq!(payload["compact_estimated_tokens_saved"], 78_000);
        assert_eq!(payload["compact_events"][0]["retained_messages"], 16);
        assert_eq!(payload["compact_events"][0]["compacted_messages"], 48);
        assert_eq!(
            payload["compact_events"][0]["estimated_tokens_before"],
            120_000
        );
        assert_eq!(
            payload["compact_events"][0]["estimated_tokens_after"],
            42_000
        );
        assert_eq!(
            payload["compact_events"][0]["estimated_tokens_saved"],
            78_000
        );
        assert_eq!(
            payload["compact_events"][0]["estimated_reduction_percent"],
            65
        );
        assert_eq!(payload["failure_category"], "none");
    }

    #[test]
    fn workspace_snapshot_diff_reports_added_modified_and_deleted_files() {
        let before = HashMap::from([
            (
                "src/old.py".to_string(),
                SnapshotFile {
                    contents: b"old".to_vec(),
                },
            ),
            (
                "src/keep.py".to_string(),
                SnapshotFile {
                    contents: b"same".to_vec(),
                },
            ),
            (
                "src/delete.py".to_string(),
                SnapshotFile {
                    contents: b"delete me".to_vec(),
                },
            ),
        ]);
        let after = HashMap::from([
            (
                ".forge/registry.db".to_string(),
                SnapshotFile {
                    contents: b"internal runtime state".to_vec(),
                },
            ),
            (
                "src/old.py".to_string(),
                SnapshotFile {
                    contents: b"new".to_vec(),
                },
            ),
            (
                "src/keep.py".to_string(),
                SnapshotFile {
                    contents: b"same".to_vec(),
                },
            ),
            (
                "src/add.py".to_string(),
                SnapshotFile {
                    contents: b"add me".to_vec(),
                },
            ),
        ]);

        let (changed_files, file_diffs) = diff_workspace_snapshots(&before, &after);

        assert_eq!(
            changed_files,
            vec![
                "src/add.py".to_string(),
                "src/delete.py".to_string(),
                "src/old.py".to_string()
            ]
        );
        assert_eq!(file_diffs[0].change_type, "added");
        assert_eq!(file_diffs[1].change_type, "deleted");
        assert_eq!(file_diffs[2].change_type, "modified");
    }

    #[test]
    fn deepseek_headless_model_ignores_anthropic_credential_model() {
        let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let previous_headless_model = std::env::var("FORGE_HEADLESS_MODEL").ok();
        let previous_eval_model = std::env::var("FORGE_EVAL_AI_MODEL").ok();
        std::env::remove_var("FORGE_HEADLESS_MODEL");
        std::env::remove_var("FORGE_EVAL_AI_MODEL");

        let model = resolve_agent_model(Some("local-forge"), Some("kimi-for-coding"), "deepseek");

        restore_env("FORGE_HEADLESS_MODEL", previous_headless_model);
        restore_env("FORGE_EVAL_AI_MODEL", previous_eval_model);
        assert_eq!(model, "deepseek-v4-flash");
    }

    #[test]
    fn trace_payload_uses_turn_tools_when_events_have_results_without_starts() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/tmp/workspace".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Read calculator".to_string(),
        );
        turn.tools.push(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "read_file".to_string(),
            category: AgentToolCategory::Read,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Loaded src/calculator.py".to_string()),
            is_error: false,
            affected_files: vec!["src/calculator.py".to_string()],
            command: None,
        });

        let payload = build_trace_payload(TracePayloadInput {
            task_id: "small-edit-success".to_string(),
            prompt: "Read src/calculator.py".to_string(),
            provider: "forge".to_string(),
            model: "local-forge".to_string(),
            raw_events: vec![StreamEvent::ToolCallResult {
                session_id: "session-1".to_string(),
                block_id: "tool-1".to_string(),
                result: "def add_one(value): return value".to_string(),
                is_error: false,
                duration_ms: 10,
            }],
            latest_turn: Some(turn),
            file_diffs: Vec::new(),
            changed_files: Vec::new(),
            final_answer: String::new(),
            duration_ms: 50,
            continuity_formed_count: None,
            continuity_error: None,
            repair_attempts_used: 0,
            validation_attempts: 0,
        });

        assert_eq!(
            payload["tool_calls"][0]["command"],
            "read_file src/calculator.py"
        );
        assert_eq!(
            payload["tool_calls"][0]["stdout"],
            "def add_one(value): return value"
        );
    }

    #[test]
    fn summarize_events_counts_calling_model_transitions_without_tool_starts() {
        let events = vec![
            agent_turn_event("session-1", AgentTurnStatus::Started),
            agent_turn_event("session-1", AgentTurnStatus::CallingModel),
            agent_turn_event("session-1", AgentTurnStatus::CallingModel),
            agent_turn_event("session-1", AgentTurnStatus::RunningTools),
            agent_turn_event("session-1", AgentTurnStatus::CallingModel),
        ];

        let summary = summarize_events(&events);

        assert_eq!(summary.model_rounds, 2);
    }

    #[test]
    fn validation_commands_prefer_case_commands_and_fall_back_to_verification_command() {
        let task = EvalHeadlessTask {
            id: Some("case-1".to_string()),
            prompt: Some("Fix the code".to_string()),
            validation_commands: vec!["npm test".to_string(), "npx tsc --noEmit".to_string()],
            verification_command: Some("npm run check".to_string()),
            max_repair_attempts: None,
            timeout_secs: None,
            max_model_rounds: None,
        };

        assert_eq!(
            validation_commands_from_task(Some(&task)),
            vec!["npm test".to_string(), "npx tsc --noEmit".to_string()]
        );

        let fallback_task = EvalHeadlessTask {
            id: Some("case-2".to_string()),
            prompt: Some("Fix the code".to_string()),
            validation_commands: Vec::new(),
            verification_command: Some("npm run check".to_string()),
            max_repair_attempts: None,
            timeout_secs: None,
            max_model_rounds: None,
        };

        assert_eq!(
            validation_commands_from_task(Some(&fallback_task)),
            vec!["npm run check".to_string()]
        );
    }

    #[test]
    fn repair_prompt_includes_validation_failure_details_without_hiding_original_task() {
        let validation = HeadlessValidationResult {
            command: "npx tsc --noEmit".to_string(),
            status: AgentVerificationStatus::Failed,
            exit_code: Some(2),
            stdout: String::new(),
            stderr: "src/normalize.test.ts(3,32): error TS5097".to_string(),
            duration_ms: 120,
        };

        let prompt =
            repair_prompt_from_validation_failure("Add normalizeInput and tests.", 1, &validation);

        assert!(prompt.contains("Add normalizeInput and tests."));
        assert!(prompt.contains("npx tsc --noEmit"));
        assert!(prompt.contains("TS5097"));
        assert!(prompt.contains("attempt 1"));
    }

    #[test]
    fn validation_result_events_are_visible_as_shell_outputs() {
        let validation = HeadlessValidationResult {
            command: "npm test".to_string(),
            status: AgentVerificationStatus::Failed,
            exit_code: Some(1),
            stdout: "1 failed".to_string(),
            stderr: "Expected true".to_string(),
            duration_ms: 50,
        };

        let events = validation_events("session-1", "validation-1", &validation);
        let summary = summarize_events(&events);

        assert_eq!(summary.shell_outputs[0]["command"], "npm test");
        assert_eq!(
            summary.shell_outputs[0]["stdout"],
            "1 failed\nExpected true"
        );
        assert_eq!(summary.shell_outputs[0]["exit_code"], 1);
    }

    #[tokio::test]
    async fn headless_event_emitter_auto_resolves_confirm_requests() {
        let pending_confirms: PendingConfirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let emitter = HeadlessEventEmitter::new(pending_confirms.clone());

        let (ask_tx, ask_rx) = tokio::sync::oneshot::channel();
        pending_confirms
            .write()
            .await
            .insert("ask-user".to_string(), ask_tx);
        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "ask-user".to_string(),
            question: "Need input?".to_string(),
            kind: "ask_user".to_string(),
            boundary: None,
        });
        let ask_response = tokio::time::timeout(Duration::from_secs(1), ask_rx)
            .await
            .expect("ask_user response should not hang")
            .expect("ask_user sender should respond");
        assert!(!ask_response);

        let (permission_tx, permission_rx) = tokio::sync::oneshot::channel();
        pending_confirms
            .write()
            .await
            .insert("write-file".to_string(), permission_tx);
        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "write-file".to_string(),
            question: "Allow write?".to_string(),
            kind: "file_write".to_string(),
            boundary: Some(WriteBoundary {
                title: "准备修改项目".to_string(),
                target_label: None,
                workspace_name: "workspace".to_string(),
                workspace_path: "/tmp/workspace".to_string(),
                operation: "修改文件".to_string(),
                affected_files: vec!["src/calculator.py".to_string()],
                command: None,
                impact: "将修改 1 个文件".to_string(),
                risk: WriteBoundaryRisk::Normal,
                recovery: "disposable eval workspace".to_string(),
                checkpoint_status: None,
                warning: None,
            }),
        });
        let permission_response = tokio::time::timeout(Duration::from_secs(1), permission_rx)
            .await
            .expect("permission response should not hang")
            .expect("permission sender should respond");
        assert!(permission_response);
    }

    #[tokio::test]
    async fn headless_event_emitter_approves_permission_by_kind_without_boundary() {
        let pending_confirms: PendingConfirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let emitter = HeadlessEventEmitter::new(pending_confirms.clone());

        let (permission_tx, permission_rx) = tokio::sync::oneshot::channel();
        pending_confirms
            .write()
            .await
            .insert("write-file".to_string(), permission_tx);
        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "write-file".to_string(),
            question: "Allow write?".to_string(),
            kind: "file_write".to_string(),
            boundary: None,
        });

        let permission_response = tokio::time::timeout(Duration::from_secs(1), permission_rx)
            .await
            .expect("permission response should not hang")
            .expect("permission sender should respond");
        assert!(permission_response);
    }

    #[tokio::test]
    async fn headless_event_emitter_retries_until_confirm_sender_is_registered() {
        let pending_confirms: PendingConfirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let emitter = HeadlessEventEmitter::new(pending_confirms.clone());

        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "late-sender".to_string(),
            question: "Allow write?".to_string(),
            kind: "file_write".to_string(),
            boundary: None,
        });

        let (permission_tx, permission_rx) = tokio::sync::oneshot::channel();
        tokio::time::sleep(Duration::from_millis(25)).await;
        pending_confirms
            .write()
            .await
            .insert("late-sender".to_string(), permission_tx);

        let permission_response = tokio::time::timeout(Duration::from_secs(1), permission_rx)
            .await
            .expect("permission response should not hang")
            .expect("permission sender should respond");
        assert!(permission_response);
    }

    #[tokio::test]
    async fn headless_event_emitter_approves_harness_write_permission() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-headless-write-permission-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace should be created");
        let pending_confirms: PendingConfirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let harness =
            crate::harness::Harness::new_with_pending(workspace.clone(), pending_confirms.clone());
        let emitter = Arc::new(HeadlessEventEmitter::new(pending_confirms));

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            harness.execute_tool_with_emitter(
                "session-1",
                "write_to_file",
                &serde_json::json!({
                    "path": workspace.join("created.txt").to_string_lossy(),
                    "content": "created by headless test"
                }),
                emitter,
                Some("tool-block-1"),
                None,
            ),
        )
        .await
        .expect("write permission should be resolved without hanging");

        assert!(result.starts_with("File written:"), "{result}");
        assert_eq!(
            std::fs::read_to_string(workspace.join("created.txt")).unwrap(),
            "created by headless test"
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn headless_continuity_records_turn_and_forms_experience() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-headless-continuity-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(workspace.join("src")).expect("workspace should be created");
        let project_path = workspace.to_string_lossy().to_string();
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            project_path.clone(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Add normalizeInput and tests".to_string(),
        );
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "edit_file".to_string(),
            category: AgentToolCategory::Write,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Edited src/normalize.ts".to_string()),
            is_error: false,
            affected_files: vec!["src/normalize.ts".to_string()],
            command: None,
        });
        turn.verification.status = AgentVerificationStatus::Passed;
        turn.verification.command = Some("npm test && npx tsc --noEmit".to_string());
        turn.verification.exit_code = Some(0);
        turn.verification.stdout_preview = Some("tests passed".to_string());
        turn.mark_status(AgentTurnStatus::Completed);

        let service = crate::continuity::ContinuityService::new();
        record_headless_continuity(
            &service,
            &workspace,
            "session-1",
            "Add normalizeInput and tests",
            Some(&turn),
            crate::continuity::ReflectionOutcome::Completed,
            42,
        )
        .expect("headless continuity should record");

        let db_path = workspace.join(".forge").join("continuity.db");
        let conn = rusqlite::Connection::open(&db_path).expect("continuity db should open");
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM continuity_events", [], |row| {
                row.get(0)
            })
            .expect("event count should query");
        let formed_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM continuity_formed_reflections",
                [],
                |row| row.get(0),
            )
            .expect("formed count should query");
        let experience_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM continuity_experiences", [], |row| {
                row.get(0)
            })
            .expect("experience count should query");

        assert!(event_count >= 5, "expected turn events, got {event_count}");
        assert_eq!(formed_count, 1);
        assert!(experience_count >= 1, "expected formed experience");

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn headless_event_emitter_tracks_model_rounds_from_agent_turn_updated() {
        let pending_confirms: PendingConfirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let emitter = HeadlessEventEmitter::new(pending_confirms);

        assert_eq!(emitter.model_rounds(), 0);

        // First transition to CallingModel counts
        emitter.emit(agent_turn_event(
            "session-1",
            AgentTurnStatus::GatheringContext,
        ));
        assert_eq!(emitter.model_rounds(), 0);
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::CallingModel));
        assert_eq!(emitter.model_rounds(), 1);

        // Repeated CallingModel without transition does not count
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::CallingModel));
        assert_eq!(emitter.model_rounds(), 1);

        // Transition out and back in counts again
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::RunningTools));
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::CallingModel));
        assert_eq!(emitter.model_rounds(), 2);
    }

    #[test]
    fn resolve_timeout_and_budget_uses_defaults_and_task_values() {
        assert_eq!(resolve_timeout_secs(None), HEADLESS_DEFAULT_TIMEOUT_SECS);
        assert_eq!(
            resolve_timeout_secs(Some(&EvalHeadlessTask {
                timeout_secs: Some(120),
                ..Default::default()
            })),
            120
        );

        assert_eq!(
            resolve_max_model_rounds(None),
            HEADLESS_DEFAULT_MAX_MODEL_ROUNDS
        );
        assert_eq!(
            resolve_max_model_rounds(Some(&EvalHeadlessTask {
                max_model_rounds: Some(20),
                ..Default::default()
            })),
            20
        );
    }

    #[tokio::test]
    async fn timeout_watchdog_kills_session_after_sleep() {
        let workspace =
            std::env::temp_dir().join(format!("forge-headless-watchdog-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&workspace).expect("workspace should be created");
        let pending_confirms: PendingConfirms = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let (session, _missing_key) = crate::ipc::session_builder::build_agent_session(
            crate::ipc::session_builder::BuildAgentSessionRequest {
                session_id: "watchdog-session".to_string(),
                provider: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                api_key: "fake-key-for-test",
                api_base: None,
                working_dir: &workspace,
                pending_confirms: pending_confirms.clone(),
                existing_context_window_tokens: None,
            },
        )
        .await
        .expect("session should build");
        let session = Arc::new(session);
        let emitter = Arc::new(HeadlessEventEmitter::new(pending_confirms));
        let started = Instant::now();

        let watchdog = spawn_timeout_watchdog(started, 0, session.clone(), emitter.clone());
        tokio::time::sleep(Duration::from_millis(50)).await;
        watchdog.abort();

        // Session should be stopped because timeout was 0 (immediate kill)
        assert!(
            !session.running.load(std::sync::atomic::Ordering::SeqCst),
            "watchdog should have killed session with zero timeout"
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    fn agent_turn_event(session_id: &str, status: AgentTurnStatus) -> StreamEvent {
        StreamEvent::AgentTurnUpdated {
            session_id: session_id.to_string(),
            state: AgentTurnProjection {
                session_id: session_id.to_string(),
                status,
                step_label: String::new(),
                workspace_path: "/tmp/workspace".to_string(),
                compact_count: 0,
                verification_status: AgentVerificationStatus::NotNeeded,
                model_rounds: 0,
                tool_call_count: 0,
                failed_tool_count: 0,
                estimated_context_tokens: None,
            },
        }
    }
}
