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
    pub context: AgentTurnContextSnapshot,
    pub tools: Vec<AgentToolTrace>,
    pub compact_events: Vec<AgentCompactTrace>,
    pub verification: AgentVerificationTrace,
    pub status: AgentTurnStatus,
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
        }
    }

    pub fn into_turn_state(self, turn_id: String) -> AgentTurnState {
        AgentTurnState::new(
            turn_id,
            self.session_id,
            self.workspace_path,
            self.provider,
            self.model,
            self.route,
            self.phase,
            self.user_goal,
        )
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
            context: AgentTurnContextSnapshot::default(),
            tools: Vec::new(),
            compact_events: Vec::new(),
            verification: AgentVerificationTrace::default(),
            status: AgentTurnStatus::Started,
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    pub fn mark_status(&mut self, status: AgentTurnStatus) {
        self.status = status;
        self.touch();
    }

    pub fn record_tool(&mut self, trace: AgentToolTrace) {
        self.tools.push(trace);
        self.touch();
    }

    pub fn record_compact(&mut self, trace: AgentCompactTrace) {
        self.compact_events.push(trace);
        self.touch();
    }

    pub fn set_verification(&mut self, trace: AgentVerificationTrace) {
        self.verification = trace;
        self.touch();
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
}

pub fn classify_tool_category(name: &str) -> AgentToolCategory {
    match name {
        "read_file" | "read" | "list_directory" | "ls" | "list" | "search_files" | "glob"
        | "search_content" | "grep" | "web_search" | "web_fetch" | "git_diff" => {
            AgentToolCategory::Read
        }
        "write_file" | "write_to_file" | "write" | "edit_file" | "edit" | "apply_patch"
        | "create_file" | "delete_file" | "move_file" => AgentToolCategory::Write,
        "run_shell" | "bash" | "shell" | "exec" | "execute_command" => AgentToolCategory::Shell,
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

fn shell_exit_code(result: &str) -> Option<i32> {
    result.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Exit code:")
            .and_then(|code| code.trim().parse::<i32>().ok())
    })
}

fn is_errorish_tool_result(result: &str) -> bool {
    result.starts_with("Error:")
        || result.starts_with("Denied:")
        || result.starts_with("Search blocked:")
        || result.starts_with("Search failed:")
        || result.starts_with("Search timed out")
        || result.starts_with("Permission denied")
        || result.starts_with("Tool disabled")
        || result.starts_with("Tool execution blocked")
}

fn summarize_tool_result(result: &str) -> Option<String> {
    let summary = result.split_whitespace().collect::<Vec<_>>().join(" ");
    if summary.is_empty() {
        None
    } else {
        Some(summary.chars().take(240).collect())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_turn() -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "agent-core".to_string(),
            "phase-1".to_string(),
            "Build turn state".to_string(),
        );
        turn.context.sources.push(AgentTurnContextSource {
            kind: "file".to_string(),
            label: "turn_state.rs".to_string(),
            reason: "requested by spec".to_string(),
            estimated_tokens: Some(42),
            injected: true,
        });
        turn.context.estimated_tokens = Some(42);
        turn.context.budget_tokens = Some(1000);
        turn.context.omitted_sources.push(AgentTurnContextSource {
            kind: "file".to_string(),
            label: "session.rs".to_string(),
            reason: "out of scope".to_string(),
            estimated_tokens: Some(900),
            injected: false,
        });
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "read_file".to_string(),
            category: AgentToolCategory::Read,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(15),
            result_summary: Some("Read a file".to_string()),
            is_error: false,
            affected_files: vec!["src-tauri/src/agent/turn_state.rs".to_string()],
            command: None,
        });
        turn.record_compact(AgentCompactTrace {
            reason: "history_window".to_string(),
            retained_messages: 8,
            compacted_messages: 22,
            estimated_tokens_before: Some(1000),
            estimated_tokens_after: Some(250),
            created_at_ms: 20,
        });
        turn.set_verification(AgentVerificationTrace {
            status: AgentVerificationStatus::Passed,
            command: Some("cargo test agent::".to_string()),
            exit_code: Some(0),
            stdout_preview: Some("4 passed".to_string()),
            stderr_preview: None,
            duration_ms: Some(1200),
            completed_at_ms: Some(30),
        });
        turn
    }

    #[test]
    fn turn_state_serializes_roundtrip() {
        let turn = sample_turn();

        let json = serde_json::to_string(&turn).expect("serialize turn state");
        let restored: AgentTurnState = serde_json::from_str(&json).expect("deserialize turn state");

        assert_eq!(restored.turn_id, "turn-1");
        assert_eq!(restored.session_id, "session-1");
        assert_eq!(restored.workspace_path, "/workspace");
        assert_eq!(restored.provider, "openai");
        assert_eq!(restored.model, "gpt-5");
        assert_eq!(restored.route, "agent-core");
        assert_eq!(restored.phase, "phase-1");
        assert_eq!(restored.user_goal, "Build turn state");
        assert_eq!(restored.context.sources.len(), 1);
        assert_eq!(restored.context.estimated_tokens, Some(42));
        assert_eq!(restored.context.budget_tokens, Some(1000));
        assert_eq!(restored.context.omitted_sources.len(), 1);
        assert_eq!(restored.tools.len(), 1);
        assert_eq!(
            restored.tools[0].result_summary.as_deref(),
            Some("Read a file")
        );
        assert!(!restored.tools[0].is_error);
        assert_eq!(
            restored.tools[0].affected_files,
            vec!["src-tauri/src/agent/turn_state.rs"]
        );
        assert_eq!(restored.compact_events.len(), 1);
        assert_eq!(restored.compact_events[0].retained_messages, 8);
        assert_eq!(restored.compact_events[0].compacted_messages, 22);
        assert_eq!(
            restored.verification.status,
            AgentVerificationStatus::Passed
        );
        assert_eq!(restored.verification.exit_code, Some(0));
        assert_eq!(restored.status, AgentTurnStatus::Started);
    }

    #[test]
    fn mark_status_updates_status_timestamp_and_snake_case_json() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "agent-core".to_string(),
            "phase-1".to_string(),
            "Build turn state".to_string(),
        );
        let previous_updated_at = turn.updated_at_ms;

        assert_eq!(turn.status, AgentTurnStatus::Started);

        turn.mark_status(AgentTurnStatus::GatheringContext);

        assert_eq!(turn.status, AgentTurnStatus::GatheringContext);
        assert!(turn.updated_at_ms >= previous_updated_at);
        let json = serde_json::to_string(&turn).expect("serialize turn state");
        assert!(json.contains(r#""status":"gathering_context""#));
    }

    #[test]
    fn status_enums_cover_agent_core_plan_values() {
        let statuses = [
            AgentTurnStatus::Started,
            AgentTurnStatus::GatheringContext,
            AgentTurnStatus::CallingModel,
            AgentTurnStatus::RunningTools,
            AgentTurnStatus::Verifying,
            AgentTurnStatus::Completed,
            AgentTurnStatus::Failed,
            AgentTurnStatus::Cancelled,
        ];

        let json = serde_json::to_value(statuses).expect("serialize statuses");

        assert_eq!(
            json,
            serde_json::json!([
                "started",
                "gathering_context",
                "calling_model",
                "running_tools",
                "verifying",
                "completed",
                "failed",
                "cancelled"
            ])
        );
    }

    #[test]
    fn verification_status_enums_cover_agent_core_plan_values() {
        let statuses = [
            AgentVerificationStatus::NotNeeded,
            AgentVerificationStatus::Skipped,
            AgentVerificationStatus::Running,
            AgentVerificationStatus::Passed,
            AgentVerificationStatus::Failed,
            AgentVerificationStatus::Error,
        ];

        let json = serde_json::to_value(statuses).expect("serialize statuses");

        assert_eq!(
            json,
            serde_json::json!([
                "not_needed",
                "skipped",
                "running",
                "passed",
                "failed",
                "error"
            ])
        );
    }

    #[test]
    fn projection_exposes_only_product_safe_turn_fields() {
        let mut turn = sample_turn();
        turn.mark_status(AgentTurnStatus::RunningTools);

        let projection = turn.to_projection();
        let json = serde_json::to_value(&projection).expect("serialize projection");

        assert_eq!(projection.session_id, "session-1");
        assert_eq!(projection.status, AgentTurnStatus::RunningTools);
        assert_eq!(projection.step_label, "处理项目");
        assert_eq!(projection.workspace_path, "/workspace");
        assert_eq!(projection.compact_count, 1);
        assert_eq!(
            projection.verification_status,
            AgentVerificationStatus::Passed
        );
        assert_eq!(
            json,
            serde_json::json!({
                "session_id": "session-1",
                "status": "running_tools",
                "step_label": "处理项目",
                "workspace_path": "/workspace",
                "compact_count": 1,
                "verification_status": "passed"
            })
        );
    }

    #[test]
    fn default_turn_metadata_keeps_legacy_send_message_compatible() {
        let metadata = AgentTurnMetadata::default_for_session(
            "session-1".to_string(),
            "/workspace".to_string(),
            "deepseek".to_string(),
            "deepseek-v4".to_string(),
            "hello".to_string(),
        );

        let turn = metadata.into_turn_state("turn-1".to_string());

        assert_eq!(turn.session_id, "session-1");
        assert_eq!(turn.workspace_path, "/workspace");
        assert_eq!(turn.provider, "deepseek");
        assert_eq!(turn.model, "deepseek-v4");
        assert_eq!(turn.route, "direct");
        assert_eq!(turn.phase, "idle");
        assert_eq!(turn.user_goal, "hello");
        assert_eq!(turn.status, AgentTurnStatus::Started);
    }

    #[test]
    fn tool_category_matches_agent_core_buckets() {
        assert_eq!(classify_tool_category("read_file"), AgentToolCategory::Read);
        assert_eq!(
            classify_tool_category("write_file"),
            AgentToolCategory::Write
        );
        assert_eq!(
            classify_tool_category("write_to_file"),
            AgentToolCategory::Write
        );
        assert_eq!(classify_tool_category("edit"), AgentToolCategory::Write);
        assert_eq!(classify_tool_category("bash"), AgentToolCategory::Shell);
        assert_eq!(
            classify_tool_category("delegate_task"),
            AgentToolCategory::Delegate
        );
        assert_eq!(
            classify_tool_category("unknown_tool"),
            AgentToolCategory::Other
        );
    }

    #[test]
    fn completed_tool_trace_extracts_summary_files_and_command() {
        let input = serde_json::json!({
            "command": "cargo test agent::",
            "path": "src-tauri/src/agent/session.rs",
            "files": ["src-tauri/src/agent/turn_state.rs"]
        });

        let trace = completed_tool_trace(
            "tool-1".to_string(),
            "bash".to_string(),
            &input,
            "ok\nsecond line",
            10,
            25,
        );

        assert_eq!(trace.tool_call_id, "tool-1");
        assert_eq!(trace.name, "bash");
        assert_eq!(trace.category, AgentToolCategory::Shell);
        assert_eq!(trace.status, AgentToolStatus::Completed);
        assert!(!trace.is_error);
        assert_eq!(trace.command.as_deref(), Some("cargo test agent::"));
        assert_eq!(trace.result_summary.as_deref(), Some("ok second line"));
        assert_eq!(
            trace.affected_files,
            vec![
                "src-tauri/src/agent/session.rs".to_string(),
                "src-tauri/src/agent/turn_state.rs".to_string()
            ]
        );
        assert_eq!(trace.started_at_ms, 10);
        assert_eq!(trace.ended_at_ms, Some(25));
    }

    #[test]
    fn failed_tool_trace_detects_errorish_results() {
        let trace = completed_tool_trace(
            "tool-2".to_string(),
            "write_file".to_string(),
            &serde_json::json!({ "file_path": "src/lib.rs" }),
            "Error: permission denied",
            10,
            11,
        );

        assert_eq!(trace.category, AgentToolCategory::Write);
        assert_eq!(trace.status, AgentToolStatus::Failed);
        assert!(trace.is_error);
        assert_eq!(trace.affected_files, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn shell_trace_with_exit_code_one_is_failed() {
        let trace = completed_tool_trace(
            "tool-3".to_string(),
            "bash".to_string(),
            &serde_json::json!({ "command": "cargo test" }),
            "Exit code: 1\nStdout:\n\nStderr:\nfailed",
            10,
            11,
        );

        assert_eq!(trace.status, AgentToolStatus::Failed);
        assert!(trace.is_error);
    }

    #[test]
    fn shell_trace_with_exit_code_zero_is_completed() {
        let trace = completed_tool_trace(
            "tool-4".to_string(),
            "bash".to_string(),
            &serde_json::json!({ "command": "cargo test" }),
            "Exit code: 0\nStdout:\nok\nStderr:\n",
            10,
            11,
        );

        assert_eq!(trace.status, AgentToolStatus::Completed);
        assert!(!trace.is_error);
    }
}
