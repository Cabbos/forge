use crate::agent::time::now_ms;
use crate::agent::turn_state::{
    AgentEvidenceKind, AgentFailureTrace, AgentPlanItemStatus, AgentRecoveryAdvice,
    AgentToolEvidence, AgentToolStatus, AgentTurnState, AgentTurnStatus, AgentVerificationStatus,
    AgentVerificationTrace,
};

pub(crate) fn api_failure_trace(message: &str) -> AgentFailureTrace {
    let kind = classify_failure_kind(message);
    AgentFailureTrace {
        kind: kind.to_string(),
        stage: "api_error".to_string(),
        message: summarize_failure_message(message),
        retryable: true,
        recovery_advice: Some(recovery_advice_for_failure_kind(kind)),
        created_at_ms: now_ms(),
    }
}

fn classify_failure_kind(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();
    if contains_any(
        &normalized,
        &[
            "api key",
            "invalid key",
            "authentication",
            "unauthorized",
            "401",
        ],
    ) {
        return "auth";
    }
    if contains_any(
        &normalized,
        &["rate limit", "rate_limit", "too many requests", "429"],
    ) {
        return "rate_limit";
    }
    if contains_any(
        &normalized,
        &[
            "context length",
            "context window",
            "maximum context",
            "too many tokens",
            "token limit",
        ],
    ) {
        return "context_overflow";
    }
    if contains_any(
        &normalized,
        &[
            "outside workspace",
            "workspace boundary",
            "search blocked",
            "write boundary",
        ],
    ) {
        return "workspace_boundary";
    }
    if contains_any(
        &normalized,
        &[
            "port conflict",
            "address already in use",
            "port already in use",
        ],
    ) {
        return "preview_conflict";
    }
    if contains_any(
        &normalized,
        &[
            "eresolve",
            "dependency tree",
            "npm install failed",
            "failed to install dependencies",
            "could not resolve dependency",
        ],
    ) {
        return "dependency_install_failed";
    }
    if contains_any(
        &normalized,
        &[
            "command not found: pnpm",
            "pnpm: command not found",
            "no such file or directory: pnpm",
            "command not found: yarn",
            "yarn: command not found",
            "command not found: bun",
            "bun: command not found",
        ],
    ) {
        return "package_manager_missing";
    }
    if contains_any(
        &normalized,
        &["command not found", "not recognized as an internal"],
    ) {
        return "command_not_found";
    }
    if contains_any(
        &normalized,
        &[
            "patch does not apply",
            "git apply failed",
            "checkpoint failed",
            "检查点失败",
            "无法读取检查点",
        ],
    ) {
        return "checkpoint_failed";
    }
    if contains_any(
        &normalized,
        &["file not found", "no such file or directory", "找不到文件"],
    ) {
        return "file_not_found";
    }
    if contains_any(
        &normalized,
        &[
            "tool result missing",
            "tool_use id",
            "matching tool_result",
            "tool call mismatch",
        ],
    ) {
        return "tool_call_mismatch";
    }
    if normalized.contains("mcp") && contains_any(&normalized, &["timed out", "timeout", "超时"])
    {
        return "mcp_timeout";
    }
    if contains_any(
        &normalized,
        &["permission denied", "access denied", "denied:"],
    ) {
        return "permission";
    }
    if contains_any(
        &normalized,
        &[
            "tool execution blocked by hook",
            "blocked by hook",
            "sensitive content",
            "secret-like",
            "secret like",
        ],
    ) {
        return "safety_policy";
    }
    if contains_any(
        &normalized,
        &[
            "tool disabled by capability settings",
            "tool disabled",
            "unknown mcp tool",
            "mcp server",
            "is not enabled",
            "连接工具不可用",
            "没有启用",
            "连接服务启动失败",
        ],
    ) {
        return "capability_unavailable";
    }
    "api"
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub(crate) fn verification_failure_trace(trace: &AgentVerificationTrace) -> AgentFailureTrace {
    AgentFailureTrace {
        kind: "verification".to_string(),
        stage: "verification_failed".to_string(),
        message: summarize_failure_message(&format_verification_failure_message(trace)),
        retryable: false,
        recovery_advice: Some(recovery_advice_for_failure_kind("verification")),
        created_at_ms: now_ms(),
    }
}

pub(crate) fn build_recovery_context(
    previous_turn: Option<&AgentTurnState>,
    follow_up: &str,
) -> Option<String> {
    let turn = previous_turn?;
    if !should_attach_recovery_context(follow_up) {
        return None;
    }
    let mut lines = match turn.status {
        AgentTurnStatus::Failed => {
            let failure = turn.failure.as_ref()?;
            let failure_kind = resolved_failure_kind(turn, failure);
            let recovery_advice = failure
                .recovery_advice
                .as_ref()
                .filter(|_| failure.kind == failure_kind)
                .cloned()
                .unwrap_or_else(|| recovery_advice_for_failure_kind(failure_kind));
            vec![
                "上一轮任务失败，需要把下面信息作为恢复线索。".to_string(),
                format!("上一轮目标：{}", turn.user_goal),
                format!("当前用户继续请求：{}", follow_up.trim()),
                format!("工作空间：{}", turn.workspace_path),
                format!("失败类别：{}", failure_kind),
                format!("失败阶段：{}", failure.stage),
                format!("失败摘要：{}", failure.message),
                format!("可重试：{}", if failure.retryable { "是" } else { "否" }),
                format!("恢复动作：{}", recovery_advice.action),
                format!("恢复策略：{}", recovery_advice.instruction),
                "处理规则：优先从失败点恢复；不要假装上一步已经成功；必要时先检查当前工作空间状态，再继续。".to_string(),
            ]
        }
        AgentTurnStatus::Cancelled => vec![
            "上一轮任务被中断，需要把下面信息作为恢复线索。".to_string(),
            format!("上一轮目标：{}", turn.user_goal),
            format!("当前用户继续请求：{}", follow_up.trim()),
            format!("工作空间：{}", turn.workspace_path),
            "中断类别：interrupted".to_string(),
            "恢复动作：inspect_interrupted_state".to_string(),
            "恢复策略：先检查上一轮正在执行的工具和当前工作空间状态；不要假设长命令已完成。"
                .to_string(),
            "处理规则：需要先确认当前工作空间状态，再继续；不要假装上一轮已经完整完成。"
                .to_string(),
        ],
        _ => return None,
    };

    if let Some(command) = turn
        .verification
        .command
        .as_deref()
        .filter(|command| !command.trim().is_empty())
    {
        lines.push(format!("最近验证命令：{command}"));
    }
    if turn.verification.status != AgentVerificationStatus::NotNeeded {
        lines.push(format!("最近验证状态：{:?}", turn.verification.status));
    }

    if let Some(command) = turn
        .input_intent
        .slash_command
        .as_deref()
        .filter(|command| !command.trim().is_empty())
    {
        lines.push(format!("上一轮动作：{command}"));
    }
    if !turn.input_intent.file_references.is_empty() {
        lines.push(format!(
            "上一轮参考文件：{}",
            turn.input_intent.file_references.join(", ")
        ));
    }
    if !turn.input_intent.selected_connectors.is_empty() {
        lines.push(format!(
            "上一轮连接资料：{}",
            turn.input_intent.selected_connectors.join(", ")
        ));
    }
    if !turn.input_intent.matched_skills.is_empty() {
        lines.push(format!(
            "上一轮启用技能：{}",
            summarize_recovery_items(&turn.input_intent.matched_skills, 8)
        ));
    }
    if !turn.input_intent.active_hooks.is_empty() {
        lines.push(format!(
            "上一轮安全规则：{}",
            summarize_recovery_items(&turn.input_intent.active_hooks, 8)
        ));
    }
    if !turn.input_intent.enabled_mcp_servers.is_empty() {
        lines.push(format!(
            "上一轮可用连接：{}",
            summarize_recovery_items(&turn.input_intent.enabled_mcp_servers, 8)
        ));
    }
    if !turn.input_intent.available_mcp_tools.is_empty() {
        lines.push(format!(
            "上一轮可用连接工具：{}",
            summarize_recovery_items(&turn.input_intent.available_mcp_tools, 8)
        ));
    }

    append_execution_plan_recovery_lines(&mut lines, turn);

    let evidence = failed_recovery_evidence(turn);
    for item in evidence.iter().take(3) {
        let mut detail = format!("失败证据：{}", item.tool_name);
        if let Some(kind) = item
            .failure_kind
            .as_deref()
            .filter(|kind| !kind.trim().is_empty())
        {
            detail.push_str(&format!("；失败类别：{kind}"));
        }
        if let Some(command) = item
            .command
            .as_deref()
            .filter(|command| !command.trim().is_empty())
        {
            detail.push_str(&format!("；命令：{command}"));
        }
        if let Some(summary) = item
            .summary
            .as_deref()
            .filter(|summary| !summary.trim().is_empty())
        {
            detail.push_str(&format!("；结果：{summary}"));
        }
        lines.push(detail);
    }

    let affected_files = evidence
        .iter()
        .flat_map(|item| item.affected_files.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    if !affected_files.is_empty() {
        lines.push(format!(
            "上一轮涉及文件：{}",
            affected_files.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    let transition_lines = recent_recovery_transition_lines(turn, 5);
    if !transition_lines.is_empty() {
        lines.push("最近运行轨迹：".to_string());
        lines.extend(
            transition_lines
                .into_iter()
                .map(|transition| format!("- {transition}")),
        );
    }

    Some(lines.join("\n"))
}

fn failed_recovery_evidence(turn: &AgentTurnState) -> Vec<AgentToolEvidence> {
    let evidence = turn
        .evidence
        .iter()
        .filter(|item| item.outcome == "failed")
        .cloned()
        .collect::<Vec<_>>();
    if !evidence.is_empty() {
        return evidence;
    }

    turn.tools
        .iter()
        .filter(|tool| tool.is_error || tool.status == AgentToolStatus::Failed)
        .map(|tool| AgentToolEvidence {
            kind: AgentEvidenceKind::Tool,
            evidence_id: format!("tool:{}", tool.tool_call_id),
            tool_call_id: tool.tool_call_id.clone(),
            tool_name: tool.name.clone(),
            category: tool.category.clone(),
            status: tool.status.clone(),
            outcome: "failed".to_string(),
            summary: tool.result_summary.clone(),
            command: tool.command.clone(),
            affected_files: tool.affected_files.clone(),
            failure_kind: None,
            created_at_ms: tool.ended_at_ms.unwrap_or(tool.started_at_ms),
        })
        .collect()
}

fn append_execution_plan_recovery_lines(lines: &mut Vec<String>, turn: &AgentTurnState) {
    let Some(plan) = turn.execution_plan.as_ref() else {
        return;
    };
    lines.push(format!("上一轮计划：{}", plan.objective));
    let unfinished = plan
        .items
        .iter()
        .filter(|item| item.status != AgentPlanItemStatus::Completed)
        .map(|item| {
            let status = match item.status {
                AgentPlanItemStatus::Pending => "pending",
                AgentPlanItemStatus::InProgress => "in_progress",
                AgentPlanItemStatus::Completed => "completed",
                AgentPlanItemStatus::Failed => "failed",
                AgentPlanItemStatus::Skipped => "skipped",
            };
            if let Some(kind) = item
                .failure_kind
                .as_deref()
                .filter(|kind| !kind.trim().is_empty())
            {
                format!("{}（{status}, {kind}）", item.title)
            } else {
                format!("{}（{status}）", item.title)
            }
        })
        .collect::<Vec<_>>();
    if !unfinished.is_empty() {
        lines.push(format!("未完成步骤：{}", unfinished.join("；")));
    }
}

fn recent_recovery_transition_lines(turn: &AgentTurnState, limit: usize) -> Vec<String> {
    let mut transitions = turn
        .transition_log
        .iter()
        .rev()
        .filter(|transition| transition.reason != "turn_started")
        .take(limit)
        .collect::<Vec<_>>();
    transitions.reverse();

    transitions
        .into_iter()
        .map(|transition| {
            let from = transition
                .from_status
                .as_ref()
                .map(|status| format!("{status:?}"))
                .unwrap_or_else(|| "None".to_string());
            let to = format!("{:?}", transition.to_status);
            let mut line = format!("{from} -> {to}：{}", transition.reason);
            if let Some(detail) = transition
                .detail
                .as_deref()
                .filter(|detail| !detail.trim().is_empty())
            {
                line.push_str(&format!("；{}", summarize_recovery_detail(detail)));
            }
            line
        })
        .collect()
}

fn summarize_recovery_detail(detail: &str) -> String {
    let normalized = detail.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= 160 {
        normalized
    } else {
        let mut shortened = normalized.chars().take(160).collect::<String>();
        shortened.push('…');
        shortened
    }
}

fn summarize_recovery_items(items: &[String], limit: usize) -> String {
    let mut unique = Vec::new();
    for item in items {
        let item = item.trim();
        if !item.is_empty() && !unique.iter().any(|existing| existing == item) {
            unique.push(item.to_string());
        }
    }

    let visible = unique.iter().take(limit).cloned().collect::<Vec<_>>();
    let suffix = if unique.len() > visible.len() {
        format!("，另有 {} 项", unique.len() - visible.len())
    } else {
        String::new()
    };
    format!("{}{}", visible.join(", "), suffix)
}

fn should_attach_recovery_context(follow_up: &str) -> bool {
    let normalized = follow_up
        .split_whitespace()
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized.is_empty()
        || is_history_recall_request(&normalized)
        || is_non_recovery_continuation_request(&normalized)
    {
        return false;
    }
    contains_any(
        &normalized,
        &[
            "继续",
            "接着",
            "重试",
            "再试",
            "再来一次",
            "重新试",
            "恢复",
            "修一下",
            "修复刚才",
            "解决刚才",
            "刚才的问题",
            "刚刚的问题",
            "上一轮",
            "上次失败",
            "上次报错",
            "这个报错",
            "还是不行",
            "没成功",
            "继续做",
            "继续搞",
            "retry",
            "tryagain",
            "continue",
            "resume",
            "keepgoing",
            "fixprevious",
            "previouserror",
            "lasterror",
        ],
    )
}

fn is_non_recovery_continuation_request(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "继续讨论",
            "继续聊",
            "继续商量",
            "继续规划",
            "继续说",
            "继续想",
            "继续分析",
            "继续看方向",
            "continueourdiscussion",
            "continuediscussing",
        ],
    ) && !contains_any(
        normalized,
        &[
            "报错", "错误", "失败", "问题", "修", "重试", "再试", "retry", "fix", "error", "failed",
        ],
    )
}

fn is_history_recall_request(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "之前说了什么",
            "之前聊了什么",
            "我们之前说",
            "我们之前聊",
            "刚才说了什么",
            "刚才聊了什么",
            "前面说了什么",
            "总结一下对话",
            "总结下对话",
            "回顾一下",
            "whathavewetalked",
            "whatdidwesay",
            "conversationhistory",
        ],
    )
}

fn resolved_failure_kind<'a>(turn: &'a AgentTurnState, failure: &'a AgentFailureTrace) -> &'a str {
    if failure.kind != "unknown" {
        return failure.kind.as_str();
    }
    infer_failure_kind_from_failed_tools(turn).unwrap_or(failure.kind.as_str())
}

fn infer_failure_kind_from_failed_tools(turn: &AgentTurnState) -> Option<&'static str> {
    let evidence_kinds = turn
        .evidence
        .iter()
        .filter(|item| item.outcome == "failed")
        .filter_map(|item| item.failure_kind.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    for known in [
        "auth",
        "rate_limit",
        "context_overflow",
        "workspace_boundary",
        "permission",
        "safety_policy",
        "capability_unavailable",
        "preview_conflict",
        "dependency_install_failed",
        "package_manager_missing",
        "command_not_found",
        "checkpoint_failed",
        "file_not_found",
        "tool_call_mismatch",
        "mcp_timeout",
        "interrupted",
        "verification",
    ] {
        if evidence_kinds.split_whitespace().any(|kind| kind == known) {
            return Some(known);
        }
    }
    let kind = classify_failure_kind(&evidence_kinds);
    if kind != "api" {
        return Some(kind);
    }

    let evidence = turn
        .tools
        .iter()
        .filter(|tool| tool.is_error || tool.status == AgentToolStatus::Failed)
        .flat_map(|tool| [tool.result_summary.as_deref(), tool.command.as_deref()])
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    let kind = classify_failure_kind(&evidence);
    (kind != "api").then_some(kind)
}

pub(crate) fn recovery_advice_for_failure_kind(kind: &str) -> AgentRecoveryAdvice {
    let (action, reason, instruction, safe_to_auto_retry, requires_user_action) = match kind {
        "auth" => (
            "configure_auth",
            "模型密钥或认证不可用",
            "模型密钥或认证不可用，先让用户完成配置；不要反复重试同一请求。",
            false,
            true,
        ),
        "rate_limit" => (
            "backoff_retry",
            "模型服务限流",
            "请求被限流，先等待或降低请求频率；不要立即连续重试。",
            true,
            false,
        ),
        "context_overflow" => (
            "compact_context",
            "上下文超过模型限制",
            "先减少或压缩上下文，再继续；不要再次发送同样规模的上下文。",
            true,
            false,
        ),
        "workspace_boundary" => (
            "check_workspace_boundary",
            "请求触碰了工作空间边界",
            "先重新确认当前工作空间和目标路径；不要访问或修改工作空间外文件。",
            false,
            false,
        ),
        "permission" => (
            "request_permission",
            "本地权限或确认被拒绝",
            "先说明权限被拒绝的位置和动作，再请求用户确认或改用安全路径。",
            false,
            true,
        ),
        "safety_policy" => (
            "explain_safety_block",
            "安全规则阻止了这次操作",
            "先说明安全规则拦截的原因；不要绕过安全规则，改用不包含敏感内容或不越界的安全方案。",
            false,
            true,
        ),
        "capability_unavailable" => (
            "check_capability_status",
            "能力、连接或工具当前不可用",
            "先检查相关能力、连接或工具是否启用；不要假装工具已经执行成功。",
            false,
            true,
        ),
        "preview_conflict" => (
            "check_preview_ownership",
            "预览端口可能属于其他项目",
            "先检查预览端口归属；不要打开或复用其他项目的预览。",
            false,
            false,
        ),
        "dependency_install_failed" => (
            "fix_dependency_install",
            "依赖安装或解析失败",
            "先查看依赖安装输出，优先修复 package 配置或锁文件冲突；不要盲目反复安装。",
            false,
            false,
        ),
        "package_manager_missing" => (
            "select_package_manager",
            "项目需要的包管理器不可用",
            "先检查项目锁文件和可用包管理器，选择本机可执行的命令；不要假装依赖已经安装。",
            false,
            false,
        ),
        "command_not_found" => (
            "check_toolchain_command",
            "本机缺少要执行的命令",
            "先确认命令是否存在或项目是否提供替代脚本；必要时说明需要用户安装工具链。",
            false,
            true,
        ),
        "checkpoint_failed" => (
            "inspect_checkpoint_state",
            "检查点创建或恢复失败",
            "先检查 Git 状态和检查点文件，再决定重新创建检查点或提示用户手动处理。",
            false,
            false,
        ),
        "file_not_found" => (
            "locate_file_reference",
            "引用的文件不存在或路径不对",
            "先在当前工作空间内重新搜索目标文件；不要根据不存在的路径继续修改。",
            false,
            false,
        ),
        "tool_call_mismatch" => (
            "repair_tool_result_flow",
            "工具调用结果和模型期望不匹配",
            "先重新检查上一轮工具调用和结果映射，缺失结果时补充查询，不要编造工具输出。",
            true,
            false,
        ),
        "mcp_timeout" => (
            "retry_or_disable_connector",
            "连接工具响应超时",
            "先尝试一次轻量重试；如果仍超时，暂时跳过该连接并说明影响范围。",
            true,
            false,
        ),
        "interrupted" => (
            "inspect_interrupted_state",
            "上一轮任务被中断",
            "先检查上一轮正在执行的工具和当前工作空间状态；不要假设长命令已完成。",
            false,
            false,
        ),
        "verification" => (
            "review_verification_output",
            "验证命令没有通过",
            "先阅读验证命令和输出，定位失败原因；验证输出是恢复依据，不要直接宣称任务完成。",
            false,
            false,
        ),
        _ => (
            "inspect_state",
            "失败类别不明确",
            "先检查当前工作空间状态和上一轮证据，再决定是否重试。",
            false,
            false,
        ),
    };

    AgentRecoveryAdvice {
        action: action.to_string(),
        reason: reason.to_string(),
        instruction: instruction.to_string(),
        safe_to_auto_retry,
        requires_user_action,
    }
}

fn format_verification_failure_message(trace: &AgentVerificationTrace) -> String {
    let mut parts = Vec::new();
    if let Some(command) = trace
        .command
        .as_deref()
        .filter(|command| !command.trim().is_empty())
    {
        parts.push(format!("Verification command failed: {command}"));
    } else {
        parts.push("Verification failed".to_string());
    }
    if let Some(exit_code) = trace.exit_code {
        parts.push(format!("exit code {exit_code}"));
    }
    if let Some(stderr) = trace
        .stderr_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(stderr.to_string());
    } else if let Some(stdout) = trace
        .stdout_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(stdout.to_string());
    }
    parts.join(" · ")
}

fn summarize_failure_message(message: &str) -> String {
    let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= 240 {
        return normalized;
    }

    let mut summary = normalized.chars().take(240).collect::<String>();
    summary.push('…');
    summary
}

#[cfg(test)]
mod tests {
    use super::{
        api_failure_trace, build_recovery_context, recovery_advice_for_failure_kind,
        verification_failure_trace,
    };
    use crate::agent::turn_state::{
        AgentEvidenceKind, AgentFailureTrace, AgentToolCategory, AgentToolEvidence,
        AgentToolStatus, AgentToolTrace, AgentTurnState, AgentTurnStatus, AgentVerificationStatus,
        AgentVerificationTrace,
    };

    fn verification(status: AgentVerificationStatus) -> AgentVerificationTrace {
        AgentVerificationTrace {
            status,
            command: Some("npm run build".to_string()),
            exit_code: Some(1),
            stdout_preview: None,
            stderr_preview: Some("build failed".to_string()),
            duration_ms: Some(10),
            completed_at_ms: Some(20),
        }
    }

    fn failed_turn(stage: &str, retryable: bool) -> AgentTurnState {
        failed_turn_with_kind("api", stage, retryable)
    }

    fn failed_turn_with_kind(kind: &str, stage: &str, retryable: bool) -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace/demo".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "workflow".to_string(),
            "implementation".to_string(),
            "做一个喝水记录工具".to_string(),
        );
        turn.record_failure(AgentFailureTrace {
            kind: kind.to_string(),
            stage: stage.to_string(),
            message: "API error: upstream timed out".to_string(),
            retryable,
            recovery_advice: None,
            created_at_ms: 42,
        });
        turn
    }

    #[test]
    fn api_failure_trace_is_retryable_and_summarized() {
        let trace = api_failure_trace(&format!("API error: {}", "timeout ".repeat(80)));

        assert_eq!(trace.kind, "api");
        assert_eq!(trace.stage, "api_error");
        assert!(trace.retryable);
        assert!(trace.message.starts_with("API error: timeout"));
        assert!(trace.message.len() <= 260);
    }

    #[test]
    fn api_failure_trace_classifies_auth_rate_limit_and_context_overflow() {
        assert_eq!(
            api_failure_trace("No DeepSeek API key configured").kind,
            "auth"
        );
        assert_eq!(
            api_failure_trace("API error: 429 rate limit exceeded").kind,
            "rate_limit"
        );
        assert_eq!(
            api_failure_trace("maximum context length exceeded").kind,
            "context_overflow"
        );
    }

    #[test]
    fn api_failure_trace_attaches_recovery_advice() {
        let auth = api_failure_trace("No DeepSeek API key configured")
            .recovery_advice
            .expect("auth recovery advice");
        assert_eq!(auth.action, "configure_auth");
        assert!(!auth.safe_to_auto_retry);
        assert!(auth.requires_user_action);

        let rate_limit = api_failure_trace("API error: 429 rate limit exceeded")
            .recovery_advice
            .expect("rate limit recovery advice");
        assert_eq!(rate_limit.action, "backoff_retry");
        assert!(rate_limit.safe_to_auto_retry);
        assert!(!rate_limit.requires_user_action);
    }

    #[test]
    fn verification_failure_trace_attaches_output_review_advice() {
        let advice = verification_failure_trace(&verification(AgentVerificationStatus::Failed))
            .recovery_advice
            .expect("verification recovery advice");

        assert_eq!(advice.action, "review_verification_output");
        assert!(!advice.safe_to_auto_retry);
        assert!(!advice.requires_user_action);
        assert!(advice.instruction.contains("验证输出"));
    }

    #[test]
    fn api_failure_trace_classifies_workspace_permission_and_preview_conflicts() {
        assert_eq!(
            api_failure_trace("Tool execution blocked: outside workspace").kind,
            "workspace_boundary"
        );
        assert_eq!(
            api_failure_trace("Permission denied writing file").kind,
            "permission"
        );
        assert_eq!(
            api_failure_trace("Preview port conflict: address already in use").kind,
            "preview_conflict"
        );
    }

    #[test]
    fn api_failure_trace_classifies_safety_and_capability_failures() {
        assert_eq!(
            api_failure_trace("Tool execution blocked by hook: secret-like input").kind,
            "safety_policy"
        );
        assert_eq!(
            api_failure_trace("Tool disabled by capability settings: mcp__demo__write").kind,
            "capability_unavailable"
        );
        assert_eq!(
            api_failure_trace("Unknown MCP tool: mcp__missing__tool").kind,
            "capability_unavailable"
        );
        assert_eq!(
            api_failure_trace("连接工具不可用：mcp__missing__tool。它可能没有启用。").kind,
            "capability_unavailable"
        );
    }

    #[test]
    fn api_failure_trace_classifies_toolchain_checkpoint_file_and_mcp_failures() {
        assert_eq!(
            api_failure_trace("npm install failed: ERESOLVE unable to resolve dependency tree")
                .kind,
            "dependency_install_failed"
        );
        assert_eq!(
            api_failure_trace("command not found: pnpm").kind,
            "package_manager_missing"
        );
        assert_eq!(
            api_failure_trace("command not found: playwright").kind,
            "command_not_found"
        );
        assert_eq!(
            api_failure_trace("git apply failed: patch does not apply").kind,
            "checkpoint_failed"
        );
        assert_eq!(
            api_failure_trace("File not found: src/App.tsx").kind,
            "file_not_found"
        );
        assert_eq!(
            api_failure_trace("Tool result missing: read_file").kind,
            "tool_call_mismatch"
        );
        assert_eq!(
            api_failure_trace("MCP server timed out while listing resources").kind,
            "mcp_timeout"
        );
    }

    #[test]
    fn recovery_advice_covers_extended_failure_matrix() {
        assert_eq!(
            recovery_advice_for_failure_kind("dependency_install_failed").action,
            "fix_dependency_install"
        );
        assert_eq!(
            recovery_advice_for_failure_kind("package_manager_missing").action,
            "select_package_manager"
        );
        assert_eq!(
            recovery_advice_for_failure_kind("checkpoint_failed").action,
            "inspect_checkpoint_state"
        );
        assert_eq!(
            recovery_advice_for_failure_kind("tool_call_mismatch").action,
            "repair_tool_result_flow"
        );
        assert_eq!(
            recovery_advice_for_failure_kind("mcp_timeout").action,
            "retry_or_disable_connector"
        );
    }

    #[test]
    fn verification_failure_trace_prefers_command_and_error_output() {
        let trace = verification_failure_trace(&verification(AgentVerificationStatus::Failed));

        assert_eq!(trace.stage, "verification_failed");
        assert!(!trace.retryable);
        assert!(trace.message.contains("npm run build"));
        assert!(trace.message.contains("build failed"));
    }

    #[test]
    fn recovery_context_summarizes_retryable_failed_turn_for_follow_up() {
        let turn = failed_turn("api_error", true);

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("上一轮任务失败"));
        assert!(context.contains("失败类别：api"));
        assert!(context.contains("失败阶段：api_error"));
        assert!(context.contains("可重试：是"));
        assert!(context.contains("做一个喝水记录工具"));
        assert!(context.contains("/workspace/demo"));
        assert!(context.contains("优先从失败点恢复"));
    }

    #[test]
    fn recovery_context_attaches_for_retry_intent() {
        let turn = failed_turn("api_error", true);

        let context = build_recovery_context(Some(&turn), "再试一次").expect("recovery context");

        assert!(context.contains("上一轮任务失败"));
        assert!(context.contains("当前用户继续请求：再试一次"));
    }

    #[test]
    fn recovery_context_attaches_for_explicit_fix_previous_issue() {
        let turn = failed_turn("api_error", true);

        let context =
            build_recovery_context(Some(&turn), "继续修刚才那个报错").expect("recovery context");

        assert!(context.contains("上一轮任务失败"));
        assert!(context.contains("当前用户继续请求：继续修刚才那个报错"));
    }

    #[test]
    fn recovery_context_does_not_attach_to_new_task_after_failure() {
        let turn = failed_turn("api_error", true);

        assert_eq!(
            build_recovery_context(Some(&turn), "我想做一个记账小工具"),
            None
        );
    }

    #[test]
    fn recovery_context_does_not_attach_to_non_recovery_discussion_continuation() {
        let turn = failed_turn("api_error", true);

        assert_eq!(
            build_recovery_context(Some(&turn), "继续讨论产品方向"),
            None
        );
    }

    #[test]
    fn recovery_context_does_not_attach_to_history_recall_question() {
        let turn = failed_turn("api_error", true);

        assert_eq!(
            build_recovery_context(Some(&turn), "我们之前说了什么"),
            None
        );
    }

    #[test]
    fn recovery_context_routes_auth_failures_to_configuration() {
        let mut turn = failed_turn_with_kind("auth", "api_error", false);
        turn.failure.as_mut().unwrap().recovery_advice =
            Some(recovery_advice_for_failure_kind("auth"));

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：auth"));
        assert!(context.contains("恢复动作：configure_auth"));
        assert!(context.contains("恢复策略：模型密钥或认证不可用"));
        assert!(context.contains("不要反复重试同一请求"));
    }

    #[test]
    fn recovery_context_routes_workspace_failures_to_boundary_check() {
        let turn = failed_turn_with_kind("workspace_boundary", "tool_failed", false);

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：workspace_boundary"));
        assert!(context.contains("恢复策略：先重新确认当前工作空间和目标路径"));
        assert!(context.contains("不要访问或修改工作空间外文件"));
    }

    #[test]
    fn recovery_context_routes_preview_conflicts_to_port_ownership_check() {
        let turn = failed_turn_with_kind("preview_conflict", "tool_failed", true);

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：preview_conflict"));
        assert!(context.contains("恢复策略：先检查预览端口归属"));
        assert!(context.contains("不要打开或复用其他项目的预览"));
    }

    #[test]
    fn recovery_context_routes_hook_blocks_to_safety_policy() {
        let mut turn = failed_turn_with_kind("unknown", "tool_failed", false);
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "write_file".to_string(),
            category: AgentToolCategory::Write,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Tool execution blocked by hook: secret-like content".to_string()),
            is_error: true,
            affected_files: vec!["src/config.rs".to_string()],
            command: None,
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：safety_policy"));
        assert!(context.contains("恢复动作：explain_safety_block"));
        assert!(context.contains("恢复策略：先说明安全规则拦截的原因"));
    }

    #[test]
    fn recovery_context_routes_unknown_mcp_tools_to_capability_check() {
        let mut turn = failed_turn_with_kind("unknown", "tool_failed", false);
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "mcp__missing__tool".to_string(),
            category: AgentToolCategory::Mcp,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Unknown MCP tool: mcp__missing__tool".to_string()),
            is_error: true,
            affected_files: Vec::new(),
            command: None,
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：capability_unavailable"));
        assert!(context.contains("恢复动作：check_capability_status"));
        assert!(context.contains("恢复策略：先检查相关能力、连接或工具是否启用"));
    }

    #[test]
    fn recovery_context_routes_context_overflow_to_compaction() {
        let turn = failed_turn_with_kind("context_overflow", "api_error", true);

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：context_overflow"));
        assert!(context.contains("恢复策略：先减少或压缩上下文"));
        assert!(context.contains("不要再次发送同样规模的上下文"));
    }

    #[test]
    fn recovery_context_routes_rate_limit_to_backoff() {
        let turn = failed_turn_with_kind("rate_limit", "api_error", true);

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：rate_limit"));
        assert!(context.contains("恢复策略：请求被限流"));
        assert!(context.contains("不要立即连续重试"));
    }

    #[test]
    fn recovery_context_routes_verification_to_output_review() {
        let mut turn = failed_turn_with_kind("verification", "verification_failed", false);
        turn.set_verification(verification(AgentVerificationStatus::Failed));

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：verification"));
        assert!(context.contains("恢复策略：先阅读验证命令和输出"));
        assert!(context.contains("不要直接宣称任务完成"));
        assert!(context.contains("最近验证命令：npm run build"));
    }

    #[test]
    fn recovery_context_infers_strategy_from_failed_tool_when_kind_is_unknown() {
        let mut turn = failed_turn_with_kind("unknown", "tool_failed", true);
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "bash".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Exit code: 1 Stderr: address already in use".to_string()),
            is_error: true,
            affected_files: Vec::new(),
            command: Some("npm run dev".to_string()),
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：preview_conflict"));
        assert!(context.contains("恢复策略：先检查预览端口归属"));
    }

    #[test]
    fn recovery_context_omits_completed_turns() {
        let mut turn = failed_turn("api_error", true);
        turn.failure = None;
        turn.mark_status(AgentTurnStatus::Completed);

        assert_eq!(build_recovery_context(Some(&turn), "继续"), None);
    }

    #[test]
    fn recovery_context_summarizes_cancelled_turn_for_manual_resume() {
        let mut turn = failed_turn("api_error", true);
        turn.failure = None;
        turn.mark_status(AgentTurnStatus::Cancelled);

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("上一轮任务被中断"));
        assert!(context.contains("上一轮目标：做一个喝水记录工具"));
        assert!(context.contains("工作空间：/workspace/demo"));
        assert!(context.contains("需要先确认当前工作空间状态"));
    }

    #[test]
    fn recovery_context_includes_interrupted_tool_for_manual_resume() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace/demo".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "workflow".to_string(),
            "implementation".to_string(),
            "安装依赖并继续生成工具".to_string(),
        );
        turn.mark_status_with_reason(
            AgentTurnStatus::RunningTools,
            "tool_calls_requested",
            Some("model requested tool execution"),
        );
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "bash".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Running,
            started_at_ms: 10,
            ended_at_ms: None,
            result_summary: None,
            is_error: false,
            affected_files: Vec::new(),
            command: Some("npm install".to_string()),
        });
        turn.normalize_for_session_resume();

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("上一轮任务被中断"));
        assert!(context.contains("中断类别：interrupted"));
        assert!(context.contains("恢复动作：inspect_interrupted_state"));
        assert!(context.contains("失败证据：bash"));
        assert!(context.contains("命令：npm install"));
        assert!(context.contains("不要假设长命令已完成"));
    }

    #[test]
    fn recovery_context_includes_failed_tool_evidence() {
        let mut turn = failed_turn("tool_failed", true);
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "bash".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Exit code: 1 Stderr: port already in use".to_string()),
            is_error: true,
            affected_files: vec!["package.json".to_string()],
            command: Some("npm run dev".to_string()),
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败证据：bash"));
        assert!(context.contains("命令：npm run dev"));
        assert!(context.contains("port already in use"));
        assert!(context.contains("上一轮涉及文件：package.json"));
    }

    #[test]
    fn recovery_context_prefers_structured_tool_evidence_ledger() {
        let mut turn = failed_turn_with_kind("unknown", "tool_failed", true);
        turn.tools.clear();
        turn.evidence.push(AgentToolEvidence {
            kind: AgentEvidenceKind::Tool,
            evidence_id: "tool:tool-1".to_string(),
            tool_call_id: "tool-1".to_string(),
            tool_name: "bash".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            outcome: "failed".to_string(),
            summary: Some("Exit code: 1 Stderr: address already in use".to_string()),
            command: Some("npm run dev".to_string()),
            affected_files: vec!["package.json".to_string()],
            failure_kind: Some("preview_conflict".to_string()),
            created_at_ms: 20,
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败证据：bash"));
        assert!(context.contains("失败类别：preview_conflict"));
        assert!(context.contains("恢复动作：check_preview_ownership"));
        assert!(context.contains("命令：npm run dev"));
        assert!(context.contains("address already in use"));
        assert!(context.contains("上一轮涉及文件：package.json"));
    }

    #[test]
    fn recovery_context_routes_verification_evidence_to_output_review() {
        let mut turn = failed_turn_with_kind("unknown", "tool_failed", false);
        turn.tools.clear();
        turn.evidence.push(AgentToolEvidence {
            kind: AgentEvidenceKind::Verification,
            evidence_id: "verification:20".to_string(),
            tool_call_id: String::new(),
            tool_name: "verification".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            outcome: "failed".to_string(),
            summary: Some("status=Failed; exit_code=1; stderr=build failed".to_string()),
            command: Some("npm run build".to_string()),
            affected_files: Vec::new(),
            failure_kind: Some("verification".to_string()),
            created_at_ms: 20,
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("失败类别：verification"));
        assert!(context.contains("恢复动作：review_verification_output"));
        assert!(context.contains("失败证据：verification"));
        assert!(context.contains("命令：npm run build"));
    }

    #[test]
    fn recovery_context_includes_unfinished_execution_plan() {
        let mut turn = failed_turn_with_kind("verification", "verification_failed", false);
        turn.set_execution_plan(
            "生成可预览第一版".to_string(),
            vec![
                "确认需求".to_string(),
                "生成页面".to_string(),
                "检查交付".to_string(),
            ],
        );
        turn.update_execution_plan_item(
            "step-1",
            crate::agent::turn_state::AgentPlanItemStatus::Completed,
            None,
            None,
        );
        turn.update_execution_plan_item(
            "step-2",
            crate::agent::turn_state::AgentPlanItemStatus::Failed,
            Some("verification:20".to_string()),
            Some("verification".to_string()),
        );

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("上一轮计划：生成可预览第一版"));
        assert!(context.contains("未完成步骤：生成页面（failed, verification）"));
        assert!(context.contains("检查交付（pending）"));
    }

    #[test]
    fn recovery_context_includes_matched_skill_intent() {
        let mut turn = failed_turn("tool_failed", true);
        turn.input_intent.matched_skills = vec!["fix-flow（触发：排查并修复）".to_string()];

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("上一轮启用技能：fix-flow（触发：排查并修复）"));
    }

    #[test]
    fn recovery_context_includes_capability_environment_for_resume() {
        let mut turn = failed_turn("tool_failed", true);
        turn.input_intent.active_hooks = vec!["Workspace Boundary Guard".to_string()];
        turn.input_intent.enabled_mcp_servers = vec!["obsidian".to_string()];
        turn.input_intent.available_mcp_tools = vec!["mcp__obsidian__search_notes".to_string()];

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("上一轮安全规则：Workspace Boundary Guard"));
        assert!(context.contains("上一轮可用连接：obsidian"));
        assert!(context.contains("上一轮可用连接工具：mcp__obsidian__search_notes"));
    }

    #[test]
    fn recovery_context_includes_recent_runtime_transitions() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace/demo".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "workflow".to_string(),
            "implementation".to_string(),
            "修复构建错误".to_string(),
        );
        turn.mark_status_with_reason(
            AgentTurnStatus::RunningTools,
            "tool_calls_requested",
            Some("model requested tool execution"),
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
        turn.record_failure(AgentFailureTrace {
            kind: "verification".to_string(),
            stage: "verification_failed".to_string(),
            message: "build failed".to_string(),
            retryable: false,
            recovery_advice: None,
            created_at_ms: 42,
        });

        let context = build_recovery_context(Some(&turn), "继续").expect("recovery context");

        assert!(context.contains("最近运行轨迹"));
        assert!(context.contains("tool_calls_requested"));
        assert!(context.contains("tool_failed"));
        assert!(context.contains("failure"));
        assert!(context.contains("npm run build"));
    }
}
