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
fn turn_transition_log_records_status_reason_and_detail() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Build turn ledger".to_string(),
    );

    turn.mark_status_with_reason(
        AgentTurnStatus::GatheringContext,
        "gather_context",
        Some("collect hidden context before model call"),
    );

    assert_eq!(turn.transition_log.len(), 2);
    assert_eq!(turn.transition_log[0].reason, "turn_started");
    assert_eq!(turn.transition_log[0].from_status, None);
    assert_eq!(turn.transition_log[0].to_status, AgentTurnStatus::Started);
    assert_eq!(turn.transition_log[1].reason, "gather_context");
    assert_eq!(
        turn.transition_log[1].from_status,
        Some(AgentTurnStatus::Started)
    );
    assert_eq!(
        turn.transition_log[1].to_status,
        AgentTurnStatus::GatheringContext
    );
    assert_eq!(
        turn.transition_log[1].detail.as_deref(),
        Some("collect hidden context before model call")
    );
}

#[test]
fn record_tool_adds_evidence_transition() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Build evidence ledger".to_string(),
    );

    turn.record_tool(AgentToolTrace {
        tool_call_id: "tool-1".to_string(),
        name: "bash".to_string(),
        category: AgentToolCategory::Shell,
        status: AgentToolStatus::Failed,
        started_at_ms: 10,
        ended_at_ms: Some(20),
        result_summary: Some("Exit code: 1 Stderr: build failed".to_string()),
        is_error: true,
        affected_files: vec!["package.json".to_string()],
        command: Some("npm run build".to_string()),
    });

    let transition = turn.transition_log.last().expect("tool transition");

    assert_eq!(transition.reason, "tool_failed");
    assert_eq!(transition.from_status, Some(AgentTurnStatus::Started));
    assert_eq!(transition.to_status, AgentTurnStatus::Started);
    assert!(transition
        .detail
        .as_deref()
        .expect("detail")
        .contains("tool=bash"));
    assert!(transition
        .detail
        .as_deref()
        .expect("detail")
        .contains("command=npm run build"));
    assert!(transition
        .detail
        .as_deref()
        .expect("detail")
        .contains("files=package.json"));
    assert!(transition
        .detail
        .as_deref()
        .expect("detail")
        .contains("Exit code: 1"));
}

#[test]
fn record_tool_appends_structured_tool_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Build tool evidence ledger".to_string(),
    );

    turn.record_tool(AgentToolTrace {
        tool_call_id: "tool-1".to_string(),
        name: "bash".to_string(),
        category: AgentToolCategory::Shell,
        status: AgentToolStatus::Failed,
        started_at_ms: 10,
        ended_at_ms: Some(20),
        result_summary: Some("Exit code: 1 Stderr: address already in use".to_string()),
        is_error: true,
        affected_files: vec!["package.json".to_string()],
        command: Some("npm run dev".to_string()),
    });

    assert_eq!(turn.evidence.len(), 1);
    let evidence = &turn.evidence[0];
    assert_eq!(evidence.tool_call_id, "tool-1");
    assert_eq!(evidence.tool_name, "bash");
    assert_eq!(evidence.category, AgentToolCategory::Shell);
    assert_eq!(evidence.status, AgentToolStatus::Failed);
    assert_eq!(evidence.outcome, "failed");
    assert_eq!(evidence.failure_kind.as_deref(), Some("preview_conflict"));
    assert_eq!(evidence.command.as_deref(), Some("npm run dev"));
    assert_eq!(evidence.affected_files, vec!["package.json"]);
    assert!(evidence
        .summary
        .as_deref()
        .expect("summary")
        .contains("address already in use"));
}

#[test]
fn old_turn_state_without_evidence_deserializes_with_empty_ledger() {
    let json = r#"{
            "turn_id":"turn-1",
            "session_id":"session-1",
            "workspace_path":"/workspace",
            "provider":"deepseek",
            "model":"deepseek-v4-flash",
            "route":"direct",
            "phase":"idle",
            "user_goal":"hello",
            "context":{"sources":[],"estimated_tokens":null,"budget_tokens":null,"omitted_sources":[]},
            "tools":[],
            "compact_events":[],
            "verification":{"status":"not_needed","command":null,"exit_code":null,"stdout_preview":null,"stderr_preview":null,"duration_ms":null,"completed_at_ms":null},
            "status":"started",
            "created_at_ms":1,
            "updated_at_ms":1
        }"#;

    let restored: AgentTurnState = serde_json::from_str(json).expect("deserialize old turn state");

    assert!(restored.evidence.is_empty());
}

#[test]
fn active_turn_is_marked_cancelled_when_restored_for_resume() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "workflow".to_string(),
        "implementation".to_string(),
        "生成第一版工具".to_string(),
    );
    turn.mark_status_with_reason(
        AgentTurnStatus::RunningTools,
        "tool_calls_requested",
        Some("model requested tool execution"),
    );

    turn.normalize_for_session_resume();

    assert_eq!(turn.status, AgentTurnStatus::Cancelled);
    let transition = turn.transition_log.last().expect("resume transition");
    assert_eq!(transition.reason, "session_restored_interrupted_turn");
    assert_eq!(transition.from_status, Some(AgentTurnStatus::RunningTools));
    assert_eq!(transition.to_status, AgentTurnStatus::Cancelled);
}

#[test]
fn resume_normalization_marks_running_tools_cancelled_for_recovery() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "workflow".to_string(),
        "implementation".to_string(),
        "安装依赖并生成第一版工具".to_string(),
    );
    turn.mark_status_with_reason(
        AgentTurnStatus::RunningTools,
        "tool_calls_requested",
        Some("model requested tool execution"),
    );
    turn.record_tool(running_tool_trace(
        "tool-1".to_string(),
        "bash".to_string(),
        &serde_json::json!({"command": "npm install"}),
        10,
    ));

    turn.normalize_for_session_resume();

    assert_eq!(turn.status, AgentTurnStatus::Cancelled);
    assert_eq!(turn.tools.len(), 1);
    assert_eq!(turn.tools[0].status, AgentToolStatus::Cancelled);
    assert!(turn.tools[0].is_error);
    assert_eq!(turn.tools[0].command.as_deref(), Some("npm install"));
    assert!(turn.tools[0]
        .result_summary
        .as_deref()
        .unwrap_or("")
        .contains("interrupted"));

    let evidence = turn
        .evidence
        .iter()
        .find(|item| item.tool_call_id == "tool-1")
        .expect("cancelled tool evidence");
    assert_eq!(evidence.status, AgentToolStatus::Cancelled);
    assert_eq!(evidence.outcome, "failed");
    assert_eq!(evidence.failure_kind.as_deref(), Some("interrupted"));
}

#[test]
fn late_failure_does_not_override_cancelled_turn() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "workflow".to_string(),
        "implementation".to_string(),
        "运行检查".to_string(),
    );
    turn.mark_status_with_reason(
        AgentTurnStatus::Cancelled,
        "user_cancelled",
        Some("session killed"),
    );

    turn.record_failure(AgentFailureTrace {
        kind: "verification".to_string(),
        stage: "verification_failed".to_string(),
        message: "verification cancelled".to_string(),
        retryable: false,
        recovery_advice: None,
        created_at_ms: 42,
    });

    assert_eq!(turn.status, AgentTurnStatus::Cancelled);
    assert!(turn.failure.is_none());
    assert_eq!(
        turn.transition_log
            .last()
            .expect("last transition")
            .to_status,
        AgentTurnStatus::Cancelled
    );
}

#[test]
fn record_tool_updates_existing_tool_trace_and_terminal_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "workflow".to_string(),
        "implementation".to_string(),
        "运行检查".to_string(),
    );

    turn.record_tool(running_tool_trace(
        "tool-1".to_string(),
        "bash".to_string(),
        &serde_json::json!({"command": "npm run build"}),
        10,
    ));
    turn.record_tool(completed_tool_trace(
        "tool-1".to_string(),
        "bash".to_string(),
        &serde_json::json!({"command": "npm run build"}),
        "Exit code: 0\nStdout: ok",
        10,
        20,
    ));

    assert_eq!(turn.tools.len(), 1);
    assert_eq!(turn.tools[0].status, AgentToolStatus::Completed);
    assert_eq!(turn.evidence.len(), 1);
    assert_eq!(turn.evidence[0].tool_call_id, "tool-1");
    assert_eq!(turn.evidence[0].outcome, "succeeded");
}

#[test]
fn set_verification_adds_evidence_transition() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Build verification ledger".to_string(),
    );

    turn.set_verification(AgentVerificationTrace {
        status: AgentVerificationStatus::Passed,
        command: Some("cargo test".to_string()),
        exit_code: Some(0),
        stdout_preview: Some("ok".to_string()),
        stderr_preview: None,
        duration_ms: Some(1200),
        completed_at_ms: Some(30),
    });

    let transition = turn.transition_log.last().expect("verification transition");

    assert_eq!(transition.reason, "verification_passed");
    assert_eq!(transition.from_status, Some(AgentTurnStatus::Started));
    assert_eq!(transition.to_status, AgentTurnStatus::Started);
    assert!(transition
        .detail
        .as_deref()
        .expect("detail")
        .contains("command=cargo test"));
    assert!(transition
        .detail
        .as_deref()
        .expect("detail")
        .contains("exit_code=0"));
}

#[test]
fn terminal_verification_appends_structured_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Build verification evidence".to_string(),
    );

    turn.set_verification(AgentVerificationTrace {
        status: AgentVerificationStatus::Failed,
        command: Some("npm run build".to_string()),
        exit_code: Some(1),
        stdout_preview: Some("".to_string()),
        stderr_preview: Some("build failed".to_string()),
        duration_ms: Some(1200),
        completed_at_ms: Some(30),
    });

    let evidence = turn.evidence.last().expect("verification evidence");
    assert_eq!(evidence.kind, AgentEvidenceKind::Verification);
    assert_eq!(evidence.tool_name, "verification");
    assert_eq!(evidence.outcome, "failed");
    assert_eq!(evidence.failure_kind.as_deref(), Some("verification"));
    assert_eq!(evidence.command.as_deref(), Some("npm run build"));
    assert!(evidence
        .summary
        .as_deref()
        .expect("summary")
        .contains("build failed"));
}

#[test]
fn cancelled_verification_evidence_is_interrupted_not_verification_failed() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Cancel verification".to_string(),
    );

    turn.set_verification(AgentVerificationTrace {
        status: AgentVerificationStatus::Error,
        command: Some("npm run build".to_string()),
        exit_code: None,
        stdout_preview: None,
        stderr_preview: Some("verification cancelled".to_string()),
        duration_ms: Some(200),
        completed_at_ms: Some(30),
    });

    let evidence = turn.evidence.last().expect("verification evidence");
    assert_eq!(evidence.kind, AgentEvidenceKind::Verification);
    assert_eq!(evidence.outcome, "failed");
    assert_eq!(evidence.failure_kind.as_deref(), Some("interrupted"));
}

#[test]
fn delivery_summary_appends_structured_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Build delivery evidence".to_string(),
    );
    let summary = crate::protocol::events::DeliverySummary {
        project_path: Some("/workspace".to_string()),
        preview_label: "预览未运行".to_string(),
        checkpoint_label: "还没有检查点".to_string(),
        next_action: "下一步：启动预览，并创建检查点。".to_string(),
        verification_label: Some("检查未通过".to_string()),
        verification_status: Some("failed".to_string()),
        verification_command: Some("npm run build".to_string()),
        record_label: Some("建议更新项目记录".to_string()),
        record_status: Some("pending".to_string()),
        record_target_pages: vec!["tasks.md".to_string()],
    };

    turn.record_delivery_summary(&summary);

    let evidence = turn.evidence.last().expect("delivery evidence");
    assert_eq!(evidence.kind, AgentEvidenceKind::Delivery);
    assert_eq!(evidence.tool_name, "delivery_status");
    assert_eq!(evidence.outcome, "needs_action");
    assert_eq!(evidence.failure_kind.as_deref(), Some("verification"));
    assert_eq!(evidence.command.as_deref(), Some("npm run build"));
    assert!(evidence
        .summary
        .as_deref()
        .expect("summary")
        .contains("预览未运行"));
}

#[test]
fn preview_status_appends_structured_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace/demo".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "agent-core".to_string(),
        "delivery".to_string(),
        "准备预览".to_string(),
    );

    turn.record_preview_status(
        Some("/workspace/demo"),
        false,
        true,
        false,
        "预览未运行",
        Some("http://localhost:5173"),
    );

    let evidence = turn.evidence.last().expect("preview evidence");
    assert_eq!(evidence.kind, AgentEvidenceKind::Preview);
    assert_eq!(evidence.tool_name, "preview_status");
    assert_eq!(evidence.status, AgentToolStatus::Completed);
    assert_eq!(evidence.outcome, "needs_action");
    assert_eq!(evidence.failure_kind, None);
    assert!(evidence
        .summary
        .as_deref()
        .expect("summary")
        .contains("/workspace/demo"));
    assert!(evidence
        .summary
        .as_deref()
        .expect("summary")
        .contains("http://localhost:5173"));
}

#[test]
fn preview_conflict_status_is_failed_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace/demo".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "agent-core".to_string(),
        "delivery".to_string(),
        "准备预览".to_string(),
    );

    turn.record_preview_status(
        Some("/workspace/demo"),
        false,
        false,
        false,
        "端口被其他项目占用",
        Some("http://localhost:5173"),
    );

    let evidence = turn.evidence.last().expect("preview evidence");
    assert_eq!(evidence.kind, AgentEvidenceKind::Preview);
    assert_eq!(evidence.outcome, "failed");
    assert_eq!(evidence.failure_kind.as_deref(), Some("preview_conflict"));
}

#[test]
fn checkpoint_status_appends_structured_evidence() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace/demo".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "agent-core".to_string(),
        "delivery".to_string(),
        "准备检查点".to_string(),
    );

    turn.record_checkpoint_status(true, true, false, "还没有检查点");

    let evidence = turn.evidence.last().expect("checkpoint evidence");
    assert_eq!(evidence.kind, AgentEvidenceKind::Checkpoint);
    assert_eq!(evidence.tool_name, "checkpoint_status");
    assert_eq!(evidence.status, AgentToolStatus::Completed);
    assert_eq!(evidence.outcome, "needs_action");
    assert_eq!(evidence.failure_kind, None);
    assert!(evidence
        .summary
        .as_deref()
        .expect("summary")
        .contains("dirty=true"));
}

#[test]
fn execution_plan_tracks_items_and_evidence_ids() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace/demo".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "agent-core".to_string(),
        "implementation".to_string(),
        "做一个喝水记录工具".to_string(),
    );

    turn.set_execution_plan(
        "生成可预览第一版".to_string(),
        vec![
            "确认需求".to_string(),
            "生成页面".to_string(),
            "检查交付".to_string(),
        ],
    );
    assert!(turn.update_execution_plan_item("step-2", AgentPlanItemStatus::InProgress, None, None,));
    assert!(turn.update_execution_plan_item(
        "step-2",
        AgentPlanItemStatus::Failed,
        Some("verification:20".to_string()),
        Some("verification".to_string()),
    ));

    let plan = turn.execution_plan.as_ref().expect("execution plan");
    assert_eq!(plan.objective, "生成可预览第一版");
    assert_eq!(plan.items.len(), 3);
    assert_eq!(plan.items[1].status, AgentPlanItemStatus::Failed);
    assert_eq!(plan.items[1].evidence_ids, vec!["verification:20"]);
    assert_eq!(plan.items[1].failure_kind.as_deref(), Some("verification"));
}

#[test]
fn old_turn_state_without_execution_plan_deserializes() {
    let json = r#"{
            "turn_id":"turn-1",
            "session_id":"session-1",
            "workspace_path":"/workspace",
            "provider":"deepseek",
            "model":"deepseek-v4",
            "route":"direct",
            "phase":"idle",
            "user_goal":"继续",
            "context":{"sources":[],"estimated_tokens":null,"budget_tokens":null,"omitted_sources":[]},
            "tools":[],
            "compact_events":[],
            "verification":{"status":"not_needed","command":null,"exit_code":null,"stdout_preview":null,"stderr_preview":null,"duration_ms":null,"completed_at_ms":null},
            "status":"started",
            "created_at_ms":1,
            "updated_at_ms":2
        }"#;

    let restored: AgentTurnState = serde_json::from_str(json).expect("deserialize old turn state");

    assert!(restored.execution_plan.is_none());
}

#[test]
fn old_turn_state_without_transition_log_deserializes_with_empty_ledger() {
    let json = r#"{
            "turn_id":"turn-1",
            "session_id":"session-1",
            "workspace_path":"/workspace",
            "provider":"deepseek",
            "model":"deepseek-v4",
            "route":"direct",
            "phase":"idle",
            "user_goal":"继续",
            "context":{"sources":[],"estimated_tokens":null,"budget_tokens":null,"omitted_sources":[]},
            "tools":[],
            "compact_events":[],
            "verification":{"status":"not_needed","command":null,"exit_code":null,"stdout_preview":null,"stderr_preview":null,"duration_ms":null,"completed_at_ms":null},
            "status":"started",
            "created_at_ms":1,
            "updated_at_ms":2
        }"#;

    let restored: AgentTurnState = serde_json::from_str(json).expect("deserialize old turn state");

    assert!(restored.transition_log.is_empty());
}

#[test]
fn turn_failure_trace_serializes_for_recovery() {
    let mut turn = sample_turn();
    turn.record_failure(AgentFailureTrace {
        kind: "api".to_string(),
        stage: "api_error".to_string(),
        message: "API error: upstream timed out after retry".to_string(),
        retryable: true,
        recovery_advice: Some(AgentRecoveryAdvice {
            action: "retry_from_failure".to_string(),
            reason: "上游服务暂时不可用".to_string(),
            instruction: "从失败点重试，先确认上一轮没有完成。".to_string(),
            safe_to_auto_retry: true,
            requires_user_action: false,
        }),
        created_at_ms: 42,
    });

    let json = serde_json::to_string(&turn).expect("serialize turn state");
    let restored: AgentTurnState = serde_json::from_str(&json).expect("deserialize turn state");
    let failure = restored.failure.expect("failure trace");

    assert!(json.contains(r#""failure""#));
    assert!(json.contains(r#""kind":"api""#));
    assert_eq!(restored.status, AgentTurnStatus::Failed);
    assert_eq!(failure.kind, "api");
    assert_eq!(failure.stage, "api_error");
    assert_eq!(failure.message, "API error: upstream timed out after retry");
    assert!(failure.retryable);
    let advice = failure.recovery_advice.expect("recovery advice");
    assert_eq!(advice.action, "retry_from_failure");
    assert!(advice.safe_to_auto_retry);
    assert!(!advice.requires_user_action);
}

#[test]
fn old_failure_trace_without_kind_deserializes_as_unknown() {
    let json = r#"{
            "turn_id":"turn-1",
            "session_id":"session-1",
            "workspace_path":"/workspace",
            "provider":"deepseek",
            "model":"deepseek-v4",
            "route":"direct",
            "phase":"idle",
            "user_goal":"继续",
            "context":{"sources":[],"estimated_tokens":null,"budget_tokens":null,"omitted_sources":[]},
            "tools":[],
            "compact_events":[],
            "verification":{"status":"not_needed","command":null,"exit_code":null,"stdout_preview":null,"stderr_preview":null,"duration_ms":null,"completed_at_ms":null},
            "failure":{"stage":"api_error","message":"API error: timeout","retryable":true,"created_at_ms":42},
            "status":"failed",
            "created_at_ms":1,
            "updated_at_ms":2
        }"#;

    let restored: AgentTurnState = serde_json::from_str(json).expect("deserialize old failure");

    assert_eq!(
        restored.failure.as_ref().expect("failure trace").kind,
        "unknown".to_string()
    );
    assert!(restored
        .failure
        .expect("failure trace")
        .recovery_advice
        .is_none());
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
            "verification_status": "passed",
            "model_rounds": 0,
            "tool_call_count": 0,
            "failed_tool_count": 0,
            "estimated_context_tokens": 42,
            "stop_reason": null
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
fn turn_metadata_carries_hidden_input_intent_without_projecting_it() {
    let mut metadata = AgentTurnMetadata::default_for_session(
        "session-1".to_string(),
        "/workspace".to_string(),
        "deepseek".to_string(),
        "deepseek-v4".to_string(),
        "修一下按钮".to_string(),
    );
    metadata.input_intent = AgentTurnInputIntent {
        slash_command: Some("/fix".to_string()),
        file_references: vec!["src/App.tsx".to_string()],
        selected_connectors: vec!["obsidian: Forge".to_string()],
        matched_skills: vec!["fix-flow（触发：排查并修复）".to_string()],
        active_hooks: vec!["Workspace Boundary Guard".to_string()],
        enabled_mcp_servers: vec!["obsidian".to_string()],
        available_mcp_tools: vec!["mcp__obsidian__search_notes".to_string()],
    };

    let turn = metadata.into_turn_state("turn-1".to_string());
    let projection_json = serde_json::to_value(turn.to_projection()).expect("serialize projection");

    assert_eq!(turn.input_intent.slash_command.as_deref(), Some("/fix"));
    assert_eq!(turn.input_intent.file_references, vec!["src/App.tsx"]);
    assert_eq!(
        turn.input_intent.selected_connectors,
        vec!["obsidian: Forge"]
    );
    assert_eq!(
        turn.input_intent.matched_skills,
        vec!["fix-flow（触发：排查并修复）"]
    );
    assert_eq!(
        turn.input_intent.active_hooks,
        vec!["Workspace Boundary Guard"]
    );
    assert_eq!(turn.input_intent.enabled_mcp_servers, vec!["obsidian"]);
    assert_eq!(
        turn.input_intent.available_mcp_tools,
        vec!["mcp__obsidian__search_notes"]
    );
    assert!(projection_json.get("input_intent").is_none());
}

#[test]
fn turn_state_tracks_budget_counters_from_zero() {
    let turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Budget test".to_string(),
    );

    assert_eq!(turn.model_rounds, 0);
    assert_eq!(turn.tool_call_count, 0);
    assert_eq!(turn.failed_tool_count, 0);
}

#[test]
fn turn_state_stop_reason_defaults_to_none() {
    let turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Stop reason test".to_string(),
    );

    assert_eq!(turn.stop_reason, None);
}

#[test]
fn turn_state_set_stop_reason_persists_and_projects() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Stop reason test".to_string(),
    );

    turn.set_stop_reason("model_round_limit");

    assert_eq!(turn.stop_reason, Some("model_round_limit".to_string()));

    let projection = turn.to_projection();
    assert_eq!(
        projection.stop_reason,
        Some("model_round_limit".to_string())
    );
}

#[test]
fn old_turn_state_without_stop_reason_deserializes_with_none() {
    let json = r#"{
            "turn_id":"turn-1",
            "session_id":"session-1",
            "workspace_path":"/workspace",
            "provider":"deepseek",
            "model":"deepseek-v4",
            "route":"direct",
            "phase":"idle",
            "user_goal":"继续",
            "context":{"sources":[],"estimated_tokens":null,"budget_tokens":null,"omitted_sources":[]},
            "tools":[],
            "compact_events":[],
            "verification":{"status":"not_needed","command":null,"exit_code":null,"stdout_preview":null,"stderr_preview":null,"duration_ms":null,"completed_at_ms":null},
            "status":"started",
            "created_at_ms":1,
            "updated_at_ms":2
        }"#;

    let restored: AgentTurnState = serde_json::from_str(json).expect("deserialize old turn state");
    assert_eq!(restored.stop_reason, None);
}

#[test]
fn projection_exposes_budget_counters_and_context_tokens() {
    let mut turn = sample_turn();
    turn.model_rounds = 5;
    turn.tool_call_count = 12;
    turn.failed_tool_count = 2;

    let projection = turn.to_projection();

    assert_eq!(projection.model_rounds, 5);
    assert_eq!(projection.tool_call_count, 12);
    assert_eq!(projection.failed_tool_count, 2);
    assert_eq!(projection.estimated_context_tokens, Some(42));
}

#[test]
fn projection_budget_counters_default_to_zero() {
    let turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Default counter test".to_string(),
    );

    let projection = turn.to_projection();

    assert_eq!(projection.model_rounds, 0);
    assert_eq!(projection.tool_call_count, 0);
    assert_eq!(projection.failed_tool_count, 0);
    assert_eq!(projection.estimated_context_tokens, None);
}

#[test]
fn old_turn_state_without_budget_counters_deserializes_with_defaults() {
    let json = r#"{
            "turn_id":"turn-1",
            "session_id":"session-1",
            "workspace_path":"/workspace",
            "provider":"deepseek",
            "model":"deepseek-v4",
            "route":"direct",
            "phase":"idle",
            "user_goal":"继续",
            "context":{"sources":[],"estimated_tokens":null,"budget_tokens":null,"omitted_sources":[]},
            "tools":[],
            "compact_events":[],
            "verification":{"status":"not_needed","command":null,"exit_code":null,"stdout_preview":null,"stderr_preview":null,"duration_ms":null,"completed_at_ms":null},
            "status":"started",
            "created_at_ms":1,
            "updated_at_ms":2
        }"#;

    let restored: AgentTurnState = serde_json::from_str(json).expect("deserialize old turn state");

    assert_eq!(restored.model_rounds, 0);
    assert_eq!(restored.tool_call_count, 0);
    assert_eq!(restored.failed_tool_count, 0);
}

#[test]
fn turn_state_budget_counters_serialize_and_deserialize() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "agent-core".to_string(),
        "phase-1".to_string(),
        "Budget serialization test".to_string(),
    );
    turn.model_rounds = 7;
    turn.tool_call_count = 15;
    turn.failed_tool_count = 3;

    let json = serde_json::to_string(&turn).expect("serialize");
    let restored: AgentTurnState = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.model_rounds, 7);
    assert_eq!(restored.tool_call_count, 15);
    assert_eq!(restored.failed_tool_count, 3);
}

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
fn unknown_mcp_tool_result_marks_trace_failed() {
    let trace = completed_tool_trace(
        "tool-1".to_string(),
        "mcp__missing__tool".to_string(),
        &serde_json::json!({}),
        "Unknown MCP tool: mcp__missing__tool",
        10,
        20,
    );

    assert!(trace.is_error);
    assert_eq!(trace.status, AgentToolStatus::Failed);
    assert_eq!(trace.category, AgentToolCategory::Mcp);
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

#[test]
fn missing_tool_result_marks_trace_failed() {
    let trace = completed_tool_trace(
        "tool-5".to_string(),
        "read_file".to_string(),
        &serde_json::json!({ "path": "src/main.rs" }),
        "Tool result missing: read_file",
        10,
        11,
    );

    assert_eq!(trace.status, AgentToolStatus::Failed);
    assert!(trace.is_error);
    assert_eq!(trace.affected_files, vec!["src/main.rs".to_string()]);
}
