use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::turn_state::AgentVerificationTrace;
use crate::loop_runtime::LoopTaskRecord;
use crate::protocol::events::StreamEvent;

pub type PendingConfirms =
    Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>;

pub const HEADLESS_CONFIRM_RETRY_ATTEMPTS: usize = 100;
pub const HEADLESS_CONFIRM_RETRY_DELAY_MS: u64 = 10;
pub const HEADLESS_DEFAULT_REPAIR_ATTEMPTS: usize = 1;
pub const HEADLESS_MAX_REPAIR_ATTEMPTS: usize = 3;
pub const HEADLESS_DEFAULT_TIMEOUT_SECS: u64 = 600;
pub const HEADLESS_DEFAULT_MAX_MODEL_ROUNDS: usize = 80;
pub const HEADLESS_VALIDATION_TIMEOUT_SECS: u64 = 120;
pub const HEADLESS_VALIDATION_OUTPUT_LIMIT: usize = 12_000;

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
    /// Optional profile id for future runtime profile selection.
    /// Not yet wired to agent session creation; stored for forward-compat.
    #[serde(default)]
    pub profile_id: Option<String>,
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
pub(crate) struct HeadlessValidationResult {
    pub(crate) command: String,
    pub(crate) status: crate::agent::turn_state::AgentVerificationStatus,
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) duration_ms: u64,
}

impl HeadlessValidationResult {
    pub(crate) fn passed(&self) -> bool {
        self.status == crate::agent::turn_state::AgentVerificationStatus::Passed
    }

    pub(crate) fn to_trace(&self) -> AgentVerificationTrace {
        AgentVerificationTrace {
            status: self.status.clone(),
            command: Some(self.command.clone()),
            exit_code: self.exit_code,
            stdout_preview: crate::eval_headless::validation::optional_text(&self.stdout),
            stderr_preview: crate::eval_headless::validation::optional_text(&self.stderr),
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
    pub loop_task: Option<LoopTaskRecord>,
    pub latest_turn: Option<crate::agent::turn_state::AgentTurnState>,
    pub file_diffs: Vec<HeadlessFileDiff>,
    pub changed_files: Vec<String>,
    pub final_answer: String,
    pub duration_ms: u64,
    pub continuity_formed_count: Option<usize>,
    pub continuity_error: Option<String>,
    pub repair_attempts_used: usize,
    pub validation_attempts: usize,
}

#[derive(Default)]
pub(crate) struct EventSummary {
    pub(crate) tool_calls: Vec<serde_json::Value>,
    pub(crate) shell_outputs: Vec<serde_json::Value>,
    pub(crate) model_rounds: u64,
    pub(crate) confirm_requests: u64,
    pub(crate) compact_events: Vec<serde_json::Value>,
    pub(crate) compact_count: u64,
    pub(crate) compact_estimated_tokens_saved: u64,
    pub(crate) input_tokens: Option<u32>,
    pub(crate) output_tokens: Option<u32>,
}

#[derive(Default)]
pub(crate) struct PendingTool {
    pub(crate) name: String,
    pub(crate) input: serde_json::Value,
}

#[derive(Default)]
pub(crate) struct PendingShell {
    pub(crate) command: String,
    pub(crate) stdout: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnapshotFile {
    pub(crate) contents: Vec<u8>,
}

pub(crate) type WorkspaceSnapshot = HashMap<String, SnapshotFile>;

pub(crate) struct SetupErrorPayloadInput {
    pub(crate) task_id: String,
    pub(crate) prompt: String,
    pub(crate) display_provider: String,
    pub(crate) display_model: String,
    pub(crate) agent_provider: String,
    pub(crate) agent_model: String,
    pub(crate) duration_ms: u64,
    pub(crate) error: String,
    pub(crate) failure_reason: String,
}
