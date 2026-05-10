// TypeScript mirror of Rust StreamEvent enum (protocol/events.rs)

export type StreamEvent =
  // ── AI Thinking ──
  | { event_type: "thinking_start"; session_id: string; block_id: string }
  | { event_type: "thinking_chunk"; session_id: string; block_id: string; content: string }
  | { event_type: "thinking_end"; session_id: string; block_id: string }
  // ── AI Text Response ──
  | { event_type: "text_start"; session_id: string; block_id: string }
  | { event_type: "text_chunk"; session_id: string; block_id: string; content: string }
  | { event_type: "text_end"; session_id: string; block_id: string }
  // ── Tool Calls ──
  | { event_type: "tool_call_start"; session_id: string; block_id: string; tool_name: string; tool_input: unknown }
  | { event_type: "tool_call_result"; session_id: string; block_id: string; result: string; is_error: boolean; duration_ms: number }
  | { event_type: "tool_call_end"; session_id: string; block_id: string }
  // ── File Diff ──
  | { event_type: "diff_view"; session_id: string; block_id: string; file_path: string; old_content: string; new_content: string }
  // ── Shell Commands ──
  | { event_type: "shell_start"; session_id: string; block_id: string; command: string }
  | { event_type: "shell_output"; session_id: string; block_id: string; content: string }
  | { event_type: "shell_end"; session_id: string; block_id: string; exit_code: number }
  // ── Permission Confirmations ──
  | { event_type: "confirm_ask"; session_id: string; block_id: string; question: string; kind: string }
  // ── Session Status ──
  | { event_type: "session_started"; session_id: string; agent_type: string; model: string }
  | { event_type: "session_status"; session_id: string; status: string }
  | { event_type: "session_stopped"; session_id: string; reason: string }
  | { event_type: "error"; session_id: string; block_id: string; message: string; code: string }
  | { event_type: "usage"; session_id: string; input_tokens: number; output_tokens: number; estimated_cost_usd: number };

// Block state for accumulating streaming chunks
export interface BlockState {
  block_id: string;
  event_type: string;
  content: string;
  metadata: Record<string, unknown>;
  isComplete: boolean;
}

// Session state
export interface SessionState {
  id: string;
  agentType: string;
  model: string;
  status: "running" | "stopped" | "error";
  blocks: BlockState[];
  costUsd: number;
}

export type AgentType = "claude" | "codex" | "hermes";
export type ToolType = "claude" | "codex" | "hermes" | "bash";
