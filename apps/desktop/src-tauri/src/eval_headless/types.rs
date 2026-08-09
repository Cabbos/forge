use std::collections::{BTreeMap, HashMap};
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
pub const HEADLESS_DEFAULT_MAX_NO_PROGRESS_BATCHES: usize = 6;
pub const HEADLESS_SEMANTIC_MAX_NO_PROGRESS_BATCHES: usize = 2;
pub const HEADLESS_VALIDATION_TIMEOUT_SECS: u64 = 120;
pub const HEADLESS_VALIDATION_OUTPUT_LIMIT: usize = 12_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalHeadlessMode {
    #[default]
    Run,
    CredentialPreflight,
}

/// The small, closed set of transports that an eval run may attest.
///
/// This is intentionally separate from the desktop provider registry.  The
/// locked DeepSeek H1/H2 campaigns use the two official V4 Flash endpoints;
/// ordinary development chains may additionally attest an explicitly approved
/// OpenAI-compatible route such as Kimi coding `k3` without changing desktop
/// defaults or user settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalProviderTransport {
    AnthropicMessages,
    #[serde(rename = "openai_chat_completions")]
    OpenAiChatCompletions,
}

impl EvalProviderTransport {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic_messages",
            Self::OpenAiChatCompletions => "openai_chat_completions",
        }
    }

    pub(crate) const fn endpoint_family(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic",
            Self::OpenAiChatCompletions => "openai",
        }
    }

    pub(crate) const fn max_output_tokens(self) -> u32 {
        match self {
            Self::AnthropicMessages => 384_000,
            Self::OpenAiChatCompletions => 8_192,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct EvalProviderRoute {
    pub transport: EvalProviderTransport,
    pub base_url: String,
}

/// Controller-owned request dimensions that must be copied into every paid
/// eval trace.  This is additive to the route identity and intentionally does
/// not alter ordinary desktop requests.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvalExecutionIdentity {
    pub temperature_policy: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    pub reasoning_effort: String,
    pub max_output_tokens: u32,
    pub tool_catalog_format: String,
    pub tool_catalog_sha256: String,
    pub controller_version: String,
    pub runtime_versions: BTreeMap<String, String>,
}

impl EvalExecutionIdentity {
    pub(crate) fn validate_for_transport(
        &self,
        transport: EvalProviderTransport,
    ) -> Result<(), String> {
        if !matches!(
            self.temperature_policy.as_str(),
            "explicit" | "omitted" | "provider_default"
        ) {
            return Err("execution_identity_temperature_policy_invalid".to_string());
        }
        if self.temperature_policy != "omitted" {
            return Err("execution_identity_temperature_policy_invalid".to_string());
        }
        if self.temperature_policy == "explicit" && self.temperature.is_none() {
            return Err("execution_identity_temperature_missing".to_string());
        }
        if self.temperature_policy != "explicit" && self.temperature.is_some() {
            return Err("execution_identity_temperature_unexpected".to_string());
        }
        if self.reasoning_effort.trim().is_empty() {
            return Err("execution_identity_reasoning_effort_missing".to_string());
        }
        if self.reasoning_effort != "provider_default" {
            return Err("execution_identity_reasoning_effort_invalid".to_string());
        }
        if self.max_output_tokens != transport.max_output_tokens() {
            return Err("execution_identity_output_limit_invalid".to_string());
        }
        if self.tool_catalog_format != "tool_names_v1"
            || self.tool_catalog_sha256.len() != 64
            || !self
                .tool_catalog_sha256
                .chars()
                .all(|value| value.is_ascii_digit() || ('a'..='f').contains(&value))
        {
            return Err("execution_identity_tool_catalog_invalid".to_string());
        }
        if self.controller_version.trim().is_empty() || self.runtime_versions.is_empty() {
            return Err("execution_identity_runtime_version_missing".to_string());
        }
        if self
            .runtime_versions
            .iter()
            .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
        {
            return Err("execution_identity_runtime_version_invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvalHeadlessRequest {
    #[serde(default)]
    pub mode: EvalHeadlessMode,
    #[serde(default)]
    pub task: Option<EvalHeadlessTask>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Optional eval-only expected provider route.  When present, Rust
    /// validates it against the isolated provider settings before credential
    /// resolution or session construction.  When absent, desktop defaults are
    /// unchanged for backwards compatibility.
    #[serde(default)]
    pub provider_route: Option<EvalProviderRoute>,
    /// Controller-owned execution controls for reproducible eval traces.
    /// Ordinary desktop callers leave this unset.
    #[serde(default)]
    pub execution_identity: Option<EvalExecutionIdentity>,
    /// Optional profile id for future runtime profile selection.
    /// Not yet wired to agent session creation; stored for forward-compat.
    #[serde(default)]
    pub profile_id: Option<String>,
    /// Explicitly allow an isolated headless run to fall back to the canonical
    /// provider credential reference in the system credential store.
    #[serde(default)]
    pub use_provider_credential_store: bool,
    pub workspace_path: PathBuf,
    /// Optional eval-only root for internal runtime state such as registry.db.
    #[serde(default)]
    pub runtime_state_path: Option<PathBuf>,
    /// Optional eval-only Continuity database path outside the observed workspace.
    #[serde(default)]
    pub continuity_database_path: Option<PathBuf>,
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
    #[serde(default)]
    pub stagnation_policy: EvalStagnationPolicy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalStagnationPolicy {
    #[default]
    CurrentLoopGuardV1,
    SemanticProgressV1,
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
