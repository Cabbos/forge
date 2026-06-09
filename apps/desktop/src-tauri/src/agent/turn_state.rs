use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTurnStatus {
    Started,
    GatheringContext,
    CallingModel,
    RunningTools,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolCategory {
    Read,
    Write,
    Shell,
    Delegate,
    Mcp,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvidenceKind {
    Tool,
    Verification,
    Delivery,
    Preview,
    Checkpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentToolTrace {
    pub tool_call_id: String,
    pub name: String,
    pub category: AgentToolCategory,
    pub status: AgentToolStatus,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub result_summary: Option<String>,
    pub is_error: bool,
    pub affected_files: Vec<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentToolEvidence {
    #[serde(default = "default_evidence_kind")]
    pub kind: AgentEvidenceKind,
    pub evidence_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub category: AgentToolCategory,
    pub status: AgentToolStatus,
    pub outcome: String,
    pub summary: Option<String>,
    pub command: Option<String>,
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentCompactTrace {
    pub reason: String,
    pub retained_messages: usize,
    pub compacted_messages: usize,
    pub estimated_tokens_before: Option<u32>,
    pub estimated_tokens_after: Option<u32>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentVerificationStatus {
    NotNeeded,
    Skipped,
    Running,
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentVerificationTrace {
    pub status: AgentVerificationStatus,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_preview: Option<String>,
    pub stderr_preview: Option<String>,
    pub duration_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
}

impl Default for AgentVerificationTrace {
    fn default() -> Self {
        Self {
            status: AgentVerificationStatus::NotNeeded,
            command: None,
            exit_code: None,
            stdout_preview: None,
            stderr_preview: None,
            duration_ms: None,
            completed_at_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentFailureTrace {
    #[serde(default = "default_failure_kind")]
    pub kind: String,
    pub stage: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_advice: Option<AgentRecoveryAdvice>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentRecoveryAdvice {
    pub action: String,
    pub reason: String,
    pub instruction: String,
    pub safe_to_auto_retry: bool,
    pub requires_user_action: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTurnTransition {
    pub from_status: Option<AgentTurnStatus>,
    pub to_status: AgentTurnStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTurnContextSource {
    pub kind: String,
    pub label: String,
    pub reason: String,
    pub estimated_tokens: Option<u32>,
    pub injected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct AgentTurnContextSnapshot {
    pub sources: Vec<AgentTurnContextSource>,
    pub estimated_tokens: Option<u32>,
    pub budget_tokens: Option<u32>,
    pub omitted_sources: Vec<AgentTurnContextSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct AgentTurnInputIntent {
    #[serde(default)]
    pub slash_command: Option<String>,
    #[serde(default)]
    pub file_references: Vec<String>,
    #[serde(default)]
    pub selected_connectors: Vec<String>,
    #[serde(default)]
    pub matched_skills: Vec<String>,
    #[serde(default)]
    pub active_hooks: Vec<String>,
    #[serde(default)]
    pub enabled_mcp_servers: Vec<String>,
    #[serde(default)]
    pub available_mcp_tools: Vec<String>,
}

impl AgentTurnInputIntent {
    fn is_empty(&self) -> bool {
        self.slash_command.is_none()
            && self.file_references.is_empty()
            && self.selected_connectors.is_empty()
            && self.matched_skills.is_empty()
            && self.active_hooks.is_empty()
            && self.enabled_mcp_servers.is_empty()
            && self.available_mcp_tools.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPlanItemStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentPlanItem {
    pub id: String,
    pub title: String,
    pub status: AgentPlanItemStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentExecutionPlan {
    pub objective: String,
    pub items: Vec<AgentPlanItem>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTurnState {
    pub turn_id: String,
    pub session_id: String,
    pub workspace_path: String,
    pub provider: String,
    pub model: String,
    pub route: String,
    pub phase: String,
    pub user_goal: String,
    #[serde(default, skip_serializing_if = "AgentTurnInputIntent::is_empty")]
    pub input_intent: AgentTurnInputIntent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_plan: Option<AgentExecutionPlan>,
    pub context: AgentTurnContextSnapshot,
    pub tools: Vec<AgentToolTrace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<AgentToolEvidence>,
    pub compact_events: Vec<AgentCompactTrace>,
    pub verification: AgentVerificationTrace,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<AgentFailureTrace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transition_log: Vec<AgentTurnTransition>,
    pub status: AgentTurnStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub model_rounds: usize,
    #[serde(default)]
    pub tool_call_count: usize,
    #[serde(default)]
    pub failed_tool_count: usize,
    #[serde(default)]
    pub compact_saved_tokens: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentTurnProjection {
    pub session_id: String,
    pub status: AgentTurnStatus,
    pub step_label: String,
    pub workspace_path: String,
    pub compact_count: usize,
    pub verification_status: AgentVerificationStatus,
    pub model_rounds: usize,
    pub tool_call_count: usize,
    pub failed_tool_count: usize,
    pub estimated_context_tokens: Option<u32>,
    pub stop_reason: Option<String>,
    pub compact_saved_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnMetadata {
    pub session_id: String,
    pub workspace_path: String,
    pub provider: String,
    pub model: String,
    pub route: String,
    pub phase: String,
    pub user_goal: String,
    pub input_intent: AgentTurnInputIntent,
}

impl AgentTurnMetadata {
    pub fn default_for_session(
        session_id: String,
        workspace_path: String,
        provider: String,
        model: String,
        user_goal: String,
    ) -> Self {
        Self {
            session_id,
            workspace_path,
            provider,
            model,
            route: "direct".to_string(),
            phase: "idle".to_string(),
            user_goal,
            input_intent: AgentTurnInputIntent::default(),
        }
    }

    pub fn into_turn_state(self, turn_id: String) -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            turn_id,
            self.session_id,
            self.workspace_path,
            self.provider,
            self.model,
            self.route,
            self.phase,
            self.user_goal,
        );
        turn.input_intent = self.input_intent;
        turn
    }
}

impl AgentTurnState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        turn_id: String,
        session_id: String,
        workspace_path: String,
        provider: String,
        model: String,
        route: String,
        phase: String,
        user_goal: String,
    ) -> Self {
        let now = now_ms();
        Self {
            turn_id,
            session_id,
            workspace_path,
            provider,
            model,
            route,
            phase,
            user_goal,
            input_intent: AgentTurnInputIntent::default(),
            execution_plan: None,
            context: AgentTurnContextSnapshot::default(),
            tools: Vec::new(),
            evidence: Vec::new(),
            compact_events: Vec::new(),
            verification: AgentVerificationTrace::default(),
            failure: None,
            transition_log: vec![AgentTurnTransition {
                from_status: None,
                to_status: AgentTurnStatus::Started,
                reason: "turn_started".to_string(),
                detail: None,
                created_at_ms: now,
            }],
            status: AgentTurnStatus::Started,
            stop_reason: None,
            model_rounds: 0,
            tool_call_count: 0,
            failed_tool_count: 0,
            compact_saved_tokens: 0,
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    pub fn mark_status(&mut self, status: AgentTurnStatus) {
        self.mark_status_with_reason(status, "status_update", None);
    }

    pub fn mark_status_with_reason(
        &mut self,
        status: AgentTurnStatus,
        reason: impl Into<String>,
        detail: Option<&str>,
    ) {
        let from_status = self.status.clone();
        if status == AgentTurnStatus::Cancelled {
            self.mark_interrupted_tools_for_cancellation();
        }
        self.status = status.clone();
        self.update_execution_plan_for_status(&status, None);
        self.record_transition(
            Some(from_status),
            status,
            reason,
            detail.map(ToString::to_string),
        );
        self.touch();
    }

    pub fn set_stop_reason(&mut self, reason: impl Into<String>) {
        self.stop_reason = Some(reason.into());
        self.touch();
    }

    pub fn record_tool(&mut self, trace: AgentToolTrace) {
        let reason = tool_transition_reason(&trace);
        let detail = tool_transition_detail(&trace);
        let tool_call_id = trace.tool_call_id.clone();
        let replaced_existing = if let Some(existing) = self.tools.iter_mut().find(|item| {
            item.tool_call_id == tool_call_id
                && item.name == trace.name
                && matches!(
                    item.status,
                    AgentToolStatus::Pending | AgentToolStatus::Running
                )
        }) {
            *existing = trace.clone();
            true
        } else {
            self.tools.push(trace.clone());
            false
        };
        if replaced_existing {
            self.evidence.retain(|item| {
                !(item.kind == AgentEvidenceKind::Tool
                    && item.tool_call_id == tool_call_id
                    && item.tool_name == trace.name)
            });
        }
        if tool_trace_is_terminal(&trace) {
            let evidence = AgentToolEvidence::from_trace(&trace);
            let evidence_id = evidence.evidence_id.clone();
            self.evidence.push(evidence);
            self.attach_evidence_to_active_plan_item(evidence_id);
        }
        self.record_transition(
            Some(self.status.clone()),
            self.status.clone(),
            reason,
            detail,
        );
        self.touch();
    }

    fn mark_interrupted_tools_for_cancellation(&mut self) {
        let now = now_ms();
        let mut interrupted = Vec::new();
        for tool in &mut self.tools {
            if matches!(
                tool.status,
                AgentToolStatus::Pending | AgentToolStatus::Running
            ) {
                tool.status = AgentToolStatus::Cancelled;
                tool.ended_at_ms = Some(now);
                tool.is_error = true;
                tool.result_summary = Some(
                    "Tool interrupted while the session was not running; re-check current workspace state before continuing."
                        .to_string(),
                );
                interrupted.push(tool.clone());
            }
        }

        for trace in interrupted {
            self.evidence.retain(|item| {
                !(item.kind == AgentEvidenceKind::Tool && item.tool_call_id == trace.tool_call_id)
            });
            let evidence = AgentToolEvidence::from_trace(&trace);
            let evidence_id = evidence.evidence_id.clone();
            self.evidence.push(evidence);
            self.attach_evidence_to_active_plan_item(evidence_id);
        }
    }

    pub fn normalize_for_session_resume(&mut self) {
        if !matches!(
            self.status,
            AgentTurnStatus::Started
                | AgentTurnStatus::GatheringContext
                | AgentTurnStatus::CallingModel
                | AgentTurnStatus::RunningTools
                | AgentTurnStatus::Verifying
        ) {
            return;
        }

        self.mark_status_with_reason(
            AgentTurnStatus::Cancelled,
            "session_restored_interrupted_turn",
            Some("session was restored while the previous turn was still in progress"),
        );
    }

    pub fn record_compact(&mut self, trace: AgentCompactTrace) {
        let reason = trace.reason.clone();
        let detail = format!(
            "retained_messages={}, compacted_messages={}",
            trace.retained_messages, trace.compacted_messages
        );
        self.compact_events.push(trace);
        self.record_transition(
            Some(self.status.clone()),
            self.status.clone(),
            reason,
            Some(detail),
        );
        self.touch();
    }

    pub fn set_verification(&mut self, trace: AgentVerificationTrace) {
        let transition_reason = verification_transition_reason(&trace.status);
        let transition_detail = verification_transition_detail(&trace);
        let evidence = AgentToolEvidence::from_verification_trace(&trace);
        self.verification = trace;
        if let Some(evidence) = evidence {
            let evidence_id = evidence.evidence_id.clone();
            self.evidence.push(evidence);
            self.attach_evidence_to_active_plan_item(evidence_id);
        }
        if let Some(reason) = transition_reason {
            self.record_transition(
                Some(self.status.clone()),
                self.status.clone(),
                reason,
                transition_detail,
            );
        }
        self.touch();
    }

    pub fn record_delivery_summary(&mut self, summary: &crate::protocol::events::DeliverySummary) {
        let evidence = AgentToolEvidence::from_delivery_summary(summary);
        let evidence_id = evidence.evidence_id.clone();
        self.evidence.push(evidence);
        self.attach_evidence_to_active_plan_item(evidence_id);
        self.record_transition(
            Some(self.status.clone()),
            self.status.clone(),
            "delivery_evidence",
            Some(delivery_evidence_summary(summary)),
        );
        self.touch();
    }

    pub fn record_preview_status(
        &mut self,
        project_path: Option<&str>,
        running: bool,
        can_start: bool,
        can_open: bool,
        label: &str,
        url: Option<&str>,
    ) {
        let evidence = AgentToolEvidence::from_preview_status(
            project_path,
            running,
            can_start,
            can_open,
            label,
            url,
        );
        let evidence_id = evidence.evidence_id.clone();
        self.evidence.push(evidence);
        self.attach_evidence_to_active_plan_item(evidence_id);
        self.record_transition(
            Some(self.status.clone()),
            self.status.clone(),
            "preview_evidence",
            Some(format!(
                "label={label}; running={running}; can_start={can_start}; can_open={can_open}"
            )),
        );
        self.touch();
    }

    pub fn record_checkpoint_status(
        &mut self,
        is_git_repo: bool,
        dirty: bool,
        has_checkpoint: bool,
        label: &str,
    ) {
        let evidence =
            AgentToolEvidence::from_checkpoint_status(is_git_repo, dirty, has_checkpoint, label);
        let evidence_id = evidence.evidence_id.clone();
        self.evidence.push(evidence);
        self.attach_evidence_to_active_plan_item(evidence_id);
        self.record_transition(
            Some(self.status.clone()),
            self.status.clone(),
            "checkpoint_evidence",
            Some(format!(
                "label={label}; is_git_repo={is_git_repo}; dirty={dirty}; has_checkpoint={has_checkpoint}"
            )),
        );
        self.touch();
    }

    pub fn record_failure(&mut self, trace: AgentFailureTrace) {
        let from_status = self.status.clone();
        if from_status == AgentTurnStatus::Cancelled {
            self.record_transition(
                Some(from_status),
                AgentTurnStatus::Cancelled,
                "late_failure_ignored_after_cancel",
                Some(format!("kind={}, stage={}", trace.kind, trace.stage)),
            );
            self.touch();
            return;
        }
        let detail = format!("kind={}, stage={}", trace.kind, trace.stage);
        let failure_kind = trace.kind.clone();
        self.failure = Some(trace);
        self.status = AgentTurnStatus::Failed;
        self.update_execution_plan_for_status(&AgentTurnStatus::Failed, Some(failure_kind));
        self.record_transition(
            Some(from_status),
            AgentTurnStatus::Failed,
            "failure",
            Some(detail),
        );
        self.touch();
    }

    pub fn set_execution_plan(&mut self, objective: String, item_titles: Vec<String>) {
        let items = item_titles
            .into_iter()
            .enumerate()
            .map(|(index, title)| AgentPlanItem {
                id: format!("step-{}", index + 1),
                title,
                status: AgentPlanItemStatus::Pending,
                evidence_ids: Vec::new(),
                failure_kind: None,
            })
            .collect::<Vec<_>>();
        self.execution_plan = Some(AgentExecutionPlan {
            objective,
            items,
            updated_at_ms: now_ms(),
        });
        self.touch();
    }

    pub fn update_execution_plan_item(
        &mut self,
        item_id: &str,
        status: AgentPlanItemStatus,
        evidence_id: Option<String>,
        failure_kind: Option<String>,
    ) -> bool {
        let Some(plan) = self.execution_plan.as_mut() else {
            return false;
        };
        let Some(item) = plan.items.iter_mut().find(|item| item.id == item_id) else {
            return false;
        };
        item.status = status;
        if let Some(evidence_id) = evidence_id {
            if !item
                .evidence_ids
                .iter()
                .any(|existing| existing == &evidence_id)
            {
                item.evidence_ids.push(evidence_id);
            }
        }
        if let Some(failure_kind) = failure_kind.filter(|kind| !kind.trim().is_empty()) {
            item.failure_kind = Some(failure_kind);
        }
        plan.updated_at_ms = now_ms();
        self.touch();
        true
    }

    pub fn set_context(&mut self, context: AgentTurnContextSnapshot) {
        self.context = context;
        self.touch();
    }

    pub fn to_projection(&self) -> AgentTurnProjection {
        AgentTurnProjection {
            session_id: self.session_id.clone(),
            status: self.status.clone(),
            step_label: self.step_label().to_string(),
            workspace_path: self.workspace_path.clone(),
            compact_count: self.compact_events.len(),
            verification_status: self.verification.status.clone(),
            model_rounds: self.model_rounds,
            tool_call_count: self.tool_call_count,
            failed_tool_count: self.failed_tool_count,
            estimated_context_tokens: self.context.estimated_tokens,
            stop_reason: self.stop_reason.clone(),
            compact_saved_tokens: self.compact_saved_tokens,
        }
    }

    fn step_label(&self) -> &'static str {
        match self.status {
            AgentTurnStatus::Started => "准备处理",
            AgentTurnStatus::GatheringContext => "整理上下文",
            AgentTurnStatus::CallingModel => "请求模型",
            AgentTurnStatus::RunningTools => "处理项目",
            AgentTurnStatus::Verifying => "检查结果",
            AgentTurnStatus::Completed => "已完成",
            AgentTurnStatus::Failed => "遇到问题",
            AgentTurnStatus::Cancelled => "已取消",
        }
    }

    fn touch(&mut self) {
        self.updated_at_ms = now_ms();
    }

    fn record_transition(
        &mut self,
        from_status: Option<AgentTurnStatus>,
        to_status: AgentTurnStatus,
        reason: impl Into<String>,
        detail: Option<String>,
    ) {
        self.transition_log.push(AgentTurnTransition {
            from_status,
            to_status,
            reason: reason.into(),
            detail,
            created_at_ms: now_ms(),
        });
    }

    fn attach_evidence_to_active_plan_item(&mut self, evidence_id: String) {
        let Some(plan) = self.execution_plan.as_mut() else {
            return;
        };
        let index = plan
            .items
            .iter()
            .position(|item| item.status == AgentPlanItemStatus::InProgress)
            .or_else(|| {
                plan.items
                    .iter()
                    .position(|item| item.status == AgentPlanItemStatus::Pending)
            });
        let Some(index) = index else {
            return;
        };
        let item = &mut plan.items[index];
        if !item
            .evidence_ids
            .iter()
            .any(|existing| existing == &evidence_id)
        {
            item.evidence_ids.push(evidence_id);
            plan.updated_at_ms = now_ms();
        }
    }

    fn update_execution_plan_for_status(
        &mut self,
        status: &AgentTurnStatus,
        failure_kind: Option<String>,
    ) {
        let Some(plan) = self.execution_plan.as_mut() else {
            return;
        };
        match status {
            AgentTurnStatus::Started
            | AgentTurnStatus::GatheringContext
            | AgentTurnStatus::CallingModel => {
                set_plan_item_status(plan, 0, AgentPlanItemStatus::InProgress, None);
            }
            AgentTurnStatus::RunningTools => {
                set_plan_item_status(plan, 0, AgentPlanItemStatus::Completed, None);
                set_plan_item_status(plan, 1, AgentPlanItemStatus::InProgress, None);
            }
            AgentTurnStatus::Verifying => {
                set_plan_item_status(plan, 0, AgentPlanItemStatus::Completed, None);
                set_plan_item_status(plan, 1, AgentPlanItemStatus::Completed, None);
                set_plan_item_status(plan, 2, AgentPlanItemStatus::InProgress, None);
            }
            AgentTurnStatus::Completed => {
                for item in &mut plan.items {
                    if matches!(
                        item.status,
                        AgentPlanItemStatus::Pending | AgentPlanItemStatus::InProgress
                    ) {
                        item.status = AgentPlanItemStatus::Completed;
                    }
                }
            }
            AgentTurnStatus::Failed => {
                let index = plan
                    .items
                    .iter()
                    .position(|item| item.status == AgentPlanItemStatus::InProgress)
                    .or_else(|| {
                        plan.items
                            .iter()
                            .position(|item| item.status == AgentPlanItemStatus::Pending)
                    });
                if let Some(index) = index {
                    set_plan_item_status(plan, index, AgentPlanItemStatus::Failed, failure_kind);
                }
            }
            AgentTurnStatus::Cancelled => {
                if let Some(index) = plan
                    .items
                    .iter()
                    .position(|item| item.status == AgentPlanItemStatus::InProgress)
                {
                    set_plan_item_status(plan, index, AgentPlanItemStatus::Skipped, None);
                }
            }
        }
        plan.updated_at_ms = now_ms();
    }
}

fn set_plan_item_status(
    plan: &mut AgentExecutionPlan,
    index: usize,
    status: AgentPlanItemStatus,
    failure_kind: Option<String>,
) {
    let Some(item) = plan.items.get_mut(index) else {
        return;
    };
    if item.status == AgentPlanItemStatus::Failed && status != AgentPlanItemStatus::Failed {
        return;
    }
    item.status = status;
    if let Some(kind) = failure_kind.filter(|kind| !kind.trim().is_empty()) {
        item.failure_kind = Some(kind);
    }
}

impl AgentToolEvidence {
    fn from_trace(trace: &AgentToolTrace) -> Self {
        let failed = trace.is_error
            || matches!(
                trace.status,
                AgentToolStatus::Failed | AgentToolStatus::Cancelled
            );
        Self {
            kind: AgentEvidenceKind::Tool,
            evidence_id: format!("tool:{}", trace.tool_call_id),
            tool_call_id: trace.tool_call_id.clone(),
            tool_name: trace.name.clone(),
            category: trace.category.clone(),
            status: trace.status.clone(),
            outcome: if failed { "failed" } else { "succeeded" }.to_string(),
            summary: trace.result_summary.clone(),
            command: trace.command.clone(),
            affected_files: trace.affected_files.clone(),
            failure_kind: failed.then(|| classify_tool_failure_kind(trace).to_string()),
            created_at_ms: trace.ended_at_ms.unwrap_or(trace.started_at_ms),
        }
    }

    fn from_verification_trace(trace: &AgentVerificationTrace) -> Option<Self> {
        let (status, outcome, failure_kind) = match trace.status {
            AgentVerificationStatus::Passed => (AgentToolStatus::Completed, "succeeded", None),
            AgentVerificationStatus::Skipped => (AgentToolStatus::Completed, "skipped", None),
            AgentVerificationStatus::Failed | AgentVerificationStatus::Error => (
                AgentToolStatus::Failed,
                "failed",
                Some(classify_verification_failure_kind(trace)),
            ),
            AgentVerificationStatus::NotNeeded | AgentVerificationStatus::Running => return None,
        };
        let created_at_ms = trace.completed_at_ms.unwrap_or_else(now_ms);
        Some(Self {
            kind: AgentEvidenceKind::Verification,
            evidence_id: format!("verification:{created_at_ms}"),
            tool_call_id: String::new(),
            tool_name: "verification".to_string(),
            category: AgentToolCategory::Shell,
            status,
            outcome: outcome.to_string(),
            summary: verification_evidence_summary(trace),
            command: trace.command.clone(),
            affected_files: Vec::new(),
            failure_kind: failure_kind.map(ToString::to_string),
            created_at_ms,
        })
    }

    fn from_delivery_summary(summary: &crate::protocol::events::DeliverySummary) -> Self {
        let failed_verification = matches!(
            summary.verification_status.as_deref(),
            Some("failed" | "error")
        );
        let needs_action = !summary.next_action.contains("交付状态可以继续验收");
        Self {
            kind: AgentEvidenceKind::Delivery,
            evidence_id: format!("delivery:{}", now_ms()),
            tool_call_id: String::new(),
            tool_name: "delivery_status".to_string(),
            category: AgentToolCategory::Other,
            status: AgentToolStatus::Completed,
            outcome: if needs_action {
                "needs_action"
            } else {
                "succeeded"
            }
            .to_string(),
            summary: Some(delivery_evidence_summary(summary)),
            command: summary.verification_command.clone(),
            affected_files: Vec::new(),
            failure_kind: failed_verification.then(|| "verification".to_string()),
            created_at_ms: now_ms(),
        }
    }

    fn from_preview_status(
        project_path: Option<&str>,
        running: bool,
        can_start: bool,
        can_open: bool,
        label: &str,
        url: Option<&str>,
    ) -> Self {
        let failed = preview_status_is_conflict(label, running, can_start, can_open);
        Self {
            kind: AgentEvidenceKind::Preview,
            evidence_id: format!("preview:{}", now_ms()),
            tool_call_id: String::new(),
            tool_name: "preview_status".to_string(),
            category: AgentToolCategory::Other,
            status: AgentToolStatus::Completed,
            outcome: if running {
                "succeeded"
            } else if failed {
                "failed"
            } else {
                "needs_action"
            }
            .to_string(),
            summary: Some(preview_evidence_summary(
                project_path,
                running,
                can_start,
                can_open,
                label,
                url,
            )),
            command: None,
            affected_files: Vec::new(),
            failure_kind: failed.then(|| "preview_conflict".to_string()),
            created_at_ms: now_ms(),
        }
    }

    fn from_checkpoint_status(
        is_git_repo: bool,
        dirty: bool,
        has_checkpoint: bool,
        label: &str,
    ) -> Self {
        Self {
            kind: AgentEvidenceKind::Checkpoint,
            evidence_id: format!("checkpoint:{}", now_ms()),
            tool_call_id: String::new(),
            tool_name: "checkpoint_status".to_string(),
            category: AgentToolCategory::Other,
            status: AgentToolStatus::Completed,
            outcome: if has_checkpoint {
                "succeeded"
            } else {
                "needs_action"
            }
            .to_string(),
            summary: Some(
                format!(
                    "{label}; is_git_repo={is_git_repo}; dirty={dirty}; has_checkpoint={has_checkpoint}"
                )
                .chars()
                .take(240)
                .collect(),
            ),
            command: None,
            affected_files: Vec::new(),
            failure_kind: None,
            created_at_ms: now_ms(),
        }
    }
}

pub fn classify_tool_category(name: &str) -> AgentToolCategory {
    match name {
        "read_file" | "read" | "list_directory" | "ls" | "list" | "search_files" | "glob"
        | "search_content" | "grep" | "web_search" | "web_fetch" | "git_diff" => {
            AgentToolCategory::Read
        }
        "write_file" | "write_to_file" | "write" | "edit_file" | "edit" | "apply_patch"
        | "create_file" | "delete_file" | "move_file" => AgentToolCategory::Write,
        "run_shell" | "bash" | "shell" | "shell_command" | "run_command" | "run_shell_command"
        | "exec" | "execute_command" => AgentToolCategory::Shell,
        "delegate_task" => AgentToolCategory::Delegate,
        name if name.starts_with("mcp__") => AgentToolCategory::Mcp,
        _ => AgentToolCategory::Other,
    }
}

pub fn completed_tool_trace(
    tool_call_id: String,
    name: String,
    input: &serde_json::Value,
    result: &str,
    started_at_ms: u64,
    ended_at_ms: u64,
) -> AgentToolTrace {
    let category = classify_tool_category(&name);
    let is_error = if category == AgentToolCategory::Shell {
        shell_exit_code(result)
            .map(|code| code != 0)
            .unwrap_or_else(|| is_errorish_tool_result(result))
    } else {
        is_errorish_tool_result(result)
    };
    AgentToolTrace {
        tool_call_id,
        category,
        name,
        status: if is_error {
            AgentToolStatus::Failed
        } else {
            AgentToolStatus::Completed
        },
        started_at_ms,
        ended_at_ms: Some(ended_at_ms),
        result_summary: summarize_tool_result(result),
        is_error,
        affected_files: extract_affected_files(input),
        command: extract_command(input),
    }
}

pub fn running_tool_trace(
    tool_call_id: String,
    name: String,
    input: &serde_json::Value,
    started_at_ms: u64,
) -> AgentToolTrace {
    AgentToolTrace {
        tool_call_id,
        category: classify_tool_category(&name),
        name,
        status: AgentToolStatus::Running,
        started_at_ms,
        ended_at_ms: None,
        result_summary: None,
        is_error: false,
        affected_files: extract_affected_files(input),
        command: extract_command(input),
    }
}

fn tool_trace_is_terminal(trace: &AgentToolTrace) -> bool {
    matches!(
        trace.status,
        AgentToolStatus::Completed | AgentToolStatus::Failed | AgentToolStatus::Cancelled
    ) || trace.is_error
}

fn tool_transition_reason(trace: &AgentToolTrace) -> &'static str {
    match trace.status {
        AgentToolStatus::Pending | AgentToolStatus::Running => "tool_started",
        AgentToolStatus::Cancelled => "tool_cancelled",
        AgentToolStatus::Failed => "tool_failed",
        AgentToolStatus::Completed if trace.is_error => "tool_failed",
        AgentToolStatus::Completed => "tool_completed",
    }
}

fn shell_exit_code(result: &str) -> Option<i32> {
    result.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Exit code:")
            .and_then(|code| code.trim().parse::<i32>().ok())
    })
}

pub(crate) fn is_errorish_tool_result(result: &str) -> bool {
    result.starts_with("Error:")
        || result.starts_with("Denied:")
        || result.starts_with("Search blocked:")
        || result.starts_with("Search failed:")
        || result.starts_with("Search timed out")
        || result.starts_with("Permission denied")
        || result.starts_with("Tool disabled")
        || result.starts_with("Tool execution blocked")
        || result.starts_with("Tool result missing:")
        || result.starts_with("Unknown MCP tool:")
        || result.to_ascii_lowercase().contains("file not found")
}

fn summarize_tool_result(result: &str) -> Option<String> {
    let summary = result.split_whitespace().collect::<Vec<_>>().join(" ");
    if summary.is_empty() {
        None
    } else {
        Some(summary.chars().take(240).collect())
    }
}

fn tool_transition_detail(trace: &AgentToolTrace) -> Option<String> {
    let mut parts = vec![format!("tool={}", trace.name)];
    if let Some(command) = trace.command.as_deref().filter(|value| !value.is_empty()) {
        parts.push(format!("command={command}"));
    }
    if !trace.affected_files.is_empty() {
        parts.push(format!("files={}", trace.affected_files.join(",")));
    }
    if let Some(summary) = trace
        .result_summary
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("summary={summary}"));
    }

    let detail = parts.join("; ");
    if detail.is_empty() {
        None
    } else {
        Some(detail.chars().take(320).collect())
    }
}

static TOOL_FAILURE_PATTERNS: &[(&[&str], &str)] = &[
    (
        &[
            "address already in use",
            "port already in use",
            "port conflict",
        ],
        "preview_conflict",
    ),
    (
        &[
            "eresolve",
            "dependency tree",
            "npm install failed",
            "failed to install dependencies",
            "could not resolve dependency",
        ],
        "dependency_install_failed",
    ),
    (
        &[
            "command not found: pnpm",
            "pnpm: command not found",
            "no such file or directory: pnpm",
            "command not found: yarn",
            "yarn: command not found",
            "command not found: bun",
            "bun: command not found",
        ],
        "package_manager_missing",
    ),
    (
        &["command not found", "not recognized as an internal"],
        "command_not_found",
    ),
    (
        &[
            "patch does not apply",
            "git apply failed",
            "checkpoint failed",
            "检查点失败",
            "无法读取检查点",
        ],
        "checkpoint_failed",
    ),
    (
        &["file not found", "no such file or directory", "找不到文件"],
        "file_not_found",
    ),
    (
        &[
            "tool result missing",
            "tool_use id",
            "matching tool_result",
            "tool call mismatch",
        ],
        "tool_call_mismatch",
    ),
    (
        &[
            "outside workspace",
            "workspace boundary",
            "search blocked",
            "write boundary",
        ],
        "workspace_boundary",
    ),
    (
        &["permission denied", "access denied", "denied:"],
        "permission",
    ),
    (
        &[
            "tool execution blocked by hook",
            "blocked by hook",
            "sensitive content",
            "secret-like",
            "secret like",
        ],
        "safety_policy",
    ),
    (
        &[
            "tool disabled by capability settings",
            "unknown mcp tool",
            "连接工具不可用",
            "没有启用",
            "连接服务启动失败",
        ],
        "capability_unavailable",
    ),
];

fn classify_tool_failure_kind(trace: &AgentToolTrace) -> &'static str {
    let evidence = [
        trace.result_summary.as_deref(),
        trace.command.as_deref(),
        Some(trace.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
    .to_ascii_lowercase();

    if trace.status == AgentToolStatus::Cancelled
        || contains_any(&evidence, &["interrupted", "cancelled", "中断"])
    {
        return "interrupted";
    }
    if evidence.contains("mcp") && contains_any(&evidence, &["timed out", "timeout", "超时"]) {
        return "mcp_timeout";
    }
    for (needles, kind) in TOOL_FAILURE_PATTERNS {
        if contains_any(&evidence, needles) {
            return kind;
        }
    }
    "tool_failed"
}

fn preview_status_is_conflict(label: &str, running: bool, can_start: bool, can_open: bool) -> bool {
    if running || can_start || can_open {
        return false;
    }
    let normalized = label.to_ascii_lowercase();
    contains_any(
        &normalized,
        &[
            "端口",
            "占用",
            "其他项目",
            "address already in use",
            "port conflict",
            "port already in use",
        ],
    )
}

fn classify_verification_failure_kind(trace: &AgentVerificationTrace) -> &'static str {
    let evidence = [
        trace.stdout_preview.as_deref(),
        trace.stderr_preview.as_deref(),
        trace.command.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
    .to_ascii_lowercase();

    if contains_any(&evidence, &["cancelled", "interrupted", "中断"]) {
        "interrupted"
    } else {
        "verification"
    }
}

fn preview_evidence_summary(
    project_path: Option<&str>,
    running: bool,
    can_start: bool,
    can_open: bool,
    label: &str,
    url: Option<&str>,
) -> String {
    let mut parts = vec![
        label.to_string(),
        format!("running={running}"),
        format!("can_start={can_start}"),
        format!("can_open={can_open}"),
    ];
    if let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("project_path={project_path}"));
    }
    if let Some(url) = url.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("url={url}"));
    }
    parts.join("; ").chars().take(240).collect()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn verification_transition_reason(status: &AgentVerificationStatus) -> Option<&'static str> {
    match status {
        AgentVerificationStatus::NotNeeded => None,
        AgentVerificationStatus::Skipped => Some("verification_skipped"),
        AgentVerificationStatus::Running => Some("verification_running"),
        AgentVerificationStatus::Passed => Some("verification_passed"),
        AgentVerificationStatus::Failed => Some("verification_failed"),
        AgentVerificationStatus::Error => Some("verification_error"),
    }
}

fn verification_transition_detail(trace: &AgentVerificationTrace) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(command) = trace.command.as_deref().filter(|value| !value.is_empty()) {
        parts.push(format!("command={command}"));
    }
    if let Some(exit_code) = trace.exit_code {
        parts.push(format!("exit_code={exit_code}"));
    }
    if let Some(stderr) = trace
        .stderr_preview
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!(
            "stderr={}",
            stderr.split_whitespace().collect::<Vec<_>>().join(" ")
        ));
    } else if let Some(stdout) = trace
        .stdout_preview
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!(
            "stdout={}",
            stdout.split_whitespace().collect::<Vec<_>>().join(" ")
        ));
    }
    if let Some(duration_ms) = trace.duration_ms {
        parts.push(format!("duration_ms={duration_ms}"));
    }

    let detail = parts.join("; ");
    if detail.is_empty() {
        None
    } else {
        Some(detail.chars().take(320).collect())
    }
}

fn verification_evidence_summary(trace: &AgentVerificationTrace) -> Option<String> {
    let mut parts = Vec::new();
    parts.push(format!("status={:?}", trace.status));
    if let Some(exit_code) = trace.exit_code {
        parts.push(format!("exit_code={exit_code}"));
    }
    if let Some(stderr) = trace
        .stderr_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!(
            "stderr={}",
            stderr.split_whitespace().collect::<Vec<_>>().join(" ")
        ));
    } else if let Some(stdout) = trace
        .stdout_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!(
            "stdout={}",
            stdout.split_whitespace().collect::<Vec<_>>().join(" ")
        ));
    }

    let summary = parts.join("; ");
    (!summary.trim().is_empty()).then(|| summary.chars().take(240).collect())
}

fn delivery_evidence_summary(summary: &crate::protocol::events::DeliverySummary) -> String {
    let mut parts = vec![
        summary.preview_label.clone(),
        summary.checkpoint_label.clone(),
        summary.next_action.clone(),
    ];
    if let Some(label) = summary.verification_label.as_deref() {
        parts.push(label.to_string());
    }
    if let Some(label) = summary.record_label.as_deref() {
        parts.push(label.to_string());
    }
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("；")
        .chars()
        .take(320)
        .collect()
}

fn extract_command(input: &serde_json::Value) -> Option<String> {
    ["command", "cmd"]
        .iter()
        .find_map(|key| input.get(key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_affected_files(input: &serde_json::Value) -> Vec<String> {
    let mut files = Vec::new();
    collect_files(input, &mut files);
    files.sort();
    files.dedup();
    files
}

fn collect_files(value: &serde_json::Value, files: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "path" | "file" | "file_path" | "filepath" | "target_file" | "target_path"
                ) {
                    if let Some(path) = value
                        .as_str()
                        .map(str::trim)
                        .filter(|path| !path.is_empty())
                    {
                        files.push(path.to_string());
                    }
                } else if matches!(key.as_str(), "files" | "paths") {
                    collect_files(value, files);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                match value {
                    serde_json::Value::String(path) if !path.trim().is_empty() => {
                        files.push(path.trim().to_string());
                    }
                    other => collect_files(other, files),
                }
            }
        }
        _ => {}
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn default_failure_kind() -> String {
    "unknown".to_string()
}

fn default_evidence_kind() -> AgentEvidenceKind {
    AgentEvidenceKind::Tool
}

#[cfg(test)]
#[path = "turn_state_tests.rs"]
mod tests;
