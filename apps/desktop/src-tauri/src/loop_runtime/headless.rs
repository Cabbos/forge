use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessResumeMode {
    #[default]
    Disabled,
    RequireHumanApproval,
    ApprovedForTask,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessResumeApproval {
    pub task_id: String,
    pub approved_by: String,
    pub approved_at_ms: u64,
    pub scope: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessAgentLease {
    pub task_id: String,
    pub session_id: String,
    pub lease_id: String,
    pub owner_pid: u32,
    pub expires_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadlessResumeReadiness {
    DesktopOwnerRequired,
    ApprovalRequired,
    ApprovalRecordedLeasePending,
    ApprovalExpired,
}

pub fn derive_headless_resume_readiness(
    mode: HeadlessResumeMode,
    approval: Option<&HeadlessResumeApproval>,
    now_ms: u64,
) -> HeadlessResumeReadiness {
    if let Some(approval) = approval {
        if approval.expires_at_ms <= now_ms {
            return HeadlessResumeReadiness::ApprovalExpired;
        }
        return HeadlessResumeReadiness::ApprovalRecordedLeasePending;
    }

    match mode {
        HeadlessResumeMode::RequireHumanApproval | HeadlessResumeMode::ApprovedForTask => {
            HeadlessResumeReadiness::ApprovalRequired
        }
        HeadlessResumeMode::Disabled => HeadlessResumeReadiness::DesktopOwnerRequired,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        derive_headless_resume_readiness, HeadlessResumeApproval, HeadlessResumeMode,
        HeadlessResumeReadiness,
    };

    #[test]
    fn headless_resume_readiness_requires_desktop_owner_when_disabled_without_approval() {
        let readiness = derive_headless_resume_readiness(HeadlessResumeMode::Disabled, None, 1_000);

        assert_eq!(readiness, HeadlessResumeReadiness::DesktopOwnerRequired);
    }

    #[test]
    fn headless_resume_readiness_waits_for_approval_when_policy_requires_human_approval() {
        let readiness =
            derive_headless_resume_readiness(HeadlessResumeMode::RequireHumanApproval, None, 1_000);

        assert_eq!(readiness, HeadlessResumeReadiness::ApprovalRequired);
    }

    #[test]
    fn headless_resume_readiness_is_lease_pending_after_unexpired_approval() {
        let approval = approval_for_test(500, 1_500);

        let readiness = derive_headless_resume_readiness(
            HeadlessResumeMode::ApprovedForTask,
            Some(&approval),
            1_000,
        );

        assert_eq!(
            readiness,
            HeadlessResumeReadiness::ApprovalRecordedLeasePending
        );
    }

    #[test]
    fn headless_resume_readiness_is_expired_when_approval_expiry_has_passed() {
        let approval = approval_for_test(500, 1_000);

        let readiness = derive_headless_resume_readiness(
            HeadlessResumeMode::ApprovedForTask,
            Some(&approval),
            1_000,
        );

        assert_eq!(readiness, HeadlessResumeReadiness::ApprovalExpired);
    }

    fn approval_for_test(approved_at_ms: u64, expires_at_ms: u64) -> HeadlessResumeApproval {
        HeadlessResumeApproval {
            task_id: "task-headless".to_string(),
            approved_by: "human-reviewer".to_string(),
            approved_at_ms,
            scope: "task".to_string(),
            expires_at_ms,
        }
    }
}
