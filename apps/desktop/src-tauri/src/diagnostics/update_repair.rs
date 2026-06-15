//! Update repair planning and execution.
//!
//! This module keeps update-time self-healing deterministic and testable: the
//! planner decides which diagnostics repair actions are safe to run, while the
//! executor can be wired to the real repair registry or a test runner.

use serde::Serialize;
use std::collections::HashSet;

use crate::diagnostics::repair::{run_repair, RepairResult};
use crate::diagnostics::{CheckStatus, DiagnosticsReport};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRepairActionPlan {
    pub check_id: String,
    pub action_id: String,
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRepairPlan {
    pub needed: bool,
    pub reason: String,
    pub actions: Vec<UpdateRepairActionPlan>,
    pub manual_blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRepairRun {
    pub success: bool,
    pub plan: UpdateRepairPlan,
    pub results: Vec<RepairResult>,
}

pub fn plan_update_repair(report: &DiagnosticsReport) -> UpdateRepairPlan {
    let mut seen_actions = HashSet::new();
    let mut actions = Vec::new();
    let mut manual_blockers = Vec::new();

    for check in &report.checks {
        if check.status == CheckStatus::Pass {
            continue;
        }

        if let Some(action_id) = check.repair_action_id.as_deref() {
            let automatic = update_repair_action_is_automatic(action_id);
            if automatic && seen_actions.insert(action_id.to_string()) {
                actions.push(UpdateRepairActionPlan {
                    check_id: check.id.clone(),
                    action_id: action_id.to_string(),
                    label: check.label.clone(),
                    reason: check.message.clone(),
                });
            } else if !automatic {
                manual_blockers.push(check.id.clone());
            }
        } else if check.status == CheckStatus::Fail {
            manual_blockers.push(check.id.clone());
        }
    }

    let needed = !actions.is_empty() || !manual_blockers.is_empty();
    let reason = if !needed {
        "No update repair needed.".to_string()
    } else {
        format!(
            "{} repair action{} planned; {} manual blocker{}.",
            actions.len(),
            if actions.len() == 1 { "" } else { "s" },
            manual_blockers.len(),
            if manual_blockers.len() == 1 { "" } else { "s" }
        )
    };

    UpdateRepairPlan {
        needed,
        reason,
        actions,
        manual_blockers,
    }
}

fn update_repair_action_is_automatic(action_id: &str) -> bool {
    matches!(action_id, "restart_gateway" | "reinstall_service")
}

pub fn execute_update_repair(report: &DiagnosticsReport) -> UpdateRepairRun {
    let plan = plan_update_repair(report);
    execute_update_repair_plan_with_runner(&plan, run_repair)
}

fn execute_update_repair_plan_with_runner(
    plan: &UpdateRepairPlan,
    mut runner: impl FnMut(&str) -> RepairResult,
) -> UpdateRepairRun {
    let results: Vec<RepairResult> = plan
        .actions
        .iter()
        .map(|action| runner(&action.action_id))
        .collect();
    let all_repairs_passed = results.iter().all(|result| result.success);
    let success = plan.manual_blockers.is_empty() && all_repairs_passed;

    UpdateRepairRun {
        success,
        plan: plan.clone(),
        results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::repair::RepairResult;
    use crate::diagnostics::{CheckStatus, DiagnosticCheck, DiagnosticsReport};

    fn report_with(checks: Vec<DiagnosticCheck>) -> DiagnosticsReport {
        DiagnosticsReport {
            ok: checks.iter().all(|check| check.status == CheckStatus::Pass),
            generated_at_ms: 42,
            checks,
        }
    }

    #[test]
    fn plan_update_repair_schedules_gateway_repair_actions() {
        let report = report_with(vec![DiagnosticCheck::warn(
            "gateway_service",
            "Gateway service",
            "Gateway service is installed but not running.",
        )
        .with_repair_action_id("restart_gateway")]);

        let plan = plan_update_repair(&report);

        assert!(plan.needed);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action_id, "restart_gateway");
        assert_eq!(plan.actions[0].check_id, "gateway_service");
        assert!(plan.manual_blockers.is_empty());
    }

    #[test]
    fn plan_update_repair_keeps_service_repair_actions_automatic() {
        let report = report_with(vec![
            DiagnosticCheck::warn("gateway_service", "Gateway service", "Gateway stopped.")
                .with_repair_action_id("restart_gateway"),
            DiagnosticCheck::warn("service_install", "Gateway service", "Service plist stale.")
                .with_repair_action_id("reinstall_service"),
        ]);

        let plan = plan_update_repair(&report);

        let action_ids = plan
            .actions
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(action_ids, vec!["restart_gateway", "reinstall_service"]);
        assert!(plan.manual_blockers.is_empty());
    }

    #[test]
    fn plan_update_repair_requires_manual_review_for_destructive_actions() {
        let report = report_with(vec![DiagnosticCheck::warn(
            "a2a_ledger",
            "A2A task ledger",
            "All A2A ledger files are corrupted.",
        )
        .with_repair_action_id("clear_a2a_ledger_cache")]);

        let plan = plan_update_repair(&report);

        assert!(plan.needed);
        assert!(plan.actions.is_empty());
        assert_eq!(plan.manual_blockers, vec!["a2a_ledger"]);
    }

    #[test]
    fn plan_update_repair_requires_manual_review_for_unknown_actions() {
        let report = report_with(vec![DiagnosticCheck::warn(
            "future_check",
            "Future check",
            "A future repair action appeared.",
        )
        .with_repair_action_id("future_repair_action")]);

        let plan = plan_update_repair(&report);

        assert!(plan.needed);
        assert!(plan.actions.is_empty());
        assert_eq!(plan.manual_blockers, vec!["future_check"]);
    }

    #[test]
    fn plan_update_repair_deduplicates_repair_actions() {
        let report = report_with(vec![
            DiagnosticCheck::warn("gateway_service", "Gateway service", "Gateway stopped.")
                .with_repair_action_id("restart_gateway"),
            DiagnosticCheck::warn(
                "gateway_runtime",
                "Gateway runtime",
                "Gateway socket offline.",
            )
            .with_repair_action_id("restart_gateway"),
        ]);

        let plan = plan_update_repair(&report);

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action_id, "restart_gateway");
    }

    #[test]
    fn plan_update_repair_keeps_unrepairable_failures_as_manual_blockers() {
        let report = report_with(vec![DiagnosticCheck::fail(
            "config_settings",
            "Config / API keys",
            "Config loaded but no keys are set.",
        )]);

        let plan = plan_update_repair(&report);

        assert!(plan.needed);
        assert!(plan.actions.is_empty());
        assert_eq!(plan.manual_blockers, vec!["config_settings"]);
    }

    #[test]
    fn execute_update_repair_plan_runs_actions_in_order() {
        let plan = UpdateRepairPlan {
            needed: true,
            reason: "1 repair action planned.".into(),
            actions: vec![
                UpdateRepairActionPlan {
                    check_id: "gateway_service".into(),
                    action_id: "restart_gateway".into(),
                    label: "Gateway service".into(),
                    reason: "Gateway service is installed but not running.".into(),
                },
                UpdateRepairActionPlan {
                    check_id: "service_install".into(),
                    action_id: "reinstall_service".into(),
                    label: "Gateway service".into(),
                    reason: "Service plist stale.".into(),
                },
            ],
            manual_blockers: Vec::new(),
        };
        let mut calls = Vec::new();

        let run = execute_update_repair_plan_with_runner(&plan, |action_id| {
            calls.push(action_id.to_string());
            RepairResult {
                action_id: action_id.to_string(),
                success: true,
                message: "ok".into(),
                verification: None,
            }
        });

        assert_eq!(calls, vec!["restart_gateway", "reinstall_service"]);
        assert!(run.success);
        assert_eq!(run.results.len(), 2);
    }

    #[test]
    fn execute_update_repair_plan_fails_when_a_repair_action_fails() {
        let plan = UpdateRepairPlan {
            needed: true,
            reason: "1 repair action planned.".into(),
            actions: vec![UpdateRepairActionPlan {
                check_id: "gateway_service".into(),
                action_id: "restart_gateway".into(),
                label: "Gateway service".into(),
                reason: "Gateway service is installed but not running.".into(),
            }],
            manual_blockers: Vec::new(),
        };

        let run = execute_update_repair_plan_with_runner(&plan, |action_id| RepairResult {
            action_id: action_id.to_string(),
            success: false,
            message: "verification failed".into(),
            verification: None,
        });

        assert!(!run.success);
        assert_eq!(run.results.len(), 1);
    }
}
