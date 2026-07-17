import type {
  DeliverySummary,
  ForgeWikiPage,
  ForgeWikiState,
  ForgeWikiUpdateProposal,
  AgentA2AProjection,
  HeadlessOwnerRun,
  LoopTaskRecord,
  MemoryPatch,
  MemoryScope,
  SelectedContextMemory,
  SelectedForgeWikiPage,
  StreamEvent,
  WikiMemory,
  WorkflowOverrideAction,
  WorkflowState,
} from "../protocol";

export type {
  DeliverySummary,
  ForgeWikiPage,
  ForgeWikiState,
  ForgeWikiUpdateProposal,
  AgentA2AProjection,
  HeadlessOwnerRun,
  LoopTaskRecord,
  MemoryPatch,
  MemoryScope,
  SelectedContextMemory,
  SelectedForgeWikiPage,
  StreamEvent,
  WikiMemory,
  WorkflowOverrideAction,
  WorkflowState,
};

export interface SessionCreated {
  session_id: string;
  provider?: string;
  model?: string;
  missing_api_key?: boolean;
}

export interface SessionInfo {
  id: string;
  provider: string;
  model: string;
  status: string;
  created_at: string;
  working_dir?: string | null;
  created_at_ms?: number | null;
  updated_at_ms?: number | null;
  context_window_tokens?: number | null;
  latest_workflow?: WorkflowState | null;
  latest_delivery?: DeliverySummary | null;
}

export interface SessionSnapshotStoreStats {
  total_snapshots: number;
  corrupted_snapshots: number;
  total_bytes: number;
  oldest_updated_at_ms?: number | null;
  newest_updated_at_ms?: number | null;
  by_provider: Record<string, number>;
  by_workspace: Record<string, number>;
}

export interface SessionSnapshotSummary {
  session_id: string;
  provider: string;
  model: string;
  working_dir: string;
  summary?: string | null;
  created_at_ms: number;
  updated_at_ms: number;
  message_count: number;
}

export interface SessionSnapshotPruneReport {
  deleted_session_ids: string[];
  kept_session_ids: string[];
  skipped_corrupted: number;
}

export interface PruneSessionStoreInput {
  keepRecent: number;
  olderThanMs?: number | null;
}

export interface RenameSessionSnapshotInput {
  sessionId: string;
  summary: string;
}

export type PermissionRuleDecision = "allow" | "deny";
export type PermissionMode = "manual_confirm" | "trust_current_project" | "full_access";

export interface PermissionRuleView {
  tool_name: string;
  decision: PermissionRuleDecision;
  created_at: string;
}

export interface PermissionModeState {
  mode: PermissionMode;
  workspace_path: string | null;
  session_scoped: boolean;
}

export interface SetPermissionRuleInput {
  toolName: string;
  decision: PermissionRuleDecision;
}

export interface SetPermissionModeInput {
  sessionId: string;
  mode: PermissionMode;
  workspacePath?: string | null;
}

export interface AppWorkspaceMetadata {
  id: string;
  name: string;
  path: string;
  lastOpenedAt: number;
}

export interface AppMetadata {
  workspaces: AppWorkspaceMetadata[];
  activeWorkspaceId?: string | null;
  activeSessionId?: string | null;
  selectedProvider?: string | null;
  selectedModel?: string | null;
}

export interface FilePreviewLine {
  number: number;
  content: string;
  is_target: boolean;
}

export interface FilePreview {
  path: string;
  display_path: string;
  requested_line: number | null;
  start_line: number;
  total_lines: number;
  lines: FilePreviewLine[];
}

export type WorkspaceReviewFileStatus = "added" | "modified" | "renamed" | "deleted";

export interface WorkspaceReviewFile {
  path: string;
  status: WorkspaceReviewFileStatus;
  additions: number;
  deletions: number;
}

export interface WorkspaceReview {
  working_dir: string;
  patch: string;
  files: WorkspaceReviewFile[];
  truncated: boolean;
}

export interface ProjectRuntimeStatus {
  working_dir: string;
  has_package_json: boolean;
  package_manager: string;
  dev_script: string | null;
  command: string | null;
  port: number;
  url: string;
  running: boolean;
  managed: boolean;
  pid: number | null;
  can_start: boolean;
  can_stop: boolean;
  can_open: boolean;
  message: string;
  logs: string[];
}

export interface ProjectCheckpoint {
  id: string;
  created_at: number;
  head: string;
  status: string;
  restorable: boolean;
  untracked_file_count: number;
  skipped_untracked_file_count: number;
}

export interface ProjectCheckpointStatus {
  working_dir: string;
  is_git_repo: boolean;
  dirty: boolean;
  last_checkpoint: ProjectCheckpoint | null;
  restorable: boolean;
  snapshot_warning?: string | null;
  message: string;
}

export type ContinuityExperienceKind =
  | "lesson"
  | "bug_pattern"
  | "workflow"
  | "decision"
  | "preference"
  | "project_fact";

export type ContinuityExperienceStatus =
  | "candidate"
  | "accepted"
  | "pinned"
  | "forgotten"
  | "archived";

export interface ContinuityExperience {
  id: string;
  kind: ContinuityExperienceKind;
  status: ContinuityExperienceStatus;
  title: string;
  body: string;
  project_path?: string | null;
  source_session_id?: string | null;
  confidence: number;
  created_at_ms: number;
  updated_at_ms: number;
  tags: string[];
}

export type UnifiedMemorySource = "wiki_memory" | "memory_fact" | "continuity_experience";
export type UnifiedMemoryActionKind =
  | "archive"
  | "restore"
  | "forget"
  | "pin"
  | "unpin"
  | "mark_wrong_project"
  | "mark_low_value"
  | "edit";
export type UnifiedMemoryListFilter = "current" | "archived";
export type UnifiedMemoryKind =
  | "preference"
  | "project_fact"
  | "decision"
  | "task_state"
  | "lesson"
  | "bug_pattern"
  | "workflow";
export type UnifiedMemoryScope = "session" | "user_profile" | "project" | "document";
export type UnifiedMemoryStatus = "candidate" | "accepted" | "pinned" | "forgotten" | "archived";
export type UnifiedMemoryVisibility = "user_visible" | "hidden_context" | "audit_only";

export interface UnifiedMemoryProvenance {
  owner: string;
  storage: string;
  source_label: string;
}

export interface UnifiedMemoryRecord {
  id: string;
  source: UnifiedMemorySource;
  source_id: string;
  kind: UnifiedMemoryKind;
  status: UnifiedMemoryStatus;
  scope: UnifiedMemoryScope;
  title: string;
  body: string;
  project_path?: string | null;
  profile_id?: string | null;
  source_session_id?: string | null;
  confidence: number;
  created_at_ms: number;
  updated_at_ms: number;
  tags: string[];
  visibility?: UnifiedMemoryVisibility;
  provenance?: UnifiedMemoryProvenance;
  last_used_at_ms?: number | null;
  archived_at_ms?: number | null;
  forget_policy?: string;
  recall_policy?: string;
}

export interface UnifiedMemoryAction {
  memory_id: string;
  action: UnifiedMemoryActionKind;
  patch?: UnifiedMemoryActionPatch | null;
}

export interface UnifiedMemoryActionPatch {
  body?: string | null;
  tags?: string[] | null;
}

export interface UnifiedMemoryActionResult {
  memory_id: string;
  source: UnifiedMemorySource;
  source_id: string;
  action: UnifiedMemoryActionKind;
  changed: boolean;
  resulting_status?: UnifiedMemoryStatus | null;
  record?: UnifiedMemoryRecord | null;
  evidence: string[];
}

export type UnifiedMemoryActionErrorKind =
  | "invalid_id"
  | "unknown_source"
  | "not_found"
  | "unsupported_action"
  | "invalid_patch"
  | "store_error";

export interface UnifiedMemoryActionError {
  kind: UnifiedMemoryActionErrorKind;
  memory_id: string;
  source?: UnifiedMemorySource | string | null;
  source_id?: string | null;
  action: UnifiedMemoryActionKind;
  message: string;
}

export interface UnifiedMemorySelectionAudit {
  memory_id: string;
  source: UnifiedMemorySource;
  source_id: string;
  kind: UnifiedMemoryKind;
  score: number;
  reason: string;
  project_match: boolean;
  profile_match: boolean;
  injected: boolean;
}

export interface McpContextResource {
  server_id: string;
  uri: string;
  name: string;
  description: string;
  mime_type: string | null;
}

export interface McpContextPromptArgument {
  name: string;
  description: string;
  required: boolean;
}

export interface McpContextPrompt {
  server_id: string;
  name: string;
  description: string;
  arguments: McpContextPromptArgument[];
}

export interface McpContextSources {
  resources: McpContextResource[];
  prompts: McpContextPrompt[];
}

export type McpContextSelection =
  | {
      kind: "resource";
      server_id: string;
      uri: string;
      name: string;
      description?: string;
      mime_type?: string | null;
    }
  | {
      kind: "prompt";
      server_id: string;
      name: string;
      description?: string;
      arguments?: Record<string, string>;
    };

export type ComposerCapabilitySelection =
  | { kind: "slash_command"; command: string }
  | { kind: "file_reference"; path: string };

export interface ManualCompactResult {
  compacted: boolean;
  skipped_reason?: string | null;
  retained_messages: number;
  compacted_messages: number;
  estimated_tokens_before: number;
  estimated_tokens_after: number;
}

export type AgentA2AStateSource = "live" | "ledger";

export interface AgentA2ASessionState {
  session_id: string;
  source: AgentA2AStateSource;
  state: AgentA2AProjection;
}

export interface AgentA2ALedgerLoadError {
  session_id: string;
  message: string;
}

export interface AgentA2AStatesPayload {
  states: AgentA2ASessionState[];
  load_errors: AgentA2ALedgerLoadError[];
}

export interface PluginEntry {
  id: string;
  name: string;
  description: string;
  plugin_type: string;
  agent: string;
  category: string;
  status: unknown;
  config_schema?: unknown;
  current_config?: unknown;
  homepage?: string;
  author?: string;
}

export interface CapabilityInfo {
  id: string;
  name: string;
  description: string;
  kind: string;
  source: string;
  version: string;
  enabled: boolean;
}

export type EcosystemItemStatus = "healthy" | "unavailable" | "warning" | "unknown";

export interface EcosystemItem {
  id: string;
  name: string;
  description: string;
  kind: string; // "skill" | "hook" | "mcp_server" | "provider" | "tool"
  source: string;
  version: string;
  enabled: boolean;
  status: EcosystemItemStatus;
  statusMessage?: string | null;
  configurable: boolean;
  configSummary?: string | null;
}

export interface McpEcosystemItemConfig {
  name?: string | null;
  description?: string | null;
  command?: string | null;
  args?: string[] | null;
  enabled?: boolean | null;
}

export interface ToolInventoryEntry {
  id: string;
  name: string;
  description: string;
  kind: string;
  source: string;
  enabled: boolean;
}

export interface MemoryFact {
  id: string;
  text: string;
  tags: string[];
  profile_id?: string | null;
  source?: string | null;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface UpsertMemoryFactInput {
  id?: string | null;
  text: string;
  tags?: string[];
  profile_id?: string | null;
  source?: string | null;
}

export interface UpsertMemoryFactOutput {
  fact: MemoryFact;
  was_update: boolean;
}

export interface ForgeProfile {
  id: string;
  name: string;
  default_provider?: string | null;
  default_model?: string | null;
  default_workspace?: string | null;
  credential_overrides?: Record<string, { service: string; account: string }>;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface UpsertProfileInput {
  id?: string | null;
  name: string;
  default_provider?: string | null;
  default_model?: string | null;
  default_workspace?: string | null;
}

export interface ProfileListPayload {
  profiles: ForgeProfile[];
  active_profile_id: string | null;
}

export type CheckStatus = "pass" | "warn" | "fail";

export interface DiagnosticCheck {
  id: string;
  label: string;
  status: CheckStatus;
  message: string;
  detail?: unknown | null;
  remediation?: string | null;
  repairActionId?: string | null;
}

export interface DiagnosticsReport {
  ok: boolean;
  generatedAtMs: number;
  checks: DiagnosticCheck[];
}

export interface GatewayTriggerRunRecord {
  id: string;
  trigger_id: string;
  session_id?: string | null;
  attempt: number;
  status: string;
  message: string;
  started_at_ms: number;
  ended_at_ms: number;
  executor_kind?: string | null;
  failure_category?: string | null;
  lease_expires_at_ms?: number | null;
  trigger_message?: string | null;
  profile_id?: string | null;
  provider?: string | null;
  model?: string | null;
  workspace_path?: string | null;
}

export interface GatewayRuntimeStatus {
  ok: boolean;
  message: string;
  ownership?: GatewayOwnershipCapability;
  degraded_mode?: GatewayDegradedModeStatus;
  runtime_health?: RuntimeHealthSnapshot;
  uptime_seconds: number;
  active_sessions: number;
  pending_triggers: number;
  pending_session_inputs: number;
  loop_runner?: string;
  pending_loop_tasks?: number;
  running_loop_tasks?: number;
  stale_loop_task_leases?: number;
  orphaned_loop_tasks?: number;
  interrupted_loop_tasks?: number;
  recoverable_loop_tasks?: number;
  dry_run_headless_owner_runs?: number;
  waiting_headless_owner_runs?: number;
  denied_headless_owner_runs?: number;
  expired_headless_owner_runs?: number;
  claimed_triggers: number;
  dead_letter_runs: number;
  recent_runs: GatewayTriggerRunRecord[];
  recent_session_inputs: GatewaySessionInputCompletionRecord[];
  runtime_tasks: GatewayRuntimeTaskStatus[];
}

export interface RuntimeHealthSnapshot {
  ok: boolean;
  generated_at_ms: number;
  active_runs: RuntimeActiveRunHealth;
  pending_confirmations: RuntimePendingConfirmationHealth;
  loop_tasks: RuntimeLoopTaskHealth;
  gateway_queue: RuntimeGatewayQueueHealth;
  scheduler_queue: RuntimeSchedulerQueueHealth;
  runtime_tasks: RuntimeTaskHealth;
  last_replay: RuntimeReplayHealth;
  last_recovery_action?: RuntimeRecoveryActionSnapshot | null;
}

export interface RuntimeActiveRunHealth {
  active_sessions: number;
  running_loop_tasks: number;
}

export interface RuntimePendingConfirmationHealth {
  count: number;
  available: boolean;
  source: string;
}

export interface RuntimeLoopTaskHealth {
  total: number;
  pending: number;
  running: number;
  waiting_for_input: number;
  waiting_for_review: number;
  completed: number;
  failed: number;
  canceled: number;
  interrupted: number;
  stale_leases: number;
  orphaned: number;
  recoverable: number;
}

export interface RuntimeGatewayQueueHealth {
  pending_triggers: number;
  claimed_triggers: number;
  pending_session_inputs: number;
  dead_letter_runs: number;
}

export interface RuntimeSchedulerQueueHealth {
  running: boolean;
  pending_tasks: number;
  source: string;
}

export interface RuntimeTaskHealth {
  total: number;
  running: number;
  failed: number;
  webhook_listener_running: boolean;
  trigger_runner_running: boolean;
  loop_runner_running: boolean;
  scheduler_tick_running: boolean;
  dashboard_http_running: boolean;
}

export interface RuntimeReplayHealth {
  ok: boolean;
  task_count: number;
  message: string;
}

export interface RuntimeRecoveryActionSnapshot {
  task_id: string;
  kind: "orphaned" | "interrupted" | string;
  reason: string;
  notice: string;
  recorded_at_ms: number;
}

export type GatewayOwnershipMode =
  | "local_default"
  | "gateway_opt_in"
  | "gateway_opt_in_dry_run"
  | "gateway_read_only_owner"
  | "gateway_patch_proposal_owner"
  | "gateway_tool_owner_blocked_by_default";

export interface GatewayOwnershipCapability {
  ownership_mode: GatewayOwnershipMode;
  gateway_default_enabled: boolean;
  gateway_can_own_sessions: boolean;
  requires_opt_in: boolean;
  parity_gate: string;
  recovery_gate: string;
  required_action: string;
}

export type GatewayOwnershipEligibilityDecision =
  | "allow"
  | "deny"
  | "requires_human_approval";

export interface GatewayOwnershipEligibilityResult {
  ok: boolean;
  decision: GatewayOwnershipEligibilityDecision;
  requested_mode: GatewayOwnershipMode;
  session_id?: string | null;
  task_id?: string | null;
  reasons: string[];
  missing_evidence: string[];
  required_action: string;
  proposal_only?: boolean;
  would_generate_patch_proposal?: boolean;
  would_apply_patch?: boolean;
  would_execute_provider: boolean;
  would_execute_tools: boolean;
  would_write_files: boolean;
  changes_task_state: boolean;
}

export interface GatewayReadOnlyOwnerSideEffects {
  provider: boolean;
  tools: boolean;
  shell: boolean;
  write_files: boolean;
  confirmations: boolean;
  commits: boolean;
}

export interface GatewayReadOnlyOwnerDiagnosticsResult {
  ok: boolean;
  started: boolean;
  completed: boolean;
  gateway_can_resume: boolean;
  task: LoopTaskRecord;
  owner_run?: HeadlessOwnerRun | null;
  summary: string;
  message: string;
  side_effects: GatewayReadOnlyOwnerSideEffects;
}

export interface GatewayDegradedModeStatus {
  active: boolean;
  reason: string;
  fallback: string;
  input_policy: string;
  confirmation_policy: string;
  recovery_command?: string;
}

export interface GatewaySessionInputCompletionRecord {
  input_id: string;
  session_id: string;
  message_preview: string;
  received_at_ms: number;
  completed_at_ms: number;
  action?: "accepted" | "cleared_stale";
  reason?: string | null;
}

export interface GatewayRuntimeTaskStatus {
  name: string;
  running: boolean;
  last_started_at_ms?: number | null;
  last_error?: string | null;
}

export type GatewaySessionAttachStatus = "live" | "restored" | "stale" | "missing";

export type GatewaySessionControlPlane =
  | "desktop_runtime_required"
  | "desktop_restore_required"
  | "unavailable";

export interface GatewaySessionControl {
  control_plane: GatewaySessionControlPlane;
  ownership_mode?: GatewayOwnershipMode;
  gateway_can_own_session?: boolean;
  gateway_can_stream: boolean;
  gateway_can_send_input: boolean;
  gateway_can_resume: boolean;
  gateway_can_read_snapshot: boolean;
  required_action: string;
}

export interface GatewaySessionSnapshotSummary {
  session_id: string;
  provider: string;
  model: string;
  working_dir: string;
  summary?: string | null;
  created_at_ms: number;
  updated_at_ms: number;
  message_count: number;
}

export interface GatewaySessionInfo {
  session_id: string;
  provider: string;
  model: string;
  workspace_path: string;
  created_at_ms: number;
  owner_pid?: number | null;
  last_seen_at_ms?: number | null;
  restored_from_registry: boolean;
}

export interface AttachGatewaySessionResult {
  ok: boolean;
  session_id: string;
  status: GatewaySessionAttachStatus;
  message: string;
  control: GatewaySessionControl;
  snapshot?: GatewaySessionSnapshotSummary | null;
  session?: GatewaySessionInfo | null;
}

export interface GatewayPendingTrigger {
  id: string;
  message: string;
  profile_id?: string | null;
  provider?: string | null;
  model?: string | null;
  workspace_path?: string | null;
  attempt_count: number;
  claimed_at_ms?: number | null;
  received_at_ms: number;
}

export interface EnqueueGatewayTriggerInput {
  message: string;
  trigger_id?: string | null;
  profile_id?: string | null;
  provider?: string | null;
  model?: string | null;
  workspace_path?: string | null;
}

export interface EnqueueGatewayTriggerResult {
  ok: boolean;
  trigger_id: string;
  pending_triggers: number;
}

export interface EnqueueGatewaySessionInputResult {
  ok: boolean;
  input_id: string;
  session_id: string;
  pending_inputs: number;
}

export interface CancelGatewayTriggerResult {
  ok: boolean;
  trigger_id: string;
  removed: boolean;
  pending_triggers: number;
}

export interface ReplayGatewayTriggerRunResult {
  ok: boolean;
  run_id: string;
  trigger_id: string;
  pending_triggers: number;
}

export interface GetGatewayTriggerRunResult {
  ok: boolean;
  run: GatewayTriggerRunRecord;
}

export interface GetGatewaySessionSnapshotResult {
  ok: boolean;
  session_id: string;
  snapshot: Record<string, unknown>;
}

export interface TailGatewaySessionEventsResult {
  ok: boolean;
  session_id: string;
  events: StreamEvent[];
  next_cursor: number;
  total_events: number;
  cursor_reset: boolean;
}

export interface RepairAction {
  id: string;
  label: string;
  description: string;
}

export interface RepairVerification {
  label: string;
  ok: boolean;
  message: string;
}

export interface RepairResult {
  action_id: string;
  success: boolean;
  message: string;
  verification?: RepairVerification | null;
}

export interface ScheduledTask {
  id: string;
  title: string;
  text: string;
  enabled: boolean;
  interval_seconds: number;
  next_run_at_ms: number;
  last_run_at_ms?: number | null;
  created_at_ms: number;
  updated_at_ms: number;
  tags: string[];
  profile_id?: string | null;
  last_error?: string | null;
}

export interface RunHistoryEntry {
  id: string;
  task_id: string;
  started_at_ms: number;
  ended_at_ms: number;
  status: string; // "queued" | "completed" | "skipped" | "error"
  message: string;
}

export interface SchedulerListPayload {
  tasks: ScheduledTask[];
  recent_history: RunHistoryEntry[];
  load_error?: string | null;
}

export interface UpsertScheduledTaskInput {
  id?: string | null;
  title: string;
  text: string;
  tags?: string[];
  interval_seconds?: number;
  profile_id?: string | null;
}

export type ServiceStatus = {
  installed: boolean;
  running: boolean;
  message: string;
  supported: boolean;
  backend: string;
  service_id: string;
  label: string;
  launch_domain: string;
  service_path: string;
  plist_path: string;
  log_path: string;
  error_log_path: string;
  status_message: string;
};

export type LogEntry = {
  timestamp_ms: number;
  level: string;
  source: string;
  message: string;
  session_id?: string;
};
