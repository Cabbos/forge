use crate::forge_wiki::model::{ForgeWikiUpdateProposal, SelectedForgeWikiPage};
use crate::harness::write_boundary::WriteBoundary;
use crate::memory::{SelectedContextMemory, WikiMemory};
use crate::workflow::WorkflowState;
use serde::{Deserialize, Serialize};

/// Streaming events emitted from Rust backend to frontend.
/// Mirrors the TypeScript protocol in src/lib/protocol.ts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum StreamEvent {
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
    },

    // ── Context Management ──
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

    // ── Workflow Routing ──
    #[serde(rename = "workflow_updated")]
    WorkflowUpdated {
        session_id: String,
        state: WorkflowState,
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
    #[serde(rename = "usage")]
    Usage {
        session_id: String,
        input_tokens: u32,
        output_tokens: u32,
        estimated_cost_usd: f64,
    },
}

impl StreamEvent {
    /// Returns the session_id for this event.
    pub fn session_id(&self) -> &str {
        use StreamEvent::*;
        match self {
            ThinkingStart { session_id, .. }
            | ThinkingChunk { session_id, .. }
            | ThinkingEnd { session_id, .. }
            | TextStart { session_id, .. }
            | TextChunk { session_id, .. }
            | TextEnd { session_id, .. }
            | ToolCallStart { session_id, .. }
            | ToolCallResult { session_id, .. }
            | ToolCallEnd { session_id, .. }
            | DiffView { session_id, .. }
            | ShellStart { session_id, .. }
            | ShellOutput { session_id, .. }
            | ShellEnd { session_id, .. }
            | ConfirmAsk { session_id, .. }
            | ContextCompacted { session_id, .. }
            | MemorySelection { session_id, .. }
            | MemoryCandidate { session_id, .. }
            | MemoryUpdated { session_id, .. }
            | ForgeWikiContextSelected { session_id, .. }
            | ForgeWikiUpdateProposed { session_id, .. }
            | ForgeWikiUpdated { session_id, .. }
            | WorkflowUpdated { session_id, .. }
            | SessionStarted { session_id, .. }
            | SessionStatus { session_id, .. }
            | SessionStopped { session_id, .. }
            | Error { session_id, .. }
            | Usage { session_id, .. } => session_id,
        }
    }
}
