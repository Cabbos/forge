use std::collections::BTreeSet;

use crate::agent::turn_state::{AgentTurnState, AgentTurnStatus, AgentVerificationStatus};
use crate::workflow::WorkflowState;

pub(crate) struct ProjectArchiveWriteback {
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) target_pages: Vec<String>,
}

pub(crate) fn build_project_archive_writeback(
    workflow: &WorkflowState,
    user_text: &str,
    latest_turn: Option<&AgentTurnState>,
) -> Option<ProjectArchiveWriteback> {
    let user_text = user_text.trim();
    if user_text.is_empty() {
        return None;
    }

    let needs_tasks = targets_tasks(user_text, latest_turn);
    let needs_decisions = targets_decisions(user_text);
    let needs_sources = contains_source_language(user_text);
    let needs_log = targets_log(user_text, latest_turn);

    let mut target_pages = Vec::new();
    if needs_tasks {
        target_pages.push("tasks.md".to_string());
    }
    if needs_decisions {
        target_pages.push("decisions.md".to_string());
    }
    if needs_sources {
        target_pages.push("sources.md".to_string());
    }
    if target_pages.is_empty() || needs_log {
        target_pages.push("log.md".to_string());
    }
    if target_pages.len() > 3 && needs_log {
        target_pages.retain(|page| page != "sources.md");
    }
    target_pages.truncate(3);

    Some(ProjectArchiveWriteback {
        title: truncate_chars("记录本轮工作", 160),
        summary: truncate_chars(&build_summary(workflow, user_text, latest_turn), 900),
        target_pages,
    })
}

fn targets_tasks(user_text: &str, latest_turn: Option<&AgentTurnState>) -> bool {
    let lower = user_text.to_lowercase();
    if contains_any(
        user_text,
        &["继续", "下一步", "下步", "计划", "阻塞", "卡住"],
    ) || contains_ascii_word(&lower, "continue")
        || lower.contains("next step")
        || contains_ascii_word(&lower, "plan")
        || contains_ascii_word(&lower, "blocker")
        || contains_ascii_word(&lower, "blockers")
    {
        return true;
    }

    latest_turn.is_some_and(|turn| {
        matches!(
            turn.status,
            AgentTurnStatus::Failed | AgentTurnStatus::Cancelled
        ) || matches!(
            turn.verification.status,
            AgentVerificationStatus::Failed | AgentVerificationStatus::Error
        )
    })
}

fn targets_decisions(user_text: &str) -> bool {
    let lower = user_text.to_lowercase();
    contains_any(
        user_text,
        &["就按照", "以后", "标准", "统一", "原则", "方向"],
    ) || (lower.contains("确定") && !lower.contains("不确定"))
}

fn targets_log(user_text: &str, latest_turn: Option<&AgentTurnState>) -> bool {
    has_build_check_test_wording(user_text)
        || latest_turn.is_some_and(|turn| {
            !affected_files_summary(turn).is_empty()
                || !matches!(turn.verification.status, AgentVerificationStatus::NotNeeded)
                || turn.verification.command.is_some()
        })
}

fn has_build_check_test_wording(user_text: &str) -> bool {
    contains_any(
        user_text,
        &[
            "build", "check", "test", "verify", "构建", "检查", "测试", "验证",
        ],
    )
}

fn build_summary(
    workflow: &WorkflowState,
    user_text: &str,
    latest_turn: Option<&AgentTurnState>,
) -> String {
    let mut lines = vec![
        format!(
            "当前任务：{}",
            truncate_chars(&sanitize_product_terms(&workflow.beginner_label), 80)
        ),
        format!(
            "用户目标：{}",
            truncate_chars(&sanitize_product_terms(user_text), 240)
        ),
    ];

    if let Some(turn) = latest_turn {
        lines.push(format!("本轮状态：{}", turn_status_label(&turn.status)));
        let affected_files = affected_files_summary(turn);
        if !affected_files.is_empty() {
            lines.push(format!("变更文件：{}", affected_files));
        }
        lines.push(format!(
            "检查结果：{}",
            verification_status_label(&turn.verification.status)
        ));
        if let Some(command) = turn.verification.command.as_deref() {
            lines.push(format!("检查命令：{}", truncate_chars(command, 120)));
        }
    } else {
        lines.push("本轮状态：已记录".to_string());
        lines.push("检查结果：未记录".to_string());
    }

    lines.join("\n")
}

fn affected_files_summary(turn: &AgentTurnState) -> String {
    let mut files = BTreeSet::new();
    for tool in &turn.tools {
        for file in &tool.affected_files {
            if !file.trim().is_empty() {
                files.insert(file.trim().to_string());
            }
        }
    }
    files.into_iter().take(6).collect::<Vec<_>>().join("、")
}

fn turn_status_label(status: &AgentTurnStatus) -> &'static str {
    match status {
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

fn verification_status_label(status: &AgentVerificationStatus) -> &'static str {
    match status {
        AgentVerificationStatus::NotNeeded => "无需检查",
        AgentVerificationStatus::Skipped => "已跳过",
        AgentVerificationStatus::Running => "检查中",
        AgentVerificationStatus::Passed => "已通过",
        AgentVerificationStatus::Failed => "未通过",
        AgentVerificationStatus::Error => "检查出错",
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn contains_source_language(value: &str) -> bool {
    let lower = value.to_lowercase();
    ["文档", "链接"].iter().any(|needle| lower.contains(needle))
        || lower.contains("资料")
        || contains_ascii_word(&lower, "pdf")
        || contains_ascii_word(&lower, "excel")
        || contains_ascii_word(&lower, "ppt")
        || lower.contains(".doc")
        || lower.contains("docx")
        || lower.contains("word 文档")
        || lower.contains("word file")
        || lower.contains("word document")
        || lower.contains("word 资料")
}

fn contains_ascii_word(value: &str, needle: &str) -> bool {
    value.match_indices(needle).any(|(index, _)| {
        let before = value[..index].chars().next_back();
        let after = value[index + needle.len()..].chars().next();
        !before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn sanitize_product_terms(value: &str) -> String {
    let mut sanitized = value.to_string();
    for (needle, replacement) in [
        ("Workflow Router", "项目流程"),
        ("Task Mode", "任务模式"),
        ("Forge Wiki", "项目档案"),
        ("Living Wiki", "项目记录"),
        ("Context Activation", "本轮参考"),
        ("Selected Context", "本轮参考"),
        ("Memory", "项目记录"),
    ] {
        sanitized = replace_ascii_phrase_case_insensitive(&sanitized, needle, replacement);
    }
    sanitized.replace("上下文工程", "项目档案整理")
}

fn replace_ascii_phrase_case_insensitive(value: &str, needle: &str, replacement: &str) -> String {
    let mut output = value.to_string();
    let mut lower = value.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let mut search_start = 0;

    while let Some(relative_index) = lower[search_start..].find(&needle_lower) {
        let index = search_start + relative_index;
        let end = index + needle.len();
        output.replace_range(index..end, replacement);
        lower.replace_range(index..end, &replacement.to_ascii_lowercase());
        search_start = index + replacement.len();
    }

    output
}

#[cfg(test)]
mod tests {
    use crate::agent::turn_state::{
        AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentTurnStatus,
        AgentVerificationStatus, AgentVerificationTrace,
    };
    use crate::forge_wiki::writeback::build_project_archive_writeback;
    use crate::workflow::classify_workflow;

    fn tool_trace(path: &str) -> AgentToolTrace {
        AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "write_file".to_string(),
            category: AgentToolCategory::Write,
            status: AgentToolStatus::Completed,
            started_at_ms: 1,
            ended_at_ms: Some(2),
            result_summary: Some("wrote file".to_string()),
            is_error: false,
            affected_files: vec![path.to_string()],
            command: None,
        }
    }

    fn turn_state(
        status: AgentTurnStatus,
        verification_status: AgentVerificationStatus,
    ) -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-pro".to_string(),
            "workflow".to_string(),
            "making".to_string(),
            "做一个报价工具".to_string(),
        );
        turn.record_tool(tool_trace("src/App.tsx"));
        turn.set_verification(AgentVerificationTrace {
            status: verification_status,
            command: Some("npm run build".to_string()),
            exit_code: Some(0),
            stdout_preview: None,
            stderr_preview: None,
            duration_ms: Some(100),
            completed_at_ms: Some(200),
        });
        turn.mark_status(status);
        turn
    }

    #[test]
    fn completed_write_with_verification_targets_log() {
        let workflow =
            classify_workflow("session-1", "帮我做一个报价工具，改完后检查是否能构建", 10);
        let turn = turn_state(AgentTurnStatus::Completed, AgentVerificationStatus::Passed);

        let writeback = build_project_archive_writeback(
            &workflow,
            "帮我做一个报价工具，改完后检查是否能构建",
            Some(&turn),
        )
        .expect("writeback");

        assert_eq!(writeback.target_pages, vec!["log.md"]);
    }

    #[test]
    fn failed_turn_targets_tasks_and_log() {
        let workflow = classify_workflow("session-1", "修复构建失败", 10);
        let turn = turn_state(AgentTurnStatus::Failed, AgentVerificationStatus::Failed);

        let writeback = build_project_archive_writeback(&workflow, "修复构建失败", Some(&turn))
            .expect("writeback");

        assert_eq!(writeback.target_pages, vec!["tasks.md", "log.md"]);
    }

    #[test]
    fn decision_language_targets_decisions() {
        let workflow = classify_workflow("session-1", "以后设置入口统一放在右上角", 10);

        let writeback =
            build_project_archive_writeback(&workflow, "以后设置入口统一放在右上角", None)
                .expect("writeback");

        assert!(writeback.target_pages.contains(&"decisions.md".to_string()));
    }

    #[test]
    fn source_language_targets_sources() {
        let workflow = classify_workflow("session-1", "把这个 PDF 资料加入项目档案", 10);

        let writeback =
            build_project_archive_writeback(&workflow, "把这个 PDF 资料加入项目档案", None)
                .expect("writeback");

        assert!(writeback.target_pages.contains(&"sources.md".to_string()));
    }

    #[test]
    fn project_archive_language_does_not_target_sources_by_itself() {
        let workflow = classify_workflow("session-1", "打开项目档案继续整理", 10);

        let writeback = build_project_archive_writeback(&workflow, "打开项目档案继续整理", None)
            .expect("writeback");

        assert!(!writeback.target_pages.contains(&"sources.md".to_string()));
    }

    #[test]
    fn summary_uses_product_language_and_turn_evidence() {
        let workflow = classify_workflow("session-1", "帮我做一个报价工具", 10);
        let turn = turn_state(AgentTurnStatus::Completed, AgentVerificationStatus::Passed);

        let writeback =
            build_project_archive_writeback(&workflow, "帮我做一个报价工具", Some(&turn))
                .expect("writeback");

        assert!(writeback.summary.contains("当前任务："));
        assert!(writeback.summary.contains("用户目标：帮我做一个报价工具"));
        assert!(writeback.summary.contains("本轮状态：已完成"));
        assert!(writeback.summary.contains("变更文件：src/App.tsx"));
        assert!(writeback.summary.contains("检查结果：已通过"));
        assert!(writeback.summary.contains("检查命令：npm run build"));
        assert!(!writeback.summary.contains("Workflow Router"));
        assert!(!writeback.summary.contains("Task Mode"));
        assert!(!writeback.summary.contains("Forge Wiki"));
        assert!(!writeback.summary.contains("Memory"));
        assert!(!writeback.summary.contains("上下文工程"));
    }

    #[test]
    fn target_pages_keep_stable_order_and_cap_to_three() {
        let workflow = classify_workflow("session-1", "以后把 PDF 资料统一整理，继续检查构建", 10);
        let turn = turn_state(AgentTurnStatus::Failed, AgentVerificationStatus::Failed);

        let writeback = build_project_archive_writeback(
            &workflow,
            "以后把 PDF 资料统一整理，继续检查构建",
            Some(&turn),
        )
        .expect("writeback");

        assert_eq!(
            writeback.target_pages,
            vec!["tasks.md", "decisions.md", "log.md"]
        );
    }

    #[test]
    fn log_is_retained_under_cap_when_log_evidence_exists() {
        let workflow = classify_workflow("session-1", "以后把 PDF 资料统一整理，继续检查构建", 10);
        let turn = turn_state(AgentTurnStatus::Failed, AgentVerificationStatus::Failed);

        let writeback = build_project_archive_writeback(
            &workflow,
            "以后把 PDF 资料统一整理，继续检查构建",
            Some(&turn),
        )
        .expect("writeback");

        assert_eq!(
            writeback.target_pages,
            vec!["tasks.md", "decisions.md", "log.md"]
        );
    }

    #[test]
    fn explain_does_not_target_tasks_as_plan_language() {
        let workflow = classify_workflow("session-1", "please explain the current design", 10);

        let writeback =
            build_project_archive_writeback(&workflow, "please explain the current design", None)
                .expect("writeback");

        assert!(!writeback.target_pages.contains(&"tasks.md".to_string()));
    }

    #[test]
    fn uncertain_chinese_does_not_target_decisions() {
        let workflow = classify_workflow("session-1", "我还不确定这个入口放哪里", 10);

        let writeback =
            build_project_archive_writeback(&workflow, "我还不确定这个入口放哪里", None)
                .expect("writeback");

        assert!(!writeback.target_pages.contains(&"decisions.md".to_string()));
    }

    #[test]
    fn unrelated_english_word_usage_does_not_target_sources() {
        let workflow =
            classify_workflow("session-1", "the password field wording is confusing", 10);

        let writeback = build_project_archive_writeback(
            &workflow,
            "the password field wording is confusing",
            None,
        )
        .expect("writeback");

        assert!(!writeback.target_pages.contains(&"sources.md".to_string()));
    }

    #[test]
    fn empty_user_text_returns_none() {
        let workflow = classify_workflow("session-1", "", 10);

        assert!(build_project_archive_writeback(&workflow, "   ", None).is_none());
    }

    #[test]
    fn summary_sanitizes_internal_terms_from_inputs() {
        let workflow = classify_workflow("session-1", "更新 Forge Wiki Memory", 10);

        let writeback = build_project_archive_writeback(
            &workflow,
            "按 Workflow Router 和 Task Mode 记录 Forge Wiki Memory，不透出上下文工程",
            None,
        )
        .expect("writeback");

        assert!(!writeback.summary.contains("Workflow Router"));
        assert!(!writeback.summary.contains("Task Mode"));
        assert!(!writeback.summary.contains("Forge Wiki"));
        assert!(!writeback.summary.contains("Memory"));
        assert!(!writeback.summary.contains("上下文工程"));
    }

    #[test]
    fn summary_sanitizes_common_internal_term_casing_variants() {
        let workflow = classify_workflow("session-1", "sync workflow router and living wiki", 10);

        let writeback = build_project_archive_writeback(
            &workflow,
            "workflow router, task mode, forge wiki, living wiki, context activation, selected context",
            None,
        )
        .expect("writeback");

        for internal in [
            "workflow router",
            "task mode",
            "forge wiki",
            "living wiki",
            "context activation",
            "selected context",
        ] {
            assert!(
                !writeback.summary.to_lowercase().contains(internal),
                "summary should not expose {internal}: {}",
                writeback.summary
            );
        }
        assert!(writeback.summary.contains("项目档案"));
        assert!(writeback.summary.contains("本轮参考"));
    }
}
