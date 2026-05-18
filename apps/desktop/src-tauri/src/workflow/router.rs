use super::model::{
    WorkflowGate, WorkflowOverrideAction, WorkflowPhase, WorkflowRoute, WorkflowState,
};

pub fn classify_workflow(session_id: &str, text: &str, updated_at: u64) -> WorkflowState {
    let signal_groups = [
        (WorkflowRoute::Direct, DIRECT_SIGNALS),
        (WorkflowRoute::StrictWorkflow, STRICT_WORKFLOW_SIGNALS),
        (WorkflowRoute::Recovery, RECOVERY_SIGNALS),
        (WorkflowRoute::Verification, VERIFICATION_SIGNALS),
        (WorkflowRoute::Workflow, WORKFLOW_SIGNALS),
        (WorkflowRoute::Light, LIGHT_SIGNALS),
    ];

    for (route, signals) in signal_groups {
        let matches = matched_signals(text, signals);
        if !matches.is_empty() {
            return workflow_state(session_id, route, matches, None, updated_at);
        }
    }

    workflow_state(
        session_id,
        WorkflowRoute::Direct,
        Vec::new(),
        None,
        updated_at,
    )
}

pub fn workflow_state_from_override(
    session_id: &str,
    action: WorkflowOverrideAction,
    updated_at: u64,
) -> WorkflowState {
    let route = match action {
        WorkflowOverrideAction::Direct => WorkflowRoute::Direct,
        WorkflowOverrideAction::PlanFirst => WorkflowRoute::Workflow,
        WorkflowOverrideAction::Debug => WorkflowRoute::Recovery,
        WorkflowOverrideAction::Verify => WorkflowRoute::Verification,
    };

    workflow_state(
        session_id,
        route,
        Vec::new(),
        Some("用户手动切换了当前工作方式。"),
        updated_at,
    )
}

fn workflow_state(
    session_id: &str,
    route: WorkflowRoute,
    matched_signals: Vec<String>,
    reason_override: Option<&str>,
    updated_at: u64,
) -> WorkflowState {
    let metadata = route_metadata(&route);

    WorkflowState {
        session_id: session_id.to_string(),
        route,
        phase: metadata.phase,
        beginner_label: metadata.beginner_label.to_string(),
        developer_label: metadata.developer_label.to_string(),
        matched_signals,
        reason: reason_override.unwrap_or(metadata.reason).to_string(),
        gate: metadata.gate,
        override_actions: override_actions(),
        spec_path: None,
        plan_path: None,
        checkpoint_id: None,
        updated_at,
    }
}

fn matched_signals(text: &str, signals: &[&str]) -> Vec<String> {
    let text_lower = text.to_lowercase();

    signals
        .iter()
        .filter(|signal| text_lower.contains(&signal.to_lowercase()))
        .map(|signal| (*signal).to_string())
        .collect()
}

fn override_actions() -> Vec<WorkflowOverrideAction> {
    vec![
        WorkflowOverrideAction::Direct,
        WorkflowOverrideAction::PlanFirst,
        WorkflowOverrideAction::Debug,
        WorkflowOverrideAction::Verify,
    ]
}

struct RouteMetadata {
    phase: WorkflowPhase,
    beginner_label: &'static str,
    developer_label: &'static str,
    reason: &'static str,
    gate: WorkflowGate,
}

fn route_metadata(route: &WorkflowRoute) -> RouteMetadata {
    match route {
        WorkflowRoute::Direct => RouteMetadata {
            phase: WorkflowPhase::Idle,
            beginner_label: "直接回答",
            developer_label: "direct",
            reason: "这是一个回答型请求，不需要改动项目。",
            gate: WorkflowGate::None,
        },
        WorkflowRoute::Light => RouteMetadata {
            phase: WorkflowPhase::Executing,
            beginner_label: "小改动，直接处理",
            developer_label: "light",
            reason: "这个请求范围较小，可以直接进入处理。",
            gate: WorkflowGate::None,
        },
        WorkflowRoute::Workflow => RouteMetadata {
            phase: WorkflowPhase::Clarifying,
            beginner_label: "先梳理想法",
            developer_label: "workflow",
            reason: "这个需求会影响多个部分，先拆清楚方案会更稳。",
            gate: WorkflowGate::Soft,
        },
        WorkflowRoute::StrictWorkflow => RouteMetadata {
            phase: WorkflowPhase::Planning,
            beginner_label: "必须先确认方案",
            developer_label: "strict_workflow",
            reason: "这个请求涉及高风险改动，需要先确认方案和步骤。",
            gate: WorkflowGate::ApprovalRequired,
        },
        WorkflowRoute::Recovery => RouteMetadata {
            phase: WorkflowPhase::Debugging,
            beginner_label: "遇到问题，正在排查",
            developer_label: "recovery",
            reason: "请求里出现了失败或异常信号，应该先定位问题。",
            gate: WorkflowGate::None,
        },
        WorkflowRoute::Verification => RouteMetadata {
            phase: WorkflowPhase::Verifying,
            beginner_label: "正在检查结果",
            developer_label: "verification",
            reason: "用户正在要求检查或验收已有结果。",
            gate: WorkflowGate::None,
        },
    }
}

const DIRECT_SIGNALS: &[&str] = &[
    "不要修改文件",
    "不要执行命令",
    "只回答",
    "仅回答",
    "解释一下",
    "是什么",
    "answer only",
    "do not edit",
    "don't edit",
    "no changes",
    "explain",
];

const RECOVERY_SIGNALS: &[&str] = &[
    "失败",
    "报错",
    "异常",
    "挂掉",
    "崩溃",
    "不能运行",
    "修复",
    "debug",
    "failed",
    "failure",
    "error",
    "crash",
    "broken",
];

const VERIFICATION_SIGNALS: &[&str] = &[
    "验收",
    "检查结果",
    "检查一下",
    "验证",
    "确认是否",
    "review",
    "verify",
    "validate",
    "check this",
    "test the change",
];

const STRICT_WORKFLOW_SIGNALS: &[&str] = &[
    "删除",
    "迁移",
    "重构整个",
    "破坏性",
    "高风险",
    "权限",
    "认证",
    "数据格式",
    "drop",
    "delete",
    "migration",
    "migrate",
    "destructive",
    "schema",
    "auth",
];

const WORKFLOW_SIGNALS: &[&str] = &[
    "我想做",
    "我想做个",
    "不知道怎么说",
    "不知道怎么概括",
    "帮我想想",
    "能力",
    "支持",
    "新增",
    "实现一个",
    "完整",
    "多个部分",
    "feature",
    "capability",
    "support",
    "implement",
    "add a",
    "build a",
];

const LIGHT_SIGNALS: &[&str] = &[
    "文案",
    "标题",
    "改成",
    "按钮",
    "颜色",
    "样式",
    "copy",
    "label",
    "title",
    "rename",
    "change text",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn route(text: &str) -> WorkflowState {
        classify_workflow("session-1", text, 42)
    }

    #[test]
    fn classifies_answer_only_request_as_direct() {
        let state = route("不要修改文件，不要执行命令。只回答：working-method routing 是什么？");
        assert_eq!(state.route, WorkflowRoute::Direct);
        assert_eq!(state.gate, WorkflowGate::None);
        assert!(state
            .matched_signals
            .iter()
            .any(|s| s.contains("不要修改文件")));
    }

    #[test]
    fn classifies_low_risk_copy_change_as_light() {
        let state = route("把右侧标题文案改成资料");
        assert_eq!(state.route, WorkflowRoute::Light);
        assert_eq!(state.phase, WorkflowPhase::Executing);
        assert_eq!(state.gate, WorkflowGate::None);
    }

    #[test]
    fn classifies_product_feature_as_workflow() {
        let state = route("我想做一个资料分析能力，支持 PDF 和 Word");
        assert_eq!(state.route, WorkflowRoute::Workflow);
        assert_eq!(state.phase, WorkflowPhase::Clarifying);
        assert_eq!(state.gate, WorkflowGate::Soft);
    }

    #[test]
    fn classifies_vague_beginner_idea_as_workflow() {
        let state = route("我想做个能记录客户的东西，最好能提醒我，还能导出表格，但我也不知道怎么说。");
        assert_eq!(state.route, WorkflowRoute::Workflow);
        assert_eq!(state.phase, WorkflowPhase::Clarifying);
        assert_eq!(state.gate, WorkflowGate::Soft);
        assert!(state
            .matched_signals
            .iter()
            .any(|signal| signal.contains("不知道怎么说")));
    }

    #[test]
    fn classifies_destructive_migration_as_strict_workflow() {
        let state = route("删除旧的 session 存储并迁移到新格式");
        assert_eq!(state.route, WorkflowRoute::StrictWorkflow);
        assert_eq!(state.phase, WorkflowPhase::Planning);
        assert_eq!(state.gate, WorkflowGate::ApprovalRequired);
    }

    #[test]
    fn classifies_failure_as_recovery() {
        let state = route("构建失败了，dev 状态下会挂掉");
        assert_eq!(state.route, WorkflowRoute::Recovery);
        assert_eq!(state.phase, WorkflowPhase::Debugging);
    }

    #[test]
    fn strict_workflow_wins_over_recovery_when_signals_overlap() {
        let state = route("构建失败了，删除旧的 session 存储并迁移到新格式");
        assert_eq!(state.route, WorkflowRoute::StrictWorkflow);
        assert_eq!(state.phase, WorkflowPhase::Planning);
        assert_eq!(state.gate, WorkflowGate::ApprovalRequired);
    }

    #[test]
    fn strict_workflow_wins_over_verification_when_signals_overlap() {
        let state = route("帮我验收：删除旧的 session 存储并迁移到新格式");
        assert_eq!(state.route, WorkflowRoute::StrictWorkflow);
        assert_eq!(state.phase, WorkflowPhase::Planning);
        assert_eq!(state.gate, WorkflowGate::ApprovalRequired);
    }

    #[test]
    fn classifies_verification_request() {
        let state = route("帮我验收一下这个改动");
        assert_eq!(state.route, WorkflowRoute::Verification);
        assert_eq!(state.phase, WorkflowPhase::Verifying);
    }

    #[test]
    fn creates_manual_override_state() {
        let state = workflow_state_from_override("session-1", WorkflowOverrideAction::Debug, 99);
        assert_eq!(state.route, WorkflowRoute::Recovery);
        assert_eq!(state.updated_at, 99);
        assert!(state.reason.contains("手动切换"));
    }
}
