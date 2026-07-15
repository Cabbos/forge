use crate::loop_runtime::budget::BudgetSnapshot;
use crate::loop_runtime::gates::HumanGateType;
use crate::loop_runtime::types::LoopPolicy;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "intent", rename_all = "snake_case")]
pub enum LoopActionIntent {
    ReadWorkspace {
        path: String,
    },
    EditDocs {
        path: String,
    },
    EditTests {
        path: String,
    },
    EditRuntimeCode {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        impact_analysis_evidence: Option<String>,
    },
    InstallDependency {
        package: String,
    },
    Commit {
        completion_contract_satisfied: bool,
        passing_evidence: bool,
    },
    PushBranch,
    DestructiveFilesystemAction {
        path: String,
    },
    ServiceLifecycle {
        service: String,
        lifecycle_action: String,
        update_repair_allowlisted: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopPolicyDecision {
    pub allowed: bool,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_gate_type: Option<HumanGateType>,
}

impl LoopPolicyDecision {
    fn allowed(reason: &str) -> Self {
        Self {
            allowed: true,
            reason: reason.to_string(),
            required_gate_type: None,
        }
    }

    fn blocked(reason: &str, gate_type: HumanGateType) -> Self {
        Self {
            allowed: false,
            reason: reason.to_string(),
            required_gate_type: Some(gate_type),
        }
    }
}

impl LoopPolicy {
    pub fn decide(
        &self,
        intent: LoopActionIntent,
        budget_snapshot: &BudgetSnapshot,
    ) -> LoopPolicyDecision {
        if budget_snapshot.budget_exceeded {
            return LoopPolicyDecision::blocked(
                "budget_exceeded_requires_human_approval",
                HumanGateType::BudgetOverride,
            );
        }

        match intent {
            LoopActionIntent::ReadWorkspace { path } => {
                if self.allow_workspace_reads && is_workspace_relative_path(&path) {
                    LoopPolicyDecision::allowed("allowed_by_background_task_policy")
                } else {
                    LoopPolicyDecision::blocked(
                        "workspace_read_requires_human_approval",
                        HumanGateType::PolicyOverride,
                    )
                }
            }
            LoopActionIntent::EditDocs { path } => {
                if self.allow_test_and_doc_edits
                    && is_workspace_relative_path(&path)
                    && path.starts_with("docs/")
                {
                    LoopPolicyDecision::allowed("allowed_by_background_task_policy")
                } else {
                    LoopPolicyDecision::blocked(
                        "docs_edit_requires_human_approval",
                        HumanGateType::PolicyOverride,
                    )
                }
            }
            LoopActionIntent::EditTests { path } => {
                if self.allow_test_and_doc_edits
                    && is_workspace_relative_path(&path)
                    && (path.contains("/tests/")
                        || path.contains("/e2e/")
                        || path.ends_with("_test.rs")
                        || path.ends_with(".spec.ts")
                        || path.ends_with(".test.ts"))
                {
                    LoopPolicyDecision::allowed("allowed_by_background_task_policy")
                } else {
                    LoopPolicyDecision::blocked(
                        "test_edit_requires_human_approval",
                        HumanGateType::PolicyOverride,
                    )
                }
            }
            LoopActionIntent::EditRuntimeCode {
                path,
                impact_analysis_evidence,
            } => {
                if self.allow_runtime_edits
                    || (is_workspace_relative_path(&path)
                        && impact_analysis_evidence
                            .as_deref()
                            .is_some_and(|evidence| !evidence.trim().is_empty()))
                {
                    LoopPolicyDecision::allowed("allowed_with_impact_analysis_evidence")
                } else {
                    LoopPolicyDecision::blocked(
                        "runtime_edit_requires_impact_analysis_or_human_approval",
                        HumanGateType::PolicyOverride,
                    )
                }
            }
            LoopActionIntent::InstallDependency { .. } => LoopPolicyDecision::blocked(
                "dependency_install_requires_human_approval",
                HumanGateType::PolicyOverride,
            ),
            LoopActionIntent::Commit {
                completion_contract_satisfied: _,
                passing_evidence: _,
            } => LoopPolicyDecision::blocked(
                "commit_remains_human_gated",
                HumanGateType::PolicyOverride,
            ),
            LoopActionIntent::PushBranch => LoopPolicyDecision::blocked(
                "push_requires_human_approval",
                HumanGateType::PolicyOverride,
            ),
            LoopActionIntent::DestructiveFilesystemAction { .. } => LoopPolicyDecision::blocked(
                "destructive_filesystem_requires_human_approval",
                HumanGateType::PolicyOverride,
            ),
            LoopActionIntent::ServiceLifecycle { .. } => {
                if self.allow_service_lifecycle {
                    LoopPolicyDecision::allowed("allowed_by_service_lifecycle_allowlist")
                } else {
                    LoopPolicyDecision::blocked(
                        "service_lifecycle_requires_human_approval",
                        HumanGateType::PolicyOverride,
                    )
                }
            }
        }
    }
}

fn is_workspace_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.split('/').any(|part| part == "..")
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{BudgetSnapshot, HumanGateType, LoopActionIntent, LoopPolicy};

    #[test]
    fn loop_policy_blocks_push_without_human_gate() {
        let policy = LoopPolicy::default_for_background_task();
        let decision = policy.decide(
            LoopActionIntent::PushBranch,
            &BudgetSnapshot::empty_for_test(),
        );

        assert!(!decision.allowed);
        assert_eq!(decision.reason, "push_requires_human_approval");
        assert_eq!(
            decision.required_gate_type,
            Some(HumanGateType::PolicyOverride)
        );
    }

    #[test]
    fn loop_policy_allows_docs_edit_inside_workspace() {
        let policy = LoopPolicy::default_for_background_task();
        let decision = policy.decide(
            LoopActionIntent::EditDocs {
                path: "docs/superpowers/plans/plan.md".to_string(),
            },
            &BudgetSnapshot::empty_for_test(),
        );

        assert!(decision.allowed);
        assert_eq!(decision.reason, "allowed_by_background_task_policy");
    }

    #[test]
    fn loop_policy_blocks_service_lifecycle_even_when_caller_claims_allowlist() {
        let policy = LoopPolicy::default_for_background_task();
        let decision = policy.decide(
            LoopActionIntent::ServiceLifecycle {
                service: "forge-runtime".to_string(),
                lifecycle_action: "restart".to_string(),
                update_repair_allowlisted: true,
            },
            &BudgetSnapshot::empty_for_test(),
        );

        assert!(!decision.allowed);
        assert_eq!(decision.reason, "service_lifecycle_requires_human_approval");
        assert_eq!(
            decision.required_gate_type,
            Some(HumanGateType::PolicyOverride)
        );
    }

    #[test]
    fn loop_policy_keeps_commit_human_gated_even_when_eligibility_is_true() {
        let mut policy = LoopPolicy::default_for_background_task();
        policy.allow_commit = true;
        let decision = policy.decide(
            LoopActionIntent::Commit {
                completion_contract_satisfied: true,
                passing_evidence: true,
            },
            &BudgetSnapshot::empty_for_test(),
        );

        assert!(!decision.allowed);
        assert_eq!(decision.reason, "commit_remains_human_gated");
        assert_eq!(
            decision.required_gate_type,
            Some(HumanGateType::PolicyOverride)
        );
    }
}
