use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::agent::event_sink::{CollectingEventEmitter, EventEmitter};
use crate::agent::loop_guard::LoopGuard;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::turn_state::{AgentTurnState, AgentTurnStatus};
use crate::continuity::{
    build_episode_from_turn, build_send_input_reflection_event, continuity_events_from_turn,
    continuity_lessons_from_turn, ContinuityEvent, ContinuityService, ReflectionOutcome,
};
use crate::protocol::events::StreamEvent;

use super::types::{
    EvalWritePolicy, PendingConfirms, HEADLESS_CONFIRM_RETRY_ATTEMPTS,
    HEADLESS_CONFIRM_RETRY_DELAY_MS,
};

/// Eval-only write boundary used before a file/shell confirmation is
/// auto-approved by the headless emitter.  The controller's workspace
/// snapshot remains authoritative after the run; this boundary prevents a
/// known out-of-scope mutation from reaching the filesystem in the first
/// place.
#[derive(Debug, Clone)]
pub(crate) struct HeadlessWritePolicy {
    workspace: PathBuf,
    allowed_files: Vec<String>,
    forbidden_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct HeadlessWritePolicyBlock {
    pub(crate) reason: String,
}

impl HeadlessWritePolicy {
    pub(crate) fn from_request(workspace: &Path, policy: &EvalWritePolicy) -> Self {
        Self {
            workspace: workspace
                .canonicalize()
                .unwrap_or_else(|_| workspace.to_path_buf()),
            allowed_files: policy
                .allowed_files
                .iter()
                .map(|path| normalize_pattern(path))
                .collect(),
            forbidden_files: policy
                .forbidden_files
                .iter()
                .map(|path| normalize_pattern(path))
                .collect(),
        }
    }

    fn denial_reason(
        &self,
        kind: &str,
        boundary: Option<&crate::harness::write_boundary::WriteBoundary>,
    ) -> Option<String> {
        if matches!(kind, "shell_cmd" | "dangerous_cmd" | "file_delete") {
            return Some("eval_write_policy_blocked_mutating_confirmation".to_string());
        }

        if kind != "file_write" {
            return None;
        }

        let Some(boundary) = boundary else {
            return Some("eval_write_policy_missing_write_boundary".to_string());
        };
        let Some(path) = boundary.affected_files.first() else {
            return Some("eval_write_policy_missing_write_target".to_string());
        };
        let relative = normalize_workspace_relative(&self.workspace, path)
            .unwrap_or_else(|| normalize_pattern(path));
        if self
            .forbidden_files
            .iter()
            .any(|pattern| path_matches_pattern(&relative, pattern))
        {
            return Some(format!("eval_write_policy_forbidden_path:{relative}"));
        }
        if !self
            .allowed_files
            .iter()
            .any(|pattern| path_matches_pattern(&relative, pattern))
        {
            return Some(format!(
                "eval_write_policy_outside_allowed_paths:{relative}"
            ));
        }
        None
    }
}

fn normalize_pattern(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

fn normalize_workspace_relative(workspace: &Path, raw_path: &str) -> Option<String> {
    let requested = Path::new(raw_path);
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        workspace.join(requested)
    };
    let absolute = match absolute.canonicalize() {
        Ok(path) => path,
        Err(_) => match (
            absolute
                .parent()
                .and_then(|parent| parent.canonicalize().ok()),
            absolute.file_name(),
        ) {
            (Some(parent), Some(file_name)) => parent.join(file_name),
            _ => absolute,
        },
    };
    let relative = absolute.strip_prefix(workspace).ok()?;
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(normalized.to_string_lossy().replace('\\', "/"))
}

fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    if path == pattern {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(&format!("{prefix}/"));
    }
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return path.ends_with(&format!("/{suffix}"));
    }
    false
}

pub(crate) struct HeadlessEventEmitter {
    collector: CollectingEventEmitter,
    pending_confirms: PendingConfirms,
    write_policy: Option<HeadlessWritePolicy>,
    write_policy_blocks: Mutex<Vec<HeadlessWritePolicyBlock>>,
    model_rounds: Arc<AtomicUsize>,
    was_calling_model: AtomicBool,
}

impl HeadlessEventEmitter {
    pub(crate) fn new(pending_confirms: PendingConfirms) -> Self {
        Self::new_with_policy(pending_confirms, None)
    }

    pub(crate) fn new_with_write_policy(
        pending_confirms: PendingConfirms,
        write_policy: HeadlessWritePolicy,
    ) -> Self {
        Self::new_with_policy(pending_confirms, Some(write_policy))
    }

    fn new_with_policy(
        pending_confirms: PendingConfirms,
        write_policy: Option<HeadlessWritePolicy>,
    ) -> Self {
        Self {
            collector: CollectingEventEmitter::new(),
            pending_confirms,
            write_policy,
            write_policy_blocks: Mutex::new(Vec::new()),
            model_rounds: Arc::new(AtomicUsize::new(0)),
            was_calling_model: AtomicBool::new(false),
        }
    }

    pub(crate) fn drain(&self) -> Vec<StreamEvent> {
        self.collector.drain()
    }

    pub(crate) fn model_rounds(&self) -> usize {
        self.model_rounds.load(Ordering::SeqCst)
    }

    pub(crate) fn drain_write_policy_blocks(&self) -> Vec<HeadlessWritePolicyBlock> {
        std::mem::take(&mut *self.write_policy_blocks.lock().expect("write policy mutex"))
    }
}

impl EventEmitter for HeadlessEventEmitter {
    fn emit(&self, event: StreamEvent) {
        if let StreamEvent::ConfirmAsk {
            block_id,
            kind,
            boundary,
            ..
        } = &event
        {
            let pending_confirms = self.pending_confirms.clone();
            let block_id = block_id.clone();
            let policy_reason = self
                .write_policy
                .as_ref()
                .and_then(|policy| policy.denial_reason(kind, boundary.as_ref()));
            if let Some(reason) = policy_reason.as_ref() {
                self.write_policy_blocks
                    .lock()
                    .expect("write policy mutex")
                    .push(HeadlessWritePolicyBlock {
                        reason: reason.clone(),
                    });
            }
            let approve = policy_reason.is_none() && kind != "ask_user";
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

pub(crate) fn spawn_timeout_watchdog(
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

pub(crate) async fn send_headless_turn(
    session: &crate::agent::session::AgentSession,
    prompt: &str,
    emitter: Arc<HeadlessEventEmitter>,
    model_rounds_used: usize,
    max_model_rounds: usize,
    max_no_progress_batches: usize,
) -> Result<(), String> {
    configure_headless_model_round_budget_with_stagnation(
        session,
        model_rounds_used,
        max_model_rounds,
        max_no_progress_batches,
    );
    let turn_guard = session.reserve_turn()?;
    session
        .send_message_with_shared_emitter(prompt, emitter, Vec::new(), None, None, turn_guard)
        .await
}

pub(crate) fn configure_headless_model_round_budget(
    session: &crate::agent::session::AgentSession,
    model_rounds_used: usize,
    max_model_rounds: usize,
) {
    configure_headless_model_round_budget_with_stagnation(
        session,
        model_rounds_used,
        max_model_rounds,
        super::types::HEADLESS_DEFAULT_MAX_NO_PROGRESS_BATCHES,
    );
}

pub(crate) fn configure_headless_model_round_budget_with_stagnation(
    session: &crate::agent::session::AgentSession,
    model_rounds_used: usize,
    max_model_rounds: usize,
    max_no_progress_batches: usize,
) {
    let remaining = max_model_rounds.saturating_sub(model_rounds_used).max(1);
    *lock_unpoisoned(&session.loop_guard) = LoopGuard::default_limits()
        .with_max_model_rounds(remaining)
        .with_max_no_progress_batches(max_no_progress_batches.max(1));
}

pub(crate) fn headless_reflection_outcome(
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
            crate::agent::turn_state::AgentVerificationStatus::Failed
                | crate::agent::turn_state::AgentVerificationStatus::Error
        )
    {
        return ReflectionOutcome::Failed;
    }

    ReflectionOutcome::Completed
}

pub(crate) fn record_headless_continuity(
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
