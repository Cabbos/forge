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
export type PermissionRiskTier = "normal" | "caution" | "high";
export type PermissionMode = "manual_confirm" | "trust_current_project" | "full_access";
export type PermissionLedgerEventKind =
  | "mode_changed"
  | "manual_required"
  | "auto_approved"
  | "blocked_external_path"
  | "blocked_sensitive_path"
  | "blocked_policy"
  | "user_approved"
  | "user_declined";

export interface PermissionLedgerEvent {
  kind: PermissionLedgerEventKind;
  workspace_path: string;
  session_id?: string | null;
  risk_tier: PermissionRiskTier;
  affected_files: string[];
  operation: string;
  permission_mode: PermissionMode;
  reason: string;
}

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

export type PreparedTurnPermissionMode = "manual_confirm" | "trust_current_project" | "full_access";

export interface PreparedTurnContextSource {
  kind: string;
  label: string;
  reason: string;
  estimated_tokens: number;
  injected: boolean;
}

export type ContextUsageBucketKind =
  | "visible_input"
  | "hidden_system"
  | "memory"
  | "files"
  | "project_records"
  | "compacted_transcript"
  | "connector_context"
  | "reserved_output";

export interface ContextUsageBucket {
  kind: ContextUsageBucketKind;
  label: string;
  estimated_tokens: number;
  source_count: number;
  injected: boolean;
}

export interface ContextUsageEstimate {
  used_tokens: number;
  context_window_tokens?: number | null;
  percent_used?: number | null;
  reserved_output_tokens: number;
  sources: PreparedTurnContextSource[];
  buckets?: ContextUsageBucket[];
}

export interface PreparedTurnMemoryAudit {
  memory_id: string;
  source: string;
  source_id: string;
  kind: string;
  score: number;
  reason: string;
  project_match: boolean;
  profile_match: boolean;
  injected: boolean;
}

export type RecallDecision =
  | "injected"
  | "duplicate"
  | "excluded_status"
  | "excluded_project"
  | "excluded_profile"
  | "no_relevance_signal"
  | "low_signal_query"
  | "budget_exceeded";

export interface RecallCandidateAudit {
  memory_id: string;
  source: string;
  source_id: string;
  kind: string;
  status: string;
  score: number;
  reason: string;
  decision: RecallDecision;
  project_match: boolean;
  profile_match: boolean;
  estimated_tokens: number;
  rank?: number | null;
}

export interface RecallBudget {
  candidate_count: number;
  injection_limit: number;
  budget_tokens: number;
  estimated_injected_tokens: number;
  injected_count: number;
}

export interface RecallPlan {
  selected_memory_ids: string[];
  candidates: RecallCandidateAudit[];
  budget: RecallBudget;
}

export interface PreparedTurn {
  session_id: string;
  project_path: string;
  user_text: string;
  activation_text: string;
  selected_memory_ids: string[];
  selected_memory_audit?: PreparedTurnMemoryAudit[];
  memory_recall_plan?: RecallPlan | null;
  selected_project_record_ids: string[];
  workflow_route: string;
  workflow_phase: string;
  slash_command?: string | null;
  permission_mode: PreparedTurnPermissionMode;
  context_estimate: ContextUsageEstimate;
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

export type AgentA2AChildEventKind =
  | "assigned"
  | "lease_claimed"
  | "started"
  | "progress"
  | "file_fact"
  | "patch_proposed"
  | "waiting_review"
  | "completed"
  | "failed"
  | "abandoned"
  | "recovered";

export interface AgentA2AChildRuntimeEvent {
  kind: AgentA2AChildEventKind;
  label: string;
  detail: string;
  created_at_ms: number;
}

export interface AgentA2AChildCapsule {
  capsule_id: string;
  parent_task_id: string;
  child_task_id: string;
  child_goal: string;
  status: string;
  artifact_titles: string[];
  changed_files: string[];
  review_decision?: string | null;
  failure_reason?: string | null;
  next_action: string;
  estimated_tokens: number;
}

export type AgentA2AReviewGateKind =
  | "approved"
  | "changes_requested"
  | "rejected"
  | "stale_review"
  | "wrong_parent"
  | "missing_evidence"
  | "waiting_review";

export interface AgentA2AReviewGateProjection {
  kind: AgentA2AReviewGateKind;
  label: string;
  reason: string;
  completion_impact: string;
  parent_task_id?: string | null;
  child_task_id: string;
  reviewed_at_ms?: number | null;
}

export type AgentA2ARecoveryActionKind =
  | "retry"
  | "abandon"
  | "reassign"
  | "inspect_worktree";

export interface AgentA2ARecoveryActionSuggestion {
  action: AgentA2ARecoveryActionKind;
  label: string;
  reason: string;
  requires_human_approval: boolean;
  retryable: boolean;
  next_attempt?: number | null;
}

export interface AgentA2ATaskProjection {
  task_id: string;
  agent_id: string;
  role: string;
  execution_mode: string;
  status: string;
  title: string;
  messages: AgentA2AMessageProjection[];
  latest_message: string | null;
  failure_message: string | null;
  updated_at_ms: number;
  artifact_count: number;
  latest_artifact_kind: string | null;
  latest_artifact_title: string | null;
  // WorktreeWorker-specific metadata
  needs_human_review: boolean | null;
  reason_codes: string[];
  tests_passed: boolean | null;
  diff_truncated: boolean | null;
  worktree_path: string | null;
  cleaned_up: boolean | null;
  suggested_action: string | null;
  review_decision?: string | null;
  reviewed_at_ms?: number | null;
  // Phase 4-A enriched fields — derived from AgentTaskRecord / artifacts.
  parent_task_id: string | null;
  child_task_ids: string[];
  created_at_ms: number;
  started_at_ms: number | null;
  ended_at_ms: number | null;
  duration_ms: number | null;
  retryable: boolean | null;
  failure_kind: string | null;
  resume_note: string | null;
  latest_progress: string | null;
  // Phase 4-C — durable WorktreeWorker lease / retry state.
  lease_owner: string | null;
  lease_acquired_at_ms: number | null;
  lease_expires_at_ms: number | null;
  last_heartbeat_at_ms: number | null;
  attempt_count: number;
  max_attempts: number;
  // A2A runtime contract: compact, replayable child event facts.
  runtime_events?: AgentA2AChildRuntimeEvent[];
  // Parent-consumable summaries of direct child tasks.
  child_capsules?: AgentA2AChildCapsule[];
  // A2A Review Gate V2 — typed task-local review facts.
  review_gate?: AgentA2AReviewGateProjection | null;
  // A2A Failure Recovery — suggestions only.
  recovery_actions?: AgentA2ARecoveryActionSuggestion[];
  // Phase 4-B — diff-derived file visibility (safe: parsed from existing artifacts).
  diff_available: boolean | null;
  changed_file_count: number | null;
  changed_files: string[];
  test_report_excerpt: string | null;
}

export interface AgentA2AMessageProjection {
  message_id: string;
  kind: string;
  content: string;
  created_at_ms: number;
}

export interface AgentA2AProjection {
  running_count: number;
  completed_count: number;
  failed_count: number;
  interrupted_count: number;
  tasks: AgentA2ATaskProjection[];
}

export type ProviderUsageReason = "provider_reported" | "provider_omitted" | "pricing_unknown";

export type HeadlessResumeMode = "disabled" | "require_human_approval" | "approved_for_task";

export type HeadlessOwnerRunState =
  | "requested"
  | "denied"
  | "ready"
  | "lease_acquired"
  | "dry_run_waiting"
  | "fake_running"
  | "running"
  | "waiting_for_input"
  | "interrupted"
  | "cancelled"
  | "expired"
  | "completed"
  | "failed";

export type HeadlessOwnerSnapshotSource =
  | "unavailable"
  | "current_desktop_session"
  | "persisted_session_snapshot"
  | "workspace_snapshot"
  | "restored_headless_snapshot";

export type HeadlessOwnerExecutorKind =
  | "none"
  | "dry_run"
  | "fake_executor"
  | "agent_session_adapter";

export interface HeadlessOwnerRun {
  owner_run_id: string;
  task_id: string;
  session_id?: string | null;
  lease_id: string;
  attempt: number;
  state?: HeadlessOwnerRunState;
  snapshot_source?: HeadlessOwnerSnapshotSource;
  snapshot_ref?: string | null;
  human_gate_id: string;
  policy_decision_id: string;
  budget_snapshot_id: string;
  idempotency_key: string;
  correlation_id: string;
  causation_id?: string | null;
  requested_by: string;
  requested_at_ms: number;
  heartbeat_at_ms?: number | null;
  expires_at_ms: number;
  cancellation_reason?: string | null;
  waiting_reason?: string | null;
  executor_kind?: HeadlessOwnerExecutorKind;
  evidence_refs?: string[];
}

export interface HeadlessResumeApproval {
  task_id: string;
  approved_by: string;
  approved_at_ms: number;
  scope: string;
  expires_at_ms: number;
}

export type SubagentRuntimePayload =
  | { type: "started"; role: string }
  | { type: "status"; status: string; message?: string | null }
  | { type: "file_io"; path: string; operation: string }
  | {
      type: "usage_recorded";
      provider_id?: string | null;
      model?: string | null;
      source?: string | null;
      reason?: ProviderUsageReason | null;
      input_tokens?: number | null;
      output_tokens?: number | null;
      cache_read_tokens?: number | null;
      cache_creation_tokens?: number | null;
      reasoning_tokens?: number | null;
      estimated_cost_micros?: number | null;
      pricing_source?: string | null;
    }
  | { type: "ended"; status: string }
  | { type: "failed"; reason: string }
  | { type: "interrupted"; reason: string };

export interface LoopTaskRecord {
  id: string;
  goal: string;
  session_id?: string | null;
  profile_id?: string | null;
  workspace_path?: string | null;
  status: string;
  owner: Record<string, unknown>;
  policy: Record<string, unknown>;
  headless_resume_mode?: HeadlessResumeMode;
  headless_resume_approval?: HeadlessResumeApproval | null;
  headless_owner_runs?: HeadlessOwnerRun[];
  budget: Record<string, unknown>;
  completion_contract: Record<string, unknown>;
  created_at_ms: number;
  updated_at_ms: number;
  lease?: Record<string, unknown> | null;
  open_gates?: unknown[];
  evidence?: unknown[];
  policy_decisions?: unknown[];
  latest_budget_snapshot?: Record<string, unknown> | null;
  latest_usage_ledger?: Record<string, unknown> | null;
  recovery_state?: {
    kind?: "orphaned" | "interrupted" | string;
    recoverable?: boolean;
    reason?: string;
    notice?: string;
    recorded_at_ms?: number;
    source_event_id?: string | null;
  } | null;
  latest_event_id?: string | null;
  outcome?: Record<string, unknown> | null;
  completion_result?: {
    status?: string;
    reasons?: string[];
    review_status?: string;
    commit_eligible?: boolean;
    commit_blockers?: string[];
    human_gate_id?: string | null;
    last_review_decision?: Record<string, unknown> | null;
    eligibility_facts?: LoopCompletionEligibilityFacts;
  } | Record<string, unknown> | null;
}

export type CompletionFactStatus = "satisfied" | "missing" | "blocked" | "not_required" | "unknown";

export interface CompletionFactBucket {
  status?: CompletionFactStatus | string;
  reason?: string;
  evidence_ids?: string[];
  blockers?: string[];
}

export interface LoopCompletionEligibilityFacts {
  verification?: CompletionFactBucket;
  changed_file_scope?: CompletionFactBucket;
  permission?: CompletionFactBucket;
  review?: CompletionFactBucket;
  docs?: CompletionFactBucket;
  eval?: CompletionFactBucket;
  residual_risk?: CompletionFactBucket;
  commit?: CompletionFactBucket;
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
  | { event_type: "file_io"; session_id: string; block_id: string; path: string; operation: string; source?: string | null }
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
      permission_evidence?: PermissionLedgerEvent | null;
      replayed_interrupted?: boolean;
    }
  | {
      event_type: "confirm_response";
      session_id: string;
      block_id: string;
      question?: string | null;
      kind?: string | null;
      boundary?: WriteBoundary | null;
      permission_evidence?: PermissionLedgerEvent | null;
      approved: boolean | null;
      responded_at_ms: number;
      reason?: string | null;
      replayed?: boolean;
    }
  | { event_type: "permission_decision"; session_id: string; block_id: string; evidence: PermissionLedgerEvent }
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
  | { event_type: "turn_prepared"; session_id: string; prepared: PreparedTurn }
  | { event_type: "agent_turn_updated"; session_id: string; state: AgentTurnProjection }
  | { event_type: "agent_a2a_updated"; session_id: string; state: AgentA2AProjection }
  | {
      event_type: "subagent_runtime_event";
      session_id: string;
      loop_task_id?: string | null;
      task_id: string;
      event: SubagentRuntimePayload;
    }
  | {
      event_type: "loop_runtime_updated";
      session_id: string;
      loop_task_id: string;
      task: LoopTaskRecord;
    }
  | {
      event_type: "ecosystem_changed";
      session_id: string;
      item_id: string;
      action: string;
      enabled?: boolean | null;
    }
  | { event_type: "delivery_summary"; session_id: string; block_id: string; summary: DeliverySummary }
  // ── Session Status ──
  | { event_type: "session_started"; session_id: string; agent_type: string; model: string; context_window_tokens?: number | null }
  | { event_type: "session_status"; session_id: string; status: string }
  | { event_type: "session_stopped"; session_id: string; reason: string }
  | { event_type: "error"; session_id: string; block_id: string; message: string; code: string }
  | { event_type: "usage"; session_id: string; input_tokens: number; output_tokens: number; estimated_cost_usd: number }
  | {
      event_type: "provider_usage";
      session_id: string;
      block_id?: string | null;
      provider_id?: string | null;
      model?: string | null;
      input_tokens: number | null;
      output_tokens: number | null;
      cache_read_tokens?: number | null;
      cache_creation_tokens?: number | null;
      reasoning_tokens?: number | null;
      estimated_cost_micros: number | null;
      pricing_source?: string | null;
      source?: string | null;
      reason: ProviderUsageReason;
    }
  // ── Recovery Notice ──
  | {
      event_type: "recovery_notice";
      session_id: string;
      notice_id: string;
      title: string;
      message: string;
      reason: string;
      recoverable: boolean;
    }
  // ── Diagnostics / Health ──
  | {
      event_type: "diagnostics_update";
      session_id: string;
      ok: boolean;
      pass_count: number;
      warn_count: number;
      fail_count: number;
      report_json?: string | null;
    }
  | {
      event_type: "health_alert";
      session_id: string;
      alert_id: string;
      level: string;
      title: string;
      message: string;
      remediation?: string | null;
    };

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

export interface SessionUsageLedgerState {
  providerId: string | null;
  model: string | null;
  source: string | null;
  reason: ProviderUsageReason | "legacy_usage";
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheCreationTokens: number | null;
  reasoningTokens: number | null;
  estimatedCostMicros: number | null;
  pricingSource: string | null;
  costUsd: number | null;
  hasUnknownInputTokens: boolean;
  hasUnknownOutputTokens: boolean;
  hasUnknownCost: boolean;
  lastEventType: "provider_usage" | "usage";
  lastProviderUsageBlockId: string | null;
  legacyDuplicateIgnored: boolean;
  updatedAt: number;
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
  status: "running" | "stopped" | "error" | "resuming";
  streaming: boolean;
  blocks: BlockState[];
  costUsd: number;
  contextUsage?: ContextUsageState | null;
  usageLedger?: SessionUsageLedgerState | null;
}

export type AgentType = "claude" | "codex" | "hermes";
export type ToolType = "claude" | "codex" | "hermes" | "bash";
