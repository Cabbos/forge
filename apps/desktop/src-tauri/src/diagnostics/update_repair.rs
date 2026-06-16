//! Update repair planning and execution.
//!
//! This module keeps update-time self-healing deterministic and testable: the
//! planner decides which diagnostics repair actions are safe to run, while the
//! executor can be wired to the real repair registry or a test runner.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::diagnostics::repair::{run_repair, RepairResult};
use crate::diagnostics::{CheckStatus, DiagnosticCheck, DiagnosticsReport};

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRepairLifecycleRun {
    pub app_version: String,
    pub executed: bool,
    pub marker_path: String,
    pub repair_run: UpdateRepairRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRepairVersionMarker {
    app_version: String,
    checked_at_ms: u64,
    success: bool,
    needed: bool,
    action_count: usize,
    manual_blockers: Vec<String>,
}

pub fn plan_update_repair(report: &DiagnosticsReport) -> UpdateRepairPlan {
    let mut seen_actions = HashSet::new();
    let mut actions = Vec::new();
    let mut manual_blockers = Vec::new();

    for check in &report.checks {
        if check.id == "update_repair" {
            continue;
        }

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

pub fn execute_update_repair_for_current_version() -> Result<UpdateRepairLifecycleRun, String> {
    execute_update_repair_for_version_at(
        &forge_data_dir(),
        env!("CARGO_PKG_VERSION"),
        crate::diagnostics::run_diagnostics_basic,
        run_repair,
    )
}

pub fn check_update_repair_status() -> DiagnosticCheck {
    check_update_repair_status_at(&forge_data_dir())
}

fn check_update_repair_status_at(root: &Path) -> DiagnosticCheck {
    let marker_path = update_repair_marker_path(root);
    match read_update_repair_marker(root) {
        Ok(Some(marker)) => update_repair_check_from_marker(marker, marker_path),
        Ok(None) => DiagnosticCheck::pass(
            "update_repair",
            "Update repair",
            "No update repair state recorded yet.",
        )
        .with_detail(serde_json::json!({
            "recorded": false,
            "markerPath": marker_path.display().to_string(),
        })),
        Err(error) => DiagnosticCheck::warn(
            "update_repair",
            "Update repair",
            format!("Cannot read update repair state: {error}"),
        )
        .with_detail(serde_json::json!({
            "recorded": false,
            "markerPath": marker_path.display().to_string(),
            "error": error,
        }))
        .with_remediation(
            "Remove the update repair marker if it stays unreadable, then restart Forge.",
        ),
    }
}

fn update_repair_check_from_marker(
    marker: UpdateRepairVersionMarker,
    marker_path: PathBuf,
) -> DiagnosticCheck {
    let detail = serde_json::json!({
        "recorded": true,
        "markerPath": marker_path.display().to_string(),
        "appVersion": marker.app_version,
        "checkedAtMs": marker.checked_at_ms,
        "success": marker.success,
        "needed": marker.needed,
        "actionCount": marker.action_count,
        "manualBlockers": marker.manual_blockers,
    });

    let app_version = detail["appVersion"].as_str().unwrap_or("unknown");
    let action_count = detail["actionCount"].as_u64().unwrap_or(0);
    let manual_blocker_count = detail["manualBlockers"].as_array().map_or(0, Vec::len);

    if detail["success"].as_bool().unwrap_or(false) {
        let message = if detail["needed"].as_bool().unwrap_or(false) {
            format!(
                "Update repair completed for version {app_version}: {action_count} action{} ran.",
                if action_count == 1 { "" } else { "s" }
            )
        } else {
            format!("Update repair checked version {app_version}; no action needed.")
        };

        return DiagnosticCheck::pass("update_repair", "Update repair", message)
            .with_detail(detail);
    }

    if manual_blocker_count > 0 {
        DiagnosticCheck::warn(
            "update_repair",
            "Update repair",
            format!(
                "Update repair for version {app_version} needs manual review: {manual_blocker_count} blocker{}.",
                if manual_blocker_count == 1 { "" } else { "s" }
            ),
        )
        .with_detail(detail)
        .with_remediation("Open Settings > Diagnostics and run the recommended repair action.")
    } else {
        DiagnosticCheck::fail(
            "update_repair",
            "Update repair",
            format!("Update repair failed for version {app_version}."),
        )
        .with_detail(detail)
        .with_remediation("Open Settings > Diagnostics and run the recommended repair action.")
    }
}

fn execute_update_repair_for_version_at(
    root: &Path,
    app_version: &str,
    mut diagnostics_runner: impl FnMut() -> DiagnosticsReport,
    runner: impl FnMut(&str) -> RepairResult,
) -> Result<UpdateRepairLifecycleRun, String> {
    let marker_path = update_repair_marker_path(root);
    match read_update_repair_marker(root) {
        Ok(Some(marker)) if marker.app_version == app_version => {
            return Ok(UpdateRepairLifecycleRun {
                app_version: app_version.to_string(),
                executed: false,
                marker_path: marker_path.display().to_string(),
                repair_run: no_update_repair_needed(),
            });
        }
        Ok(None) => {
            write_update_repair_marker(root, &baseline_update_repair_marker(app_version))?;
            return Ok(UpdateRepairLifecycleRun {
                app_version: app_version.to_string(),
                executed: false,
                marker_path: marker_path.display().to_string(),
                repair_run: no_update_repair_needed(),
            });
        }
        Ok(Some(_)) | Err(_) => {}
    }

    let report = diagnostics_runner();
    let plan = plan_update_repair(&report);
    let repair_run = execute_update_repair_plan_with_runner(&plan, runner);
    write_update_repair_marker(
        root,
        &UpdateRepairVersionMarker {
            app_version: app_version.to_string(),
            checked_at_ms: now_ms(),
            success: repair_run.success,
            needed: repair_run.plan.needed,
            action_count: repair_run.results.len(),
            manual_blockers: repair_run.plan.manual_blockers.clone(),
        },
    )?;

    Ok(UpdateRepairLifecycleRun {
        app_version: app_version.to_string(),
        executed: true,
        marker_path: marker_path.display().to_string(),
        repair_run,
    })
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

fn baseline_update_repair_marker(app_version: &str) -> UpdateRepairVersionMarker {
    UpdateRepairVersionMarker {
        app_version: app_version.to_string(),
        checked_at_ms: now_ms(),
        success: true,
        needed: false,
        action_count: 0,
        manual_blockers: Vec::new(),
    }
}

fn no_update_repair_needed() -> UpdateRepairRun {
    UpdateRepairRun {
        success: true,
        plan: UpdateRepairPlan {
            needed: false,
            reason: "Update repair already checked for this version.".to_string(),
            actions: Vec::new(),
            manual_blockers: Vec::new(),
        },
        results: Vec::new(),
    }
}

fn read_update_repair_marker(root: &Path) -> Result<Option<UpdateRepairVersionMarker>, String> {
    let path = update_repair_marker_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let json =
        fs::read_to_string(&path).map_err(|error| format!("read update repair marker: {error}"))?;
    let marker = serde_json::from_str(&json)
        .map_err(|error| format!("parse update repair marker: {error}"))?;
    Ok(Some(marker))
}

fn write_update_repair_marker(
    root: &Path,
    marker: &UpdateRepairVersionMarker,
) -> Result<(), String> {
    let path = update_repair_marker_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create update repair marker dir: {error}"))?;
    }
    let json = serde_json::to_string_pretty(marker)
        .map_err(|error| format!("serialize update repair marker: {error}"))?;
    fs::write(&path, json).map_err(|error| format!("write update repair marker: {error}"))
}

fn update_repair_marker_path(root: &Path) -> PathBuf {
    root.join("update-repair-state.json")
}

fn forge_data_dir() -> PathBuf {
    home_dir().join(".forge")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::repair::RepairResult;
    use crate::diagnostics::{CheckStatus, DiagnosticCheck, DiagnosticsReport};
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn report_with(checks: Vec<DiagnosticCheck>) -> DiagnosticsReport {
        DiagnosticsReport {
            ok: checks.iter().all(|check| check.status == CheckStatus::Pass),
            generated_at_ms: 42,
            checks,
        }
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "forge-update-repair-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
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
    fn plan_update_repair_ignores_update_repair_observability_check() {
        let report = report_with(vec![
            DiagnosticCheck::fail("update_repair", "Update repair", "Previous repair failed."),
            DiagnosticCheck::warn("gateway_service", "Gateway service", "Gateway stopped.")
                .with_repair_action_id("restart_gateway"),
        ]);

        let plan = plan_update_repair(&report);

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action_id, "restart_gateway");
        assert!(plan.manual_blockers.is_empty());
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

    #[test]
    fn update_repair_check_passes_when_marker_is_successful() {
        let root = temp_root("check-pass");
        write_update_repair_marker(
            &root,
            &UpdateRepairVersionMarker {
                app_version: "1.2.3".into(),
                checked_at_ms: 1234,
                success: true,
                needed: true,
                action_count: 2,
                manual_blockers: Vec::new(),
            },
        )
        .expect("marker");

        let check = check_update_repair_status_at(&root);

        assert_eq!(check.id, "update_repair");
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.message.contains("1.2.3"));
        let detail = check.detail.expect("detail");
        assert_eq!(detail["appVersion"], "1.2.3");
        assert_eq!(detail["actionCount"], 2);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_repair_check_warns_when_manual_blockers_remain() {
        let root = temp_root("check-manual-blockers");
        write_update_repair_marker(
            &root,
            &UpdateRepairVersionMarker {
                app_version: "1.2.4".into(),
                checked_at_ms: 5678,
                success: false,
                needed: true,
                action_count: 1,
                manual_blockers: vec!["config_settings".into()],
            },
        )
        .expect("marker");

        let check = check_update_repair_status_at(&root);

        assert_eq!(check.id, "update_repair");
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("manual review"));
        assert_eq!(
            check.detail.expect("detail")["manualBlockers"],
            serde_json::json!(["config_settings"])
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_repair_for_version_baselines_then_runs_once_on_version_change() {
        let root = temp_root("runs-once");
        let diagnostics_calls = Cell::new(0);
        let repair_calls = RefCell::new(Vec::new());

        let first = execute_update_repair_for_version_at(
            &root,
            "1.0.0",
            || {
                diagnostics_calls.set(diagnostics_calls.get() + 1);
                report_with(vec![DiagnosticCheck::warn(
                    "gateway_service",
                    "Gateway service",
                    "Gateway stopped.",
                )
                .with_repair_action_id("restart_gateway")])
            },
            |action_id| {
                repair_calls.borrow_mut().push(action_id.to_string());
                RepairResult {
                    action_id: action_id.to_string(),
                    success: true,
                    message: "ok".into(),
                    verification: None,
                }
            },
        )
        .expect("first update repair");

        assert!(!first.executed);
        assert!(first.repair_run.success);
        assert_eq!(diagnostics_calls.get(), 0);
        assert!(repair_calls.borrow().is_empty());

        let second = execute_update_repair_for_version_at(
            &root,
            "1.0.0",
            || {
                diagnostics_calls.set(diagnostics_calls.get() + 1);
                report_with(Vec::new())
            },
            |action_id| {
                repair_calls.borrow_mut().push(action_id.to_string());
                RepairResult {
                    action_id: action_id.to_string(),
                    success: true,
                    message: "unexpected".into(),
                    verification: None,
                }
            },
        )
        .expect("second update repair");

        assert!(!second.executed);
        assert_eq!(diagnostics_calls.get(), 0);
        assert!(repair_calls.borrow().is_empty());

        let third = execute_update_repair_for_version_at(
            &root,
            "1.1.0",
            || {
                diagnostics_calls.set(diagnostics_calls.get() + 1);
                report_with(vec![DiagnosticCheck::warn(
                    "gateway_service",
                    "Gateway service",
                    "Gateway stopped.",
                )
                .with_repair_action_id("restart_gateway")])
            },
            |action_id| {
                repair_calls.borrow_mut().push(action_id.to_string());
                RepairResult {
                    action_id: action_id.to_string(),
                    success: true,
                    message: "ok".into(),
                    verification: None,
                }
            },
        )
        .expect("third update repair");

        assert!(third.executed);
        assert!(third.repair_run.success);
        assert_eq!(diagnostics_calls.get(), 1);
        assert_eq!(repair_calls.borrow().as_slice(), ["restart_gateway"]);

        let _ = fs::remove_dir_all(root);
    }
}
