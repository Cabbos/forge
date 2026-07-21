use crate::agent::a2a::projection::AgentA2AProjection;
use crate::agent::prepared_turn::PreparedTurn;
use crate::agent::turn_state::AgentTurnProjection;
use crate::forge_wiki::model::{ForgeWikiUpdateProposal, SelectedForgeWikiPage};
use crate::harness::permission_ledger::PermissionLedgerEvent;
use crate::harness::write_boundary::WriteBoundary;
use crate::loop_runtime::LoopTaskRecord;
use crate::memory::{SelectedContextMemory, WikiMemory};
use crate::workflow::WorkflowState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliverySummary {
    pub project_path: Option<String>,
    pub preview_label: String,
    pub checkpoint_label: String,
    pub next_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub record_target_pages: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderUsageReason {
    #[default]
    ProviderReported,
    ProviderOmitted,
    PricingUnknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubagentRuntimePayload {
    Started {
        role: String,
    },
    Status {
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    FileIo {
        path: String,
        operation: String,
    },
    UsageRecorded {
        #[serde(default)]
        provider_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        #[serde(default)]
        reason: ProviderUsageReason,
        #[serde(default)]
        input_tokens: Option<u64>,
        #[serde(default)]
        output_tokens: Option<u64>,
        #[serde(default)]
        cache_read_tokens: Option<u64>,
        #[serde(default)]
        cache_creation_tokens: Option<u64>,
        #[serde(default)]
        reasoning_tokens: Option<u64>,
        #[serde(default)]
        estimated_cost_micros: Option<u64>,
        #[serde(default)]
        pricing_source: Option<String>,
    },
    Ended {
        status: String,
    },
    Failed {
        reason: String,
    },
    Interrupted {
        reason: String,
    },
}

/// Streaming events emitted from Rust backend to frontend.
/// Mirrors the TypeScript protocol in src/lib/protocol.ts
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum StreamEvent {
    // ── Transcript ──
    #[serde(rename = "user_message")]
    UserMessage {
        session_id: String,
        block_id: String,
        content: String,
    },

    // ── AI Thinking ──
    #[serde(rename = "thinking_start")]
    ThinkingStart {
        session_id: String,
        block_id: String,
    },
    #[serde(rename = "thinking_chunk")]
    ThinkingChunk {
        session_id: String,
        block_id: String,
        content: String,
    },
    #[serde(rename = "thinking_end")]
    ThinkingEnd {
        session_id: String,
        block_id: String,
    },

    // ── AI Text Response ──
    #[serde(rename = "text_start")]
    TextStart {
        session_id: String,
        block_id: String,
    },
    #[serde(rename = "text_chunk")]
    TextChunk {
        session_id: String,
        block_id: String,
        content: String,
    },
    #[serde(rename = "text_end")]
    TextEnd {
        session_id: String,
        block_id: String,
    },

    // ── Tool Calls ──
    #[serde(rename = "tool_call_start")]
    ToolCallStart {
        session_id: String,
        block_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    },
    #[serde(rename = "tool_call_result")]
    ToolCallResult {
        session_id: String,
        block_id: String,
        result: String,
        is_error: bool,
        duration_ms: u64,
    },
    #[serde(rename = "tool_call_end")]
    ToolCallEnd {
        session_id: String,
        block_id: String,
    },

    // ── File Diff ──
    #[serde(rename = "diff_view")]
    DiffView {
        session_id: String,
        block_id: String,
        file_path: String,
        old_content: String,
        new_content: String,
    },
    #[serde(rename = "file_io")]
    FileIo {
        session_id: String,
        block_id: String,
        path: String,
        operation: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },

    // ── Shell Commands ──
    #[serde(rename = "shell_start")]
    ShellStart {
        session_id: String,
        block_id: String,
        command: String,
    },
    #[serde(rename = "shell_output")]
    ShellOutput {
        session_id: String,
        block_id: String,
        content: String,
    },
    #[serde(rename = "shell_end")]
    ShellEnd {
        session_id: String,
        block_id: String,
        exit_code: i32,
    },

    // ── Permission Confirmations ──
    #[serde(rename = "confirm_ask")]
    ConfirmAsk {
        session_id: String,
        block_id: String,
        question: String,
        kind: String, // "dangerous_cmd" | "file_delete" | "api_call"
        #[serde(skip_serializing_if = "Option::is_none")]
        boundary: Option<WriteBoundary>,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_evidence: Option<PermissionLedgerEvent>,
        /// When true, this confirm is a replayed/interrupted descriptor from a
        /// restored session. The frontend should render it as non-interactive
        /// (same visual path as `closeInterruptedConfirmBlocks` with reason
        /// "session_restored").
        #[serde(default, skip_serializing_if = "is_false")]
        replayed_interrupted: bool,
    },
    #[serde(rename = "confirm_response")]
    ConfirmResponse {
        session_id: String,
        block_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        question: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        boundary: Option<WriteBoundary>,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_evidence: Option<PermissionLedgerEvent>,
        approved: Option<bool>,
        responded_at_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        replayed: bool,
    },
    #[serde(rename = "permission_decision")]
    PermissionDecision {
        session_id: String,
        block_id: String,
        evidence: PermissionLedgerEvent,
    },

    // ── Context Management ──
    #[serde(rename = "context_compact_start")]
    ContextCompactStart {
        session_id: String,
        block_id: String,
    },
    #[serde(rename = "context_compacted")]
    ContextCompacted {
        session_id: String,
        block_id: String,
        summary: String,
        retained_messages: usize,
        compacted_messages: usize,
        estimated_tokens_before: u32,
        estimated_tokens_after: u32,
    },
    #[serde(rename = "context_compact_skipped")]
    ContextCompactSkipped {
        session_id: String,
        block_id: String,
        reason: String,
        retained_messages: usize,
    },

    // ── Project Records Memory ──
    #[serde(rename = "memory_selection")]
    MemorySelection {
        session_id: String,
        selected: Vec<SelectedContextMemory>,
    },
    #[serde(rename = "memory_candidate")]
    MemoryCandidate {
        session_id: String,
        memory: WikiMemory,
    },
    #[serde(rename = "memory_updated")]
    MemoryUpdated {
        session_id: String,
        memory: WikiMemory,
    },

    // ── Project Records ──
    #[serde(rename = "forge_wiki_context_selected")]
    ForgeWikiContextSelected {
        session_id: String,
        selected: Vec<SelectedForgeWikiPage>,
    },
    #[serde(rename = "forge_wiki_update_proposed")]
    ForgeWikiUpdateProposed {
        session_id: String,
        proposal: ForgeWikiUpdateProposal,
    },
    #[serde(rename = "forge_wiki_updated")]
    ForgeWikiUpdated {
        session_id: String,
        proposal: ForgeWikiUpdateProposal,
    },

    // ── Connector Context ──
    #[serde(rename = "mcp_context_status")]
    McpContextStatus {
        session_id: String,
        source_id: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    // ── Workflow Routing ──
    #[serde(rename = "workflow_updated")]
    WorkflowUpdated {
        session_id: String,
        state: WorkflowState,
    },
    #[serde(rename = "turn_prepared")]
    TurnPrepared {
        session_id: String,
        prepared: PreparedTurn,
    },

    // ── Agent Turn Projection ──
    #[serde(rename = "agent_turn_updated")]
    AgentTurnUpdated {
        session_id: String,
        state: AgentTurnProjection,
    },

    // ── Agent A2A Projection ──
    #[serde(rename = "agent_a2a_updated")]
    AgentA2AUpdated {
        session_id: String,
        state: AgentA2AProjection,
    },

    // ── Subagent / Loop Runtime Projection ──
    #[serde(rename = "subagent_runtime_event")]
    SubagentRuntimeEvent {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        loop_task_id: Option<String>,
        task_id: String,
        event: SubagentRuntimePayload,
    },
    #[serde(rename = "loop_runtime_updated")]
    LoopRuntimeUpdated {
        session_id: String,
        loop_task_id: String,
        task: LoopTaskRecord,
    },

    // ── Ecosystem / Tooling Projection ──
    #[serde(rename = "ecosystem_changed")]
    EcosystemChanged {
        session_id: String,
        item_id: String,
        action: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled: Option<bool>,
    },

    // ── Delivery Summary ──
    #[serde(rename = "delivery_summary")]
    DeliverySummary {
        session_id: String,
        block_id: String,
        summary: DeliverySummary,
    },

    // ── Session Status ──
    #[serde(rename = "session_started")]
    SessionStarted {
        session_id: String,
        agent_type: String,
        model: String,
        context_window_tokens: Option<u32>,
    },
    #[serde(rename = "session_status")]
    SessionStatus {
        session_id: String,
        status: String, // "thinking" | "working" | "idle" | "error"
    },
    #[serde(rename = "session_stopped")]
    SessionStopped { session_id: String, reason: String },
    #[serde(rename = "error")]
    Error {
        session_id: String,
        block_id: String,
        message: String,
        code: String,
    },
    /// Legacy compatibility usage event.
    ///
    /// New UI and state projections should prefer `ProviderUsage`, because it
    /// preserves provider, model, unknown/cache/reasoning token fields, pricing
    /// source, and the reason usage or pricing was omitted.
    #[serde(rename = "usage")]
    Usage {
        session_id: String,
        input_tokens: u32,
        output_tokens: u32,
        estimated_cost_usd: f64,
    },
    /// Canonical provider usage fact for one model call.
    #[serde(rename = "provider_usage")]
    ProviderUsage {
        session_id: String,
        #[serde(default = "new_provider_usage_block_id")]
        block_id: String,
        #[serde(default)]
        provider_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default)]
        input_tokens: Option<u64>,
        #[serde(default)]
        output_tokens: Option<u64>,
        #[serde(default)]
        cache_read_tokens: Option<u64>,
        #[serde(default)]
        cache_creation_tokens: Option<u64>,
        #[serde(default)]
        reasoning_tokens: Option<u64>,
        #[serde(default)]
        estimated_cost_micros: Option<u64>,
        #[serde(default)]
        pricing_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        #[serde(default)]
        reason: ProviderUsageReason,
    },

    // ── Recovery Notice ──
    #[serde(rename = "recovery_notice")]
    RecoveryNotice {
        session_id: String,
        notice_id: String,
        title: String,
        message: String,
        reason: String,
        recoverable: bool,
    },

    // ── Diagnostics / Health ──
    /// Emitted when a diagnostics report is refreshed (e.g. after the check
    /// runner completes). Carries a summary suitable for a status bar badge and
    /// an optional serialized report for a diagnostics panel.
    #[serde(rename = "diagnostics_update")]
    DiagnosticsUpdate {
        session_id: String,
        ok: bool,
        pass_count: u32,
        warn_count: u32,
        fail_count: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        report_json: Option<String>,
    },
    /// Emitted when a health threshold is crossed (e.g. session hung, gateway
    /// down, disk space low). Carries an alert level, message, and optional
    /// remediation hint.
    #[serde(rename = "health_alert")]
    HealthAlert {
        session_id: String,
        alert_id: String,
        level: String, // "info" | "warn" | "critical"
        title: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        remediation: Option<String>,
    },
}

fn new_provider_usage_block_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

impl StreamEvent {
    /// Returns the session_id for this event.
    pub fn session_id(&self) -> &str {
        use StreamEvent::*;
        match self {
            UserMessage { session_id, .. }
            | ThinkingStart { session_id, .. }
            | ThinkingChunk { session_id, .. }
            | ThinkingEnd { session_id, .. }
            | TextStart { session_id, .. }
            | TextChunk { session_id, .. }
            | TextEnd { session_id, .. }
            | ToolCallStart { session_id, .. }
            | ToolCallResult { session_id, .. }
            | ToolCallEnd { session_id, .. }
            | DiffView { session_id, .. }
            | FileIo { session_id, .. }
            | ShellStart { session_id, .. }
            | ShellOutput { session_id, .. }
            | ShellEnd { session_id, .. }
            | ConfirmAsk { session_id, .. }
            | ConfirmResponse { session_id, .. }
            | PermissionDecision { session_id, .. }
            | ContextCompactStart { session_id, .. }
            | ContextCompacted { session_id, .. }
            | ContextCompactSkipped { session_id, .. }
            | MemorySelection { session_id, .. }
            | MemoryCandidate { session_id, .. }
            | MemoryUpdated { session_id, .. }
            | ForgeWikiContextSelected { session_id, .. }
            | ForgeWikiUpdateProposed { session_id, .. }
            | ForgeWikiUpdated { session_id, .. }
            | McpContextStatus { session_id, .. }
            | WorkflowUpdated { session_id, .. }
            | TurnPrepared { session_id, .. }
            | AgentTurnUpdated { session_id, .. }
            | AgentA2AUpdated { session_id, .. }
            | SubagentRuntimeEvent { session_id, .. }
            | LoopRuntimeUpdated { session_id, .. }
            | EcosystemChanged { session_id, .. }
            | DeliverySummary { session_id, .. }
            | SessionStarted { session_id, .. }
            | SessionStatus { session_id, .. }
            | SessionStopped { session_id, .. }
            | Error { session_id, .. }
            | Usage { session_id, .. }
            | ProviderUsage { session_id, .. }
            | RecoveryNotice { session_id, .. }
            | DiagnosticsUpdate { session_id, .. }
            | HealthAlert { session_id, .. } => session_id,
        }
    }

    /// Returns the serde `event_type` tag for this variant.
    /// Kept in sync with src/lib/protocol.ts.
    pub fn event_type(&self) -> &'static str {
        use StreamEvent::*;
        match self {
            UserMessage { .. } => "user_message",
            ThinkingStart { .. } => "thinking_start",
            ThinkingChunk { .. } => "thinking_chunk",
            ThinkingEnd { .. } => "thinking_end",
            TextStart { .. } => "text_start",
            TextChunk { .. } => "text_chunk",
            TextEnd { .. } => "text_end",
            ToolCallStart { .. } => "tool_call_start",
            ToolCallResult { .. } => "tool_call_result",
            ToolCallEnd { .. } => "tool_call_end",
            DiffView { .. } => "diff_view",
            FileIo { .. } => "file_io",
            ShellStart { .. } => "shell_start",
            ShellOutput { .. } => "shell_output",
            ShellEnd { .. } => "shell_end",
            ConfirmAsk { .. } => "confirm_ask",
            ConfirmResponse { .. } => "confirm_response",
            PermissionDecision { .. } => "permission_decision",
            ContextCompactStart { .. } => "context_compact_start",
            ContextCompacted { .. } => "context_compacted",
            ContextCompactSkipped { .. } => "context_compact_skipped",
            MemorySelection { .. } => "memory_selection",
            MemoryCandidate { .. } => "memory_candidate",
            MemoryUpdated { .. } => "memory_updated",
            ForgeWikiContextSelected { .. } => "forge_wiki_context_selected",
            ForgeWikiUpdateProposed { .. } => "forge_wiki_update_proposed",
            ForgeWikiUpdated { .. } => "forge_wiki_updated",
            McpContextStatus { .. } => "mcp_context_status",
            WorkflowUpdated { .. } => "workflow_updated",
            TurnPrepared { .. } => "turn_prepared",
            AgentTurnUpdated { .. } => "agent_turn_updated",
            AgentA2AUpdated { .. } => "agent_a2a_updated",
            SubagentRuntimeEvent { .. } => "subagent_runtime_event",
            LoopRuntimeUpdated { .. } => "loop_runtime_updated",
            EcosystemChanged { .. } => "ecosystem_changed",
            DeliverySummary { .. } => "delivery_summary",
            SessionStarted { .. } => "session_started",
            SessionStatus { .. } => "session_status",
            SessionStopped { .. } => "session_stopped",
            Error { .. } => "error",
            Usage { .. } => "usage",
            ProviderUsage { .. } => "provider_usage",
            RecoveryNotice { .. } => "recovery_notice",
            DiagnosticsUpdate { .. } => "diagnostics_update",
            HealthAlert { .. } => "health_alert",
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_runtime_event_serializes_snake_case() {
        let event = StreamEvent::SubagentRuntimeEvent {
            session_id: "s1".to_string(),
            loop_task_id: Some("loop-1".to_string()),
            task_id: "t1".to_string(),
            event: SubagentRuntimePayload::Started {
                role: "implementer".to_string(),
            },
        };

        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["event_type"], "subagent_runtime_event");
        assert_eq!(json["event"]["type"], "started");
    }

    #[test]
    fn file_io_event_serializes_executor_source() {
        let event = StreamEvent::FileIo {
            session_id: "s1".to_string(),
            block_id: "b1".to_string(),
            path: "src/main.rs".to_string(),
            operation: "read".to_string(),
            source: Some("executor".to_string()),
        };

        assert_eq!(event.event_type(), "file_io");
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["event_type"], "file_io");
        assert_eq!(json["session_id"], "s1");
        assert_eq!(json["block_id"], "b1");
        assert_eq!(json["path"], "src/main.rs");
        assert_eq!(json["operation"], "read");
        assert_eq!(json["source"], "executor");
    }

    #[test]
    fn confirm_response_event_serializes_replayable_decision() {
        let event = StreamEvent::ConfirmResponse {
            session_id: "s1".to_string(),
            block_id: "confirm-1".to_string(),
            question: Some("Allow write?".to_string()),
            kind: Some("file_write".to_string()),
            boundary: None,
            permission_evidence: None,
            approved: Some(false),
            responded_at_ms: 1234,
            reason: Some("user_response".to_string()),
            replayed: false,
        };

        assert_eq!(event.event_type(), "confirm_response");
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["event_type"], "confirm_response");
        assert_eq!(json["session_id"], "s1");
        assert_eq!(json["block_id"], "confirm-1");
        assert_eq!(json["question"], "Allow write?");
        assert_eq!(json["kind"], "file_write");
        assert_eq!(json["approved"], false);
        assert_eq!(json["responded_at_ms"], 1234);
        assert_eq!(json["reason"], "user_response");
        assert!(json.get("replayed").is_none());
    }

    #[test]
    fn provider_usage_event_serializes_unknown_cost_as_null_with_reason() {
        let event = StreamEvent::ProviderUsage {
            session_id: "s1".to_string(),
            block_id: "usage-1".to_string(),
            provider_id: Some("anthropic".to_string()),
            model: Some("mystery-model".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(50),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: None,
            pricing_source: None,
            source: Some("anthropic".to_string()),
            reason: ProviderUsageReason::PricingUnknown,
        };

        assert_eq!(event.event_type(), "provider_usage");
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["event_type"], "provider_usage");
        assert_eq!(json["block_id"], "usage-1");
        assert_eq!(json["model"], "mystery-model");
        assert_eq!(json["provider_id"], "anthropic");
        assert_eq!(json["input_tokens"], 100);
        assert_eq!(json["output_tokens"], 50);
        assert_eq!(json["cache_read_tokens"], serde_json::Value::Null);
        assert_eq!(json["cache_creation_tokens"], serde_json::Value::Null);
        assert_eq!(json["reasoning_tokens"], serde_json::Value::Null);
        assert_eq!(json["estimated_cost_micros"], serde_json::Value::Null);
        assert_eq!(json["pricing_source"], serde_json::Value::Null);
        assert_eq!(json["source"], "anthropic");
        assert_eq!(json["reason"], "pricing_unknown");
    }

    #[test]
    fn provider_usage_event_deserializes_legacy_payload_without_block_id() {
        let event: StreamEvent = serde_json::from_value(serde_json::json!({
            "event_type": "provider_usage",
            "session_id": "s1",
            "model": "legacy-model",
            "input_tokens": null,
            "output_tokens": null,
            "estimated_cost_micros": null,
            "source": "anthropic",
            "reason": "provider_omitted"
        }))
        .unwrap();

        match event {
            StreamEvent::ProviderUsage {
                block_id,
                provider_id,
                cache_read_tokens,
                cache_creation_tokens,
                reasoning_tokens,
                pricing_source,
                ..
            } => {
                assert!(!block_id.is_empty());
                assert_eq!(provider_id, None);
                assert_eq!(cache_read_tokens, None);
                assert_eq!(cache_creation_tokens, None);
                assert_eq!(reasoning_tokens, None);
                assert_eq!(pricing_source, None);
            }
            other => panic!("expected provider_usage, got {other:?}"),
        }
    }

    /// Guardrail: every StreamEvent variant must have an `event_type` that
    /// matches the TypeScript discriminated union in src/lib/protocol.ts.
    #[test]
    fn stream_event_types_match_typescript_protocol() {
        // Minimal dummy instances — only the tag matters for this test.
        let cases: Vec<(StreamEvent, &'static str)> = vec![
            (
                StreamEvent::UserMessage {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    content: "c".into(),
                },
                "user_message",
            ),
            (
                StreamEvent::ThinkingStart {
                    session_id: "s".into(),
                    block_id: "b".into(),
                },
                "thinking_start",
            ),
            (
                StreamEvent::ThinkingChunk {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    content: "c".into(),
                },
                "thinking_chunk",
            ),
            (
                StreamEvent::ThinkingEnd {
                    session_id: "s".into(),
                    block_id: "b".into(),
                },
                "thinking_end",
            ),
            (
                StreamEvent::TextStart {
                    session_id: "s".into(),
                    block_id: "b".into(),
                },
                "text_start",
            ),
            (
                StreamEvent::TextChunk {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    content: "c".into(),
                },
                "text_chunk",
            ),
            (
                StreamEvent::TextEnd {
                    session_id: "s".into(),
                    block_id: "b".into(),
                },
                "text_end",
            ),
            (
                StreamEvent::ToolCallStart {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    tool_name: "t".into(),
                    tool_input: serde_json::Value::Null,
                },
                "tool_call_start",
            ),
            (
                StreamEvent::ToolCallResult {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    result: "r".into(),
                    is_error: false,
                    duration_ms: 0,
                },
                "tool_call_result",
            ),
            (
                StreamEvent::ToolCallEnd {
                    session_id: "s".into(),
                    block_id: "b".into(),
                },
                "tool_call_end",
            ),
            (
                StreamEvent::DiffView {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    file_path: "f".into(),
                    old_content: "o".into(),
                    new_content: "n".into(),
                },
                "diff_view",
            ),
            (
                StreamEvent::FileIo {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    path: "f".into(),
                    operation: "read".into(),
                    source: Some("executor".into()),
                },
                "file_io",
            ),
            (
                StreamEvent::ShellStart {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    command: "c".into(),
                },
                "shell_start",
            ),
            (
                StreamEvent::ShellOutput {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    content: "c".into(),
                },
                "shell_output",
            ),
            (
                StreamEvent::ShellEnd {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    exit_code: 0,
                },
                "shell_end",
            ),
            (
                StreamEvent::ConfirmAsk {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    question: "q".into(),
                    kind: "k".into(),
                    boundary: None,
                    permission_evidence: None,
                    replayed_interrupted: false,
                },
                "confirm_ask",
            ),
            (
                StreamEvent::ConfirmResponse {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    question: Some("q".into()),
                    kind: Some("k".into()),
                    boundary: None,
                    permission_evidence: None,
                    approved: Some(true),
                    responded_at_ms: 1,
                    reason: Some("user_response".into()),
                    replayed: false,
                },
                "confirm_response",
            ),
            (
                StreamEvent::PermissionDecision {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    evidence: crate::harness::permission_ledger::PermissionLedgerEvent {
                        kind: crate::harness::permission_ledger::PermissionLedgerEventKind::AutoApproved,
                        workspace_path: "/tmp/workspace".into(),
                        session_id: Some("s".into()),
                        risk_tier: crate::harness::permission_ledger::PermissionRiskTier::Normal,
                        affected_files: Vec::new(),
                        operation: "read_file".into(),
                        permission_mode: crate::harness::permissions::PermissionMode::ManualConfirm,
                        reason: "allow_rule".into(),
                    },
                },
                "permission_decision",
            ),
            (
                StreamEvent::ContextCompactStart {
                    session_id: "s".into(),
                    block_id: "b".into(),
                },
                "context_compact_start",
            ),
            (
                StreamEvent::ContextCompacted {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    summary: "sum".into(),
                    retained_messages: 0,
                    compacted_messages: 0,
                    estimated_tokens_before: 0,
                    estimated_tokens_after: 0,
                },
                "context_compacted",
            ),
            (
                StreamEvent::ContextCompactSkipped {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    reason: "history_too_short".into(),
                    retained_messages: 12,
                },
                "context_compact_skipped",
            ),
            (
                StreamEvent::MemorySelection {
                    session_id: "s".into(),
                    selected: vec![],
                },
                "memory_selection",
            ),
            (
                StreamEvent::MemoryCandidate {
                    session_id: "s".into(),
                    memory: WikiMemory {
                        id: "i".into(),
                        category: crate::memory::MemoryCategory::Preference,
                        scope: crate::memory::MemoryScope::Session,
                        status: crate::memory::MemoryStatus::Candidate,
                        title: "t".into(),
                        body: "b".into(),
                        project_path: None,
                        source_session_id: None,
                        source_message_ids: vec![],
                        confidence: 0.0,
                        created_at: "0".into(),
                        updated_at: "0".into(),
                        last_used_at: None,
                        use_count: 0,
                        tags: vec![],
                    },
                },
                "memory_candidate",
            ),
            (
                StreamEvent::MemoryUpdated {
                    session_id: "s".into(),
                    memory: WikiMemory {
                        id: "i".into(),
                        category: crate::memory::MemoryCategory::Preference,
                        scope: crate::memory::MemoryScope::Session,
                        status: crate::memory::MemoryStatus::Candidate,
                        title: "t".into(),
                        body: "b".into(),
                        project_path: None,
                        source_session_id: None,
                        source_message_ids: vec![],
                        confidence: 0.0,
                        created_at: "0".into(),
                        updated_at: "0".into(),
                        last_used_at: None,
                        use_count: 0,
                        tags: vec![],
                    },
                },
                "memory_updated",
            ),
            (
                StreamEvent::ForgeWikiContextSelected {
                    session_id: "s".into(),
                    selected: vec![],
                },
                "forge_wiki_context_selected",
            ),
            (
                StreamEvent::ForgeWikiUpdateProposed {
                    session_id: "s".into(),
                    proposal: ForgeWikiUpdateProposal {
                        id: "i".into(),
                        project_path: "p".into(),
                        session_id: None,
                        target_pages: vec![],
                        title: "t".into(),
                        summary: "s".into(),
                        patch_preview: None,
                        status: crate::forge_wiki::model::ForgeWikiProposalStatus::Pending,
                        created_at: "0".into(),
                    },
                },
                "forge_wiki_update_proposed",
            ),
            (
                StreamEvent::ForgeWikiUpdated {
                    session_id: "s".into(),
                    proposal: ForgeWikiUpdateProposal {
                        id: "i".into(),
                        project_path: "p".into(),
                        session_id: None,
                        target_pages: vec![],
                        title: "t".into(),
                        summary: "s".into(),
                        patch_preview: None,
                        status: crate::forge_wiki::model::ForgeWikiProposalStatus::Pending,
                        created_at: "0".into(),
                    },
                },
                "forge_wiki_updated",
            ),
            (
                StreamEvent::McpContextStatus {
                    session_id: "s".into(),
                    source_id: "src".into(),
                    status: "ok".into(),
                    message: None,
                },
                "mcp_context_status",
            ),
            (
                StreamEvent::WorkflowUpdated {
                    session_id: "s".into(),
                    state: WorkflowState {
                        session_id: "s".into(),
                        route: crate::workflow::WorkflowRoute::Direct,
                        phase: crate::workflow::WorkflowPhase::Idle,
                        beginner_label: "b".into(),
                        developer_label: "d".into(),
                        matched_signals: vec![],
                        reason: "r".into(),
                        gate: crate::workflow::WorkflowGate::None,
                        override_actions: vec![],
                        spec_path: None,
                        plan_path: None,
                        checkpoint_id: None,
                        updated_at: 0,
                    },
                },
                "workflow_updated",
            ),
            (
                StreamEvent::AgentTurnUpdated {
                    session_id: "s".into(),
                    state: AgentTurnProjection {
                        session_id: "s".into(),
                        status: crate::agent::turn_state::AgentTurnStatus::Started,
                        step_label: "l".into(),
                        workspace_path: "w".into(),
                        compact_count: 0,
                        verification_status:
                            crate::agent::turn_state::AgentVerificationStatus::NotNeeded,
                        model_rounds: 0,
                        tool_call_count: 0,
                        failed_tool_count: 0,
                        estimated_context_tokens: None,
                        compact_saved_tokens: 0,
                        stop_reason: None,
                    },
                },
                "agent_turn_updated",
            ),
            (
                StreamEvent::DeliverySummary {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    summary: DeliverySummary {
                        project_path: None,
                        preview_label: "p".into(),
                        checkpoint_label: "c".into(),
                        next_action: "n".into(),
                        verification_label: None,
                        verification_status: None,
                        verification_command: None,
                        record_label: None,
                        record_status: None,
                        record_target_pages: vec![],
                    },
                },
                "delivery_summary",
            ),
            (
                StreamEvent::SessionStarted {
                    session_id: "s".into(),
                    agent_type: "a".into(),
                    model: "m".into(),
                    context_window_tokens: None,
                },
                "session_started",
            ),
            (
                StreamEvent::SessionStatus {
                    session_id: "s".into(),
                    status: "idle".into(),
                },
                "session_status",
            ),
            (
                StreamEvent::SessionStopped {
                    session_id: "s".into(),
                    reason: "r".into(),
                },
                "session_stopped",
            ),
            (
                StreamEvent::Error {
                    session_id: "s".into(),
                    block_id: "b".into(),
                    message: "m".into(),
                    code: "c".into(),
                },
                "error",
            ),
            (
                StreamEvent::AgentA2AUpdated {
                    session_id: "s".into(),
                    state: AgentA2AProjection::default(),
                },
                "agent_a2a_updated",
            ),
            (
                StreamEvent::SubagentRuntimeEvent {
                    session_id: "s".into(),
                    loop_task_id: Some("loop".into()),
                    task_id: "task".into(),
                    event: SubagentRuntimePayload::Status {
                        status: "running".into(),
                        message: None,
                    },
                },
                "subagent_runtime_event",
            ),
            (
                StreamEvent::LoopRuntimeUpdated {
                    session_id: "gateway".into(),
                    loop_task_id: "loop".into(),
                    task: LoopTaskRecord::new_for_test("loop", "goal"),
                },
                "loop_runtime_updated",
            ),
            (
                StreamEvent::EcosystemChanged {
                    session_id: "global".into(),
                    item_id: "skill-a".into(),
                    action: "enabled".into(),
                    enabled: Some(true),
                },
                "ecosystem_changed",
            ),
            (
                StreamEvent::Usage {
                    session_id: "s".into(),
                    input_tokens: 0,
                    output_tokens: 0,
                    estimated_cost_usd: 0.0,
                },
                "usage",
            ),
            (
                StreamEvent::ProviderUsage {
                    session_id: "s".into(),
                    block_id: "usage-1".into(),
                    provider_id: Some("anthropic".into()),
                    model: Some("m".into()),
                    input_tokens: Some(1),
                    output_tokens: Some(2),
                    cache_read_tokens: None,
                    cache_creation_tokens: None,
                    reasoning_tokens: None,
                    estimated_cost_micros: None,
                    pricing_source: None,
                    source: Some("anthropic".into()),
                    reason: ProviderUsageReason::PricingUnknown,
                },
                "provider_usage",
            ),
            (
                StreamEvent::RecoveryNotice {
                    session_id: "s".into(),
                    notice_id: "n".into(),
                    title: "t".into(),
                    message: "m".into(),
                    reason: "r".into(),
                    recoverable: true,
                },
                "recovery_notice",
            ),
            (
                StreamEvent::DiagnosticsUpdate {
                    session_id: "s".into(),
                    ok: true,
                    pass_count: 5,
                    warn_count: 1,
                    fail_count: 0,
                    report_json: None,
                },
                "diagnostics_update",
            ),
            (
                StreamEvent::HealthAlert {
                    session_id: "s".into(),
                    alert_id: "a".into(),
                    level: "warn".into(),
                    title: "t".into(),
                    message: "m".into(),
                    remediation: Some("r".into()),
                },
                "health_alert",
            ),
        ];

        for (event, expected) in cases {
            assert_eq!(
                event.event_type(),
                expected,
                "StreamEvent variant must serialize with event_type={expected}"
            );
            let json = serde_json::to_value(&event).unwrap();
            let actual = json["event_type"].as_str().unwrap();
            assert_eq!(
                actual, expected,
                "serde tag mismatch: expected {expected}, got {actual}"
            );
        }
    }
}
