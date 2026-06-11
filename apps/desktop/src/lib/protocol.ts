// TypeScript mirror of Rust StreamEvent enum (protocol/events.rs)

export type MemoryCategory = "preference" | "project_fact" | "decision" | "task_state";
export type MemoryScope = "session" | "user_profile" | "project" | "document";
export type MemoryStatus = "candidate" | "accepted" | "pinned" | "forgotten" | "archived";

export interface WikiMemory {
  id: string;
  category: MemoryCategory;
  scope: MemoryScope;
  status: MemoryStatus;
  title: string;
  body: string;
  project_path: string | null;
  source_session_id: string | null;
  source_message_ids: string[];
  confidence: number;
  created_at: string;
  updated_at: string;
  last_used_at: string | null;
  use_count: number;
  tags: string[];
}

export interface SelectedContextMemory {
  memory_id: string;
  title: string;
  body: string;
  category: MemoryCategory;
  scope: MemoryScope;
  score: number;
  reason: string;
  injected: boolean;
}

export interface MemoryPatch {
  title?: string | null;
  body?: string | null;
  status?: MemoryStatus | null;
  tags?: string[] | null;
}

export type ForgeWikiPageKind =
  | "index"
  | "schema"
  | "sources"
  | "decisions"
  | "tasks"
  | "log"
  | "custom";

export interface ForgeWikiPage {
  id: string;
  project_path: string;
  path: string;
  title: string;
  kind: ForgeWikiPageKind;
  summary: string | null;
  updated_at: string | null;
  token_estimate: number | null;
}

export interface ForgeWikiState {
  project_path: string;
  exists: boolean;
  wiki_dir: string;
  pages: ForgeWikiPage[];
  message: string;
}

export interface SelectedForgeWikiPage {
  page_id: string;
  title: string;
  path: string;
  kind: ForgeWikiPageKind;
  summary: string;
  score: number;
  reason: string;
  injected: boolean;
}

export type ForgeWikiProposalStatus = "pending" | "accepted" | "discarded";

export interface ForgeWikiUpdateProposal {
  id: string;
  project_path: string;
  session_id: string | null;
  target_pages: string[];
  title: string;
  summary: string;
  patch_preview: string | null;
  status: ForgeWikiProposalStatus;
  created_at: string;
}

export type WorkflowRoute =
  | "direct"
  | "light"
  | "workflow"
  | "strict_workflow"
  | "recovery"
  | "verification";

export type WorkflowPhase =
  | "idle"
  | "classifying"
  | "clarifying"
  | "designing"
  | "spec"
  | "planning"
  | "executing"
  | "debugging"
  | "verifying"
  | "done"
  | "blocked";

export type WorkflowGate = "none" | "soft" | "approval_required";

export type WorkflowOverrideAction = "direct" | "plan_first" | "debug" | "verify";

export interface WorkflowState {
  session_id: string;
  route: WorkflowRoute;
  phase: WorkflowPhase;
  beginner_label: string;
  developer_label: string;
  matched_signals: string[];
  reason: string;
  gate: WorkflowGate;
  override_actions: WorkflowOverrideAction[];
  spec_path: string | null;
  plan_path: string | null;
  checkpoint_id: string | null;
  updated_at: number;
}

export type WriteBoundaryRisk = "normal" | "caution" | "high";

export interface WriteBoundary {
  title: string;
  target_label?: string | null;
  workspace_name: string;
  workspace_path: string;
  operation: string;
  affected_files: string[];
  command?: string | null;
  impact: string;
  risk: WriteBoundaryRisk;
  recovery: string;
  checkpoint_status?: "ready" | "pending" | "unavailable" | "missing" | null;
  warning?: string | null;
}

export interface DeliverySummary {
  project_path: string | null;
  preview_label: string;
  checkpoint_label: string;
  next_action: string;
  verification_label?: string | null;
  verification_status?: string | null;
  verification_command?: string | null;
  record_label?: string | null;
  record_status?: string | null;
  record_target_pages?: string[];
}

export interface McpContextStatus {
  source_id: string;
  status: "ready" | "failed";
  message?: string | null;
}

export type AgentTurnStatus =
  | "started"
  | "gathering_context"
  | "calling_model"
  | "running_tools"
  | "verifying"
  | "completed"
  | "failed"
  | "cancelled";

export type AgentVerificationStatus =
  | "not_needed"
  | "skipped"
  | "running"
  | "passed"
  | "failed"
  | "error";

export interface AgentTurnProjection {
  session_id: string;
  status: AgentTurnStatus;
  step_label: string;
  workspace_path: string;
  compact_count: number;
  verification_status: AgentVerificationStatus;
  model_rounds: number;
  tool_call_count: number;
  failed_tool_count: number;
  estimated_context_tokens?: number | null;
  stop_reason?: string | null;
  compact_saved_tokens: number;
}

export interface AgentA2ATaskProjection {
  task_id: string;
  agent_id: string;
  role: string;
  execution_mode: string;
  status: string;
  title: string;
  latest_message: string | null;
  failure_message: string | null;
  updated_at_ms: number;
}

export interface AgentA2AProjection {
  running_count: number;
  completed_count: number;
  failed_count: number;
  interrupted_count: number;
  tasks: AgentA2ATaskProjection[];
}

export type StreamEvent =
  // ── Transcript ──
  | { event_type: "user_message"; session_id: string; block_id: string; content: string }
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
  | {
      event_type: "confirm_ask";
      session_id: string;
      block_id: string;
      question: string;
      kind: string;
      boundary?: WriteBoundary | null;
    }
  // ── Context Management ──
  | {
      event_type: "context_compact_start";
      session_id: string;
      block_id: string;
    }
  | {
      event_type: "context_compacted";
      session_id: string;
      block_id: string;
      summary: string;
      retained_messages: number;
      compacted_messages: number;
      estimated_tokens_before: number;
      estimated_tokens_after: number;
    }
  | {
      event_type: "context_compact_skipped";
      session_id: string;
      block_id: string;
      reason: string;
      retained_messages: number;
    }
  // ── Saved Context ──
  | { event_type: "memory_selection"; session_id: string; selected: SelectedContextMemory[] }
  | { event_type: "memory_candidate"; session_id: string; memory: WikiMemory }
  | { event_type: "memory_updated"; session_id: string; memory: WikiMemory }
  // ── Project Records ──
  | { event_type: "forge_wiki_context_selected"; session_id: string; selected: SelectedForgeWikiPage[] }
  | { event_type: "forge_wiki_update_proposed"; session_id: string; proposal: ForgeWikiUpdateProposal }
  | { event_type: "forge_wiki_updated"; session_id: string; proposal: ForgeWikiUpdateProposal }
  | { event_type: "mcp_context_status"; session_id: string; source_id: string; status: "ready" | "failed"; message?: string | null }
  | { event_type: "workflow_updated"; session_id: string; state: WorkflowState }
  | { event_type: "agent_turn_updated"; session_id: string; state: AgentTurnProjection }
  | { event_type: "agent_a2a_updated"; session_id: string; state: AgentA2AProjection }
  | { event_type: "delivery_summary"; session_id: string; block_id: string; summary: DeliverySummary }
  // ── Session Status ──
  | { event_type: "session_started"; session_id: string; agent_type: string; model: string; context_window_tokens?: number | null }
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

export interface ContextUsageState {
  usedTokens: number | null;
  contextWindowTokens: number | null;
  percentUsed: number | null;
  source: "provider_usage" | "local_estimate";
  lastUpdatedAt: number;
  lastCompactedAt?: number | null;
  compactedFromTokens?: number | null;
  compactedToTokens?: number | null;
}

// Session state
export interface SessionState {
  id: string;
  agentType: string;
  model: string;
  workingDir?: string | null;
  workspaceId?: string | null;
  createdAt?: number | null;
  updatedAt?: number | null;
  contextWindowTokens?: number | null;
  status: "running" | "stopped" | "error";
  streaming: boolean;
  blocks: BlockState[];
  costUsd: number;
  contextUsage?: ContextUsageState | null;
}

export type AgentType = "claude" | "codex" | "hermes";
export type ToolType = "claude" | "codex" | "hermes" | "bash";
