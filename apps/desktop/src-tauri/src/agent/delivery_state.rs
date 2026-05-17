use crate::agent::turn_state::{AgentVerificationStatus, AgentVerificationTrace};
use crate::protocol::events::DeliverySummary;

pub(crate) struct DeliveryRuntimeInput {
    pub project_path: Option<String>,
    pub running: bool,
    pub can_start: bool,
    pub can_open: bool,
}

pub(crate) struct DeliveryCheckpointInput {
    pub is_git_repo: bool,
    pub dirty: bool,
    pub has_checkpoint: bool,
}

pub(crate) struct DeliveryRecordInput {
    pub status: String,
    pub target_pages: Vec<String>,
}

pub(crate) fn build_delivery_summary(
    runtime: Option<DeliveryRuntimeInput>,
    checkpoint: Option<DeliveryCheckpointInput>,
    verification: Option<&AgentVerificationTrace>,
    record: Option<DeliveryRecordInput>,
) -> DeliverySummary {
    let preview_label = match runtime.as_ref() {
        Some(runtime) if runtime.running => "预览运行中",
        Some(_) => "预览未运行",
        None => "预览状态未知",
    }
    .to_string();

    let checkpoint_label = match checkpoint.as_ref() {
        Some(checkpoint) if checkpoint.has_checkpoint && checkpoint.dirty => {
            "已有检查点，当前有改动"
        }
        Some(checkpoint) if checkpoint.has_checkpoint => "检查点已就绪",
        Some(checkpoint) if checkpoint.is_git_repo => "还没有检查点",
        Some(_) => "当前不是 Git 项目",
        None => "检查点状态未知",
    }
    .to_string();

    let verification_label = verification.and_then(|trace| match trace.status {
        AgentVerificationStatus::Passed => Some("检查已通过".to_string()),
        AgentVerificationStatus::Failed => Some("检查未通过".to_string()),
        AgentVerificationStatus::Error => Some("检查出错".to_string()),
        AgentVerificationStatus::Skipped => Some("检查已跳过".to_string()),
        AgentVerificationStatus::NotNeeded | AgentVerificationStatus::Running => None,
    });
    let verification_status = verification_label
        .as_ref()
        .and_then(|_| verification.map(|trace| verification_status_value(&trace.status)));
    let verification_command = verification_label
        .as_ref()
        .and_then(|_| verification.and_then(|trace| trace.command.clone()));

    let needs_preview = runtime
        .as_ref()
        .is_some_and(|runtime| !runtime.running && runtime.can_start);
    let needs_checkpoint = checkpoint
        .as_ref()
        .is_some_and(|checkpoint| checkpoint.is_git_repo && !checkpoint.has_checkpoint);

    let next_action = match verification.map(|trace| &trace.status) {
        Some(AgentVerificationStatus::Failed) => "下一步：先修复检查未通过的问题。",
        Some(AgentVerificationStatus::Error) => "下一步：先处理检查出错的问题。",
        _ if needs_preview && needs_checkpoint => "下一步：启动预览，并创建检查点。",
        _ if needs_preview => "下一步：启动预览。",
        _ if needs_checkpoint => "下一步：创建检查点。",
        _ => "下一步：交付状态可以继续验收。",
    }
    .to_string();

    DeliverySummary {
        project_path: runtime.and_then(|runtime| runtime.project_path),
        preview_label,
        checkpoint_label,
        next_action,
        verification_label,
        verification_status,
        verification_command,
        record_label: record
            .as_ref()
            .filter(|record| record.status == "pending")
            .map(|_| "建议更新项目记录".to_string()),
        record_status: record.as_ref().map(|record| record.status.clone()),
        record_target_pages: record.map(|record| record.target_pages).unwrap_or_default(),
    }
}

fn verification_status_value(status: &AgentVerificationStatus) -> String {
    match status {
        AgentVerificationStatus::NotNeeded => "not_needed",
        AgentVerificationStatus::Skipped => "skipped",
        AgentVerificationStatus::Running => "running",
        AgentVerificationStatus::Passed => "passed",
        AgentVerificationStatus::Failed => "failed",
        AgentVerificationStatus::Error => "error",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use crate::agent::delivery_state::{
        build_delivery_summary, DeliveryCheckpointInput, DeliveryRecordInput, DeliveryRuntimeInput,
    };
    use crate::agent::turn_state::{AgentVerificationStatus, AgentVerificationTrace};

    fn verification(status: AgentVerificationStatus) -> AgentVerificationTrace {
        AgentVerificationTrace {
            status,
            command: Some("npm run build".to_string()),
            exit_code: Some(0),
            stdout_preview: None,
            stderr_preview: None,
            duration_ms: Some(100),
            completed_at_ms: Some(200),
        }
    }

    #[test]
    fn running_preview_ready_checkpoint_and_passed_verification_are_ready() {
        let summary = build_delivery_summary(
            Some(DeliveryRuntimeInput {
                project_path: Some("/workspace".to_string()),
                running: true,
                can_start: false,
                can_open: true,
            }),
            Some(DeliveryCheckpointInput {
                is_git_repo: true,
                dirty: false,
                has_checkpoint: true,
            }),
            Some(&verification(AgentVerificationStatus::Passed)),
            None,
        );

        assert_eq!(summary.preview_label, "预览运行中");
        assert_eq!(summary.checkpoint_label, "检查点已就绪");
        assert_eq!(summary.verification_label.as_deref(), Some("检查已通过"));
        assert_eq!(summary.next_action, "下一步：交付状态可以继续验收。");
    }

    #[test]
    fn stopped_preview_missing_checkpoint_needs_both_actions() {
        let summary = build_delivery_summary(
            Some(DeliveryRuntimeInput {
                project_path: Some("/workspace".to_string()),
                running: false,
                can_start: true,
                can_open: false,
            }),
            Some(DeliveryCheckpointInput {
                is_git_repo: true,
                dirty: true,
                has_checkpoint: false,
            }),
            None,
            None,
        );

        assert_eq!(summary.preview_label, "预览未运行");
        assert_eq!(summary.checkpoint_label, "还没有检查点");
        assert_eq!(summary.next_action, "下一步：启动预览，并创建检查点。");
    }

    #[test]
    fn failed_verification_takes_next_action_priority() {
        let summary = build_delivery_summary(
            Some(DeliveryRuntimeInput {
                project_path: Some("/workspace".to_string()),
                running: false,
                can_start: true,
                can_open: false,
            }),
            Some(DeliveryCheckpointInput {
                is_git_repo: true,
                dirty: true,
                has_checkpoint: false,
            }),
            Some(&verification(AgentVerificationStatus::Failed)),
            None,
        );

        assert_eq!(summary.verification_label.as_deref(), Some("检查未通过"));
        assert_eq!(summary.next_action, "下一步：先修复检查未通过的问题。");
    }

    #[test]
    fn pending_record_evidence_sets_record_fields() {
        let summary = build_delivery_summary(
            None,
            None,
            None,
            Some(DeliveryRecordInput {
                status: "pending".to_string(),
                target_pages: vec!["tasks.md".to_string(), "log.md".to_string()],
            }),
        );

        assert_eq!(summary.record_label.as_deref(), Some("建议更新项目记录"));
        assert_eq!(summary.record_status.as_deref(), Some("pending"));
        assert_eq!(summary.record_target_pages, vec!["tasks.md", "log.md"]);
    }

    #[test]
    fn record_fields_are_empty_without_record_evidence() {
        let summary = build_delivery_summary(None, None, None, None);

        assert_eq!(summary.record_label, None);
        assert_eq!(summary.record_status, None);
        assert!(summary.record_target_pages.is_empty());
    }
}
