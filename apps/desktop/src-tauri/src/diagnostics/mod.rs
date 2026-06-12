//! Diagnostics / doctor module — runtime health checks for Forge desktop.
//!
//! Produces a structured `DiagnosticsReport` with ordered checks that can be
//! serialized to JSON for the Tauri IPC layer and the CLI doctor command.
//!
//! Sub-modules:
//! - `watchdog` — session event tracker and stale-session health alert emission.

pub mod repair;
pub mod watchdog;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Report model ──────────────────────────────────────────────────────────

/// Top-level diagnostics report returned by the health-check runner.
///
/// `ok` is true when every check has status `Pass`; any `Warn` or `Fail`
/// makes it false.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub ok: bool,
    pub generated_at_ms: u64,
    pub checks: Vec<DiagnosticCheck>,
}

/// A single named health check with status, message, optional detail, and an
/// optional remediation hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub id: String,
    pub label: String,
    pub status: CheckStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

// ── Check builder helpers ─────────────────────────────────────────────────

impl DiagnosticCheck {
    pub fn pass(
        id: impl Into<String>,
        label: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: CheckStatus::Pass,
            message: message.into(),
            detail: None,
            remediation: None,
        }
    }

    pub fn warn(
        id: impl Into<String>,
        label: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: CheckStatus::Warn,
            message: message.into(),
            detail: None,
            remediation: None,
        }
    }

    pub fn fail(
        id: impl Into<String>,
        label: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: CheckStatus::Fail,
            message: message.into(),
            detail: None,
            remediation: None,
        }
    }

    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }
}

// ── Diagnostics runner ────────────────────────────────────────────────────

/// Run all health checks and return a report.
///
/// `capabilities` is optional — when `Some`, the tool-inventory check can
/// report real data; when `None`, it emits a `Warn` with a TODO message.
pub fn run_diagnostics(capabilities: Option<Vec<CapabilitySummary>>) -> DiagnosticsReport {
    let checks: Vec<DiagnosticCheck> = vec![
        check_config_key_presence(),
        check_session_snapshots(),
        check_app_metadata(),
        check_log_directory(),
        check_capability_inventory(capabilities),
        check_project_runtime(),
    ];

    let ok = checks.iter().all(|c| c.status == CheckStatus::Pass);
    let generated_at_ms = now_ms();

    DiagnosticsReport {
        ok,
        generated_at_ms,
        checks,
    }
}

/// Run diagnostics without requiring capability data — used by the Tauri
/// command when the harness is available, and by tests.
///
/// Returns a `DiagnosticsReport` with `Warn` for the capability check.
pub fn run_diagnostics_basic() -> DiagnosticsReport {
    run_diagnostics(None)
}

// ── Individual check functions (public for unit testing) ──────────────────

/// Check that the config file is readable and summarize API-key presence.
pub fn check_config_key_presence() -> DiagnosticCheck {
    match crate::settings::Settings::load().key_status() {
        key_statuses if key_statuses.is_empty() => {
            DiagnosticCheck::warn(
                "config_settings",
                "Config / API keys",
                "No API keys configured for any provider.",
            )
            .with_remediation("Add an API key in Settings → Models or set an environment variable like DEEPSEEK_API_KEY.")
        }
        key_statuses => {
            let set_count = key_statuses.iter().filter(|k| k.set).count();
            let total = key_statuses.len();
            let details: Vec<serde_json::Value> = key_statuses
                .iter()
                .map(|k| {
                    serde_json::json!({
                        "provider": k.provider,
                        "set": k.set,
                        "preview": k.preview,
                    })
                })
                .collect();

            if set_count == 0 {
                DiagnosticCheck::fail(
                    "config_settings",
                    "Config / API keys",
                    format!(
                        "Config loaded ({total} provider{} tracked) but no keys are set.",
                        if total == 1 { "" } else { "s" }
                    ),
                )
                .with_detail(serde_json::Value::Array(details))
                .with_remediation("Add an API key in Settings → Models or set an environment variable like DEEPSEEK_API_KEY.")
            } else {
                DiagnosticCheck::pass(
                    "config_settings",
                    "Config / API keys",
                    format!(
                        "Config readable — {set_count}/{total} provider{} have API keys set.",
                        if total == 1 { "" } else { "s" }
                    ),
                )
                .with_detail(serde_json::Value::Array(details))
            }
        }
    }
}

/// Check that session snapshots are listable and summarize count, newest age,
/// and corruption count.
pub fn check_session_snapshots() -> DiagnosticCheck {
    // We need to count both successful snapshots and corruption errors.
    // list_session_snapshots already logs warnings for corrupted files but
    // swallows them. To get a corruption count we scan the sessions dir
    // ourselves and compare.

    let snapshots_dir = forge_data_dir().join("sessions");

    let (total_files, corrupt_count, readable_snapshots) = if snapshots_dir.exists() {
        match std::fs::read_dir(&snapshots_dir) {
            Ok(entries) => {
                let json_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
                    .collect();
                let total = json_files.len();

                let mut corrupt = 0u64;
                let mut snapshots = Vec::new();
                for entry in &json_files {
                    match std::fs::read_to_string(entry.path()) {
                        Ok(json) => match serde_json::from_str::<
                            crate::agent::snapshot::AgentSessionSnapshot,
                        >(&json)
                        {
                            Ok(snapshot) => snapshots.push(snapshot),
                            Err(_) => corrupt += 1,
                        },
                        Err(_) => corrupt += 1,
                    }
                }
                (total, corrupt, snapshots)
            }
            Err(_) => (0, 0, Vec::new()),
        }
    } else {
        (0, 0, Vec::new())
    };

    let now = now_ms();
    let snapshot_count = readable_snapshots.len();

    // Find the newest snapshot's age in seconds
    let newest_age_secs = readable_snapshots
        .iter()
        .map(|s| {
            let age_ms = now.saturating_sub(s.updated_at_ms);
            age_ms / 1000
        })
        .min();

    let detail = serde_json::json!({
        "total_snapshots": snapshot_count,
        "corrupt_snapshots": corrupt_count,
        "total_json_files": total_files,
        "newest_age_secs": newest_age_secs,
    });

    if total_files == 0 {
        DiagnosticCheck::pass(
            "session_snapshots",
            "Session snapshots",
            "No session snapshots found (fresh install or all sessions cleaned).",
        )
        .with_detail(detail)
    } else if corrupt_count > 0 && snapshot_count == 0 {
        DiagnosticCheck::fail(
            "session_snapshots",
            "Session snapshots",
            format!(
                "All {total_files} snapshot file{} are corrupted.",
                if total_files == 1 { "" } else { "s" }
            ),
        )
        .with_detail(detail)
        .with_remediation(
            "Corrupted snapshots cannot be recovered. Delete them from the sessions directory to start fresh.",
        )
    } else if corrupt_count > 0 {
        DiagnosticCheck::warn(
            "session_snapshots",
            "Session snapshots",
            format!(
                "{snapshot_count} readable snapshot{}, {corrupt_count} corrupted.",
                if snapshot_count == 1 { "" } else { "s" }
            ),
        )
        .with_detail(detail)
        .with_remediation(
            "Corrupted snapshots will be skipped during restore. Delete the corrupted files to silence this warning.",
        )
    } else if let Some(age_secs) = newest_age_secs {
        if age_secs > 86_400 {
            // > 24 hours
            DiagnosticCheck::warn(
                "session_snapshots",
                "Session snapshots",
                format!(
                    "{snapshot_count} snapshot{} — newest is {:.1} hours old.",
                    if snapshot_count == 1 { "" } else { "s" },
                    age_secs as f64 / 3600.0
                ),
            )
            .with_detail(detail)
        } else {
            DiagnosticCheck::pass(
                "session_snapshots",
                "Session snapshots",
                format!(
                    "{snapshot_count} snapshot{} — newest is {} seconds old.",
                    if snapshot_count == 1 { "" } else { "s" },
                    age_secs
                ),
            )
            .with_detail(detail)
        }
    } else {
        DiagnosticCheck::pass(
            "session_snapshots",
            "Session snapshots",
            format!(
                "{snapshot_count} snapshot{} readable.",
                if snapshot_count == 1 { "" } else { "s" }
            ),
        )
        .with_detail(detail)
    }
}

/// Check that app metadata is readable.
pub fn check_app_metadata() -> DiagnosticCheck {
    let metadata_path = forge_data_dir().join("app-state.json");
    if !metadata_path.exists() {
        DiagnosticCheck::pass(
            "app_metadata",
            "App metadata",
            "App metadata file not present (fresh install).",
        )
        .with_detail(serde_json::json!({"path": metadata_path.to_string_lossy()}))
    } else {
        match std::fs::read_to_string(&metadata_path) {
            Ok(json) => match serde_json::from_str::<crate::app_metadata::AppMetadata>(&json) {
                Ok(metadata) => {
                    let workspace_count = metadata.workspaces.len();
                    DiagnosticCheck::pass(
                        "app_metadata",
                        "App metadata",
                        format!(
                            "App metadata readable — {workspace_count} workspace{}.",
                            if workspace_count == 1 { "" } else { "s" }
                        ),
                    )
                    .with_detail(serde_json::json!({
                        "path": metadata_path.to_string_lossy(),
                        "workspace_count": workspace_count,
                        "has_active_workspace": metadata.active_workspace_id.is_some(),
                        "has_active_session": metadata.active_session_id.is_some(),
                    }))
                }
                Err(e) => DiagnosticCheck::fail(
                    "app_metadata",
                    "App metadata",
                    format!("App metadata is corrupted: {e}"),
                )
                .with_detail(serde_json::json!({"path": metadata_path.to_string_lossy()}))
                .with_remediation("Delete the app-state.json file to start fresh."),
            },
            Err(e) => DiagnosticCheck::fail(
                "app_metadata",
                "App metadata",
                format!("Cannot read app metadata: {e}"),
            )
            .with_detail(serde_json::json!({"path": metadata_path.to_string_lossy()})),
        }
    }
}

/// Check that the log / data directory exists and is writable.
pub fn check_log_directory() -> DiagnosticCheck {
    let log_path_str = crate::logger::log_path_str();
    let log_path = Path::new(&log_path_str);

    let data_dir = forge_data_dir();
    let data_dir_exists = data_dir.exists();
    let data_dir_writable = if data_dir_exists {
        can_write_marker_file(&data_dir)
    } else {
        // Try creating the dir
        std::fs::create_dir_all(&data_dir).is_ok()
    };

    let log_exists = log_path.exists();

    let detail = serde_json::json!({
        "data_dir": data_dir.to_string_lossy(),
        "data_dir_exists": data_dir_exists,
        "data_dir_writable": data_dir_writable,
        "log_path": log_path_str,
        "log_exists": log_exists,
    });

    if !data_dir_writable {
        DiagnosticCheck::fail(
            "log_directory",
            "Log / data directory",
            format!("Data directory {} is not writable.", data_dir.display()),
        )
        .with_detail(detail)
        .with_remediation("Check file permissions on ~/.forge/.")
    } else if !data_dir_exists {
        DiagnosticCheck::warn(
            "log_directory",
            "Log / data directory",
            "Data directory created during check (was missing).",
        )
        .with_detail(detail)
    } else if !log_exists {
        DiagnosticCheck::warn(
            "log_directory",
            "Log / data directory",
            "Log file not yet created (no sessions have run).",
        )
        .with_detail(detail)
    } else {
        DiagnosticCheck::pass(
            "log_directory",
            "Log / data directory",
            "Data directory exists and is writable; log file present.",
        )
        .with_detail(detail)
    }
}

/// Summary of a tool/capability for the diagnostics report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    /// Health/availability status (Phase 3-A).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Optional human-readable status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

/// Check that tool / capability inventory is loadable.
///
/// When `capabilities` is `Some`, reports real counts including
/// healthy/unhealthy/unavailable breakdown when status data is available.
/// When `None`, emits a `Warn` with a remediation hint.
pub fn check_capability_inventory(capabilities: Option<Vec<CapabilitySummary>>) -> DiagnosticCheck {
    match capabilities {
        Some(caps) if caps.is_empty() => DiagnosticCheck::warn(
            "capability_inventory",
            "Tool / capability inventory",
            "Capability registry is empty — no tools, hooks, MCP servers, or skills found.",
        )
        .with_remediation(
            "Verify your plugin/skill configuration and check that the .claude/ directory exists.",
        ),
        Some(caps) => {
            let total = caps.len();
            let enabled = caps.iter().filter(|c| c.enabled).count();
            let disabled = total - enabled;
            let by_kind: std::collections::HashMap<String, usize> =
                caps.iter()
                    .fold(std::collections::HashMap::new(), |mut acc, c| {
                        *acc.entry(c.kind.clone()).or_default() += 1;
                        acc
                    });

            // Phase 3-A: count unhealthy/unavailable items
            let unhealthy: Vec<&CapabilitySummary> = caps
                .iter()
                .filter(|c| {
                    c.status.as_deref() == Some("unavailable")
                        || c.status.as_deref() == Some("warning")
                })
                .collect();
            let unhealthy_count = unhealthy.len();
            let has_status_data = caps.iter().any(|c| c.status.is_some());

            let mut detail = serde_json::json!({
                "total": total,
                "enabled": enabled,
                "disabled": disabled,
                "by_kind": by_kind,
            });

            let check = if unhealthy_count > 0 {
                let names: Vec<&str> = unhealthy.iter().map(|c| c.name.as_str()).collect();
                detail["unhealthy_count"] = serde_json::json!(unhealthy_count);
                detail["unhealthy_items"] = serde_json::json!(names);
                DiagnosticCheck::warn(
                    "capability_inventory",
                    "Tool / capability inventory",
                    format!(
                        "{total} total ({enabled} enabled, {disabled} disabled) — \
                         {unhealthy_count} unhealthy/unavailable.",
                    ),
                )
                .with_detail(detail)
                .with_remediation(
                    "Check the unhealthy items in Settings → Ecosystem for details \
                     and try reconnecting or restarting the affected service.",
                )
            } else {
                DiagnosticCheck::pass(
                    "capability_inventory",
                    "Tool / capability inventory",
                    format!("{total} total ({enabled} enabled, {disabled} disabled).",),
                )
                .with_detail(detail)
            };

            if has_status_data {
                check
            } else {
                // No status data means we are running basic diagnostics;
                // keep the pass/warn result but note the limitation.
                check
            }
        }
        None => DiagnosticCheck::warn(
            "capability_inventory",
            "Tool / capability inventory",
            "Capability inventory not available (check runs outside Tauri runtime).",
        )
        .with_remediation("Run forge from the desktop app to see the full ecosystem inventory."),
    }
}

/// Cheap probe for project runtime / dev server status.
///
/// Avoids starting or stopping servers. Only reports whether a working dir
/// can be resolved from metadata and whether it has a package.json.
pub fn check_project_runtime() -> DiagnosticCheck {
    let metadata_path = forge_data_dir().join("app-state.json");
    let metadata: Option<crate::app_metadata::AppMetadata> =
        std::fs::read_to_string(&metadata_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok());

    let active_workspace = metadata.as_ref().and_then(configured_workspace_path);

    match active_workspace {
        Some(working_dir) => {
            let pkg_json_path = Path::new(working_dir).join("package.json");
            let has_package_json = pkg_json_path.exists();
            let detail = serde_json::json!({
                "working_dir": working_dir,
                "has_package_json": has_package_json,
            });

            if has_package_json {
                DiagnosticCheck::pass(
                    "project_runtime",
                    "Project runtime",
                    format!("Working directory '{}' has a package.json.", working_dir),
                )
                .with_detail(detail)
            } else {
                DiagnosticCheck::warn(
                    "project_runtime",
                    "Project runtime",
                    format!(
                        "Working directory '{}' has no package.json — runtime status unavailable.",
                        working_dir
                    ),
                )
                .with_detail(detail)
            }
        }
        None => DiagnosticCheck::warn(
            "project_runtime",
            "Project runtime",
            "No working directory configured — cannot check project runtime.",
        )
        .with_remediation("Open a project workspace in Forge to enable project runtime checks."),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn forge_data_dir() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".forge")
}

fn configured_workspace_path(metadata: &crate::app_metadata::AppMetadata) -> Option<&str> {
    metadata
        .active_workspace_id
        .as_deref()
        .and_then(|active| {
            metadata
                .workspaces
                .iter()
                .find(|workspace| workspace.id == active || workspace.path == active)
                .map(|workspace| workspace.path.as_str())
        })
        .or_else(|| {
            metadata
                .workspaces
                .first()
                .map(|workspace| workspace.path.as_str())
        })
}

fn can_write_marker_file(dir: &Path) -> bool {
    let marker = dir.join(format!(".diagnostics_write_test-{}", std::process::id()));
    match std::fs::write(&marker, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(marker);
            true
        }
        Err(_) => false,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "forge-diag-{}-{}",
            prefix,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    // ── Report aggregation ────────────────────────────────────────────────

    #[test]
    fn basic_report_has_stable_check_order_and_generated_timestamp() {
        let report = run_diagnostics_basic();
        assert!(report.generated_at_ms > 0);
        // The 6 checks must appear in a stable order
        let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "config_settings",
                "session_snapshots",
                "app_metadata",
                "log_directory",
                "capability_inventory",
                "project_runtime",
            ]
        );
    }

    #[test]
    fn report_ok_is_true_when_all_checks_pass() {
        let report = DiagnosticsReport {
            ok: true,
            generated_at_ms: 1,
            checks: vec![
                DiagnosticCheck::pass("a", "A", "ok"),
                DiagnosticCheck::pass("b", "B", "ok"),
            ],
        };
        assert!(report.ok);
    }

    #[test]
    fn report_ok_is_false_when_any_check_warns_or_fails() {
        let warn_report = DiagnosticsReport {
            ok: false,
            generated_at_ms: 1,
            checks: vec![
                DiagnosticCheck::pass("a", "A", "ok"),
                DiagnosticCheck::warn("b", "B", "maybe"),
            ],
        };
        assert!(!warn_report.ok);

        let fail_report = DiagnosticsReport {
            ok: false,
            generated_at_ms: 1,
            checks: vec![
                DiagnosticCheck::pass("a", "A", "ok"),
                DiagnosticCheck::fail("b", "B", "bad"),
            ],
        };
        assert!(!fail_report.ok);
    }

    // ── Check serialization ───────────────────────────────────────────────

    #[test]
    fn diagnostic_check_serializes_camelcase_and_omits_optional_fields() {
        let check = DiagnosticCheck::pass("test_id", "Test Label", "All good.");
        let json = serde_json::to_value(&check).unwrap();
        assert_eq!(json["id"], "test_id");
        assert_eq!(json["label"], "Test Label");
        assert_eq!(json["status"], "pass");
        assert_eq!(json["message"], "All good.");
        assert!(json.get("detail").is_none());
        assert!(json.get("remediation").is_none());
    }

    #[test]
    fn diagnostic_check_with_detail_and_remediation_serializes_both() {
        let check = DiagnosticCheck::fail("err", "Error", "Something broke.")
            .with_detail(serde_json::json!({"code": 42}))
            .with_remediation("Try restarting.");
        let json = serde_json::to_value(&check).unwrap();
        assert_eq!(json["status"], "fail");
        assert_eq!(json["detail"]["code"], 42);
        assert_eq!(json["remediation"], "Try restarting.");
    }

    // ── Config check shape ────────────────────────────────────────────────

    #[test]
    fn config_check_uses_correct_id_and_label() {
        let check = check_config_key_presence();
        assert_eq!(check.id, "config_settings");
        assert_eq!(check.label, "Config / API keys");
        // In test environment there are no stored keys, but known providers
        // are always reported (deepseek, anthropic, openai, openrouter).
        // The check should produce a valid report regardless.
        assert!(!check.message.is_empty());
    }

    // ── Snapshot check shape ──────────────────────────────────────────────

    #[test]
    fn snapshot_check_with_no_snapshots_reports_pass() {
        // Use a temp dir as HOME so no real snapshots are found
        let root = temp_root("snap-empty");
        fs::create_dir_all(&root).unwrap();
        // Ensure no sessions dir exists
        let _ = fs::remove_dir_all(root.join(".forge").join("sessions"));

        let check = check_session_snapshots_isolated(&root);
        assert_eq!(check.id, "session_snapshots");
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.message.contains("No session snapshots"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_check_reports_corrupt_count() {
        let root = temp_root("snap-corr");
        let sessions_dir = root.join(".forge").join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();

        // Write one valid snapshot
        let valid = crate::agent::snapshot::AgentSessionSnapshot::new(
            "session-ok".into(),
            "deepseek".into(),
            "deepseek-v4".into(),
            "/tmp".into(),
            vec![],
            None,
            None,
        );
        let mut valid = valid;
        valid.updated_at_ms = 1000;
        fs::write(
            sessions_dir.join("session-ok.json"),
            serde_json::to_string(&valid).unwrap(),
        )
        .unwrap();

        // Write one corrupted file
        fs::write(sessions_dir.join("corrupt.json"), "{ broken").unwrap();

        // Write a non-JSON file (should be ignored)
        fs::write(sessions_dir.join("readme.txt"), "hello").unwrap();

        let check = check_session_snapshots_isolated(&root);
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("1 readable snapshot"));
        assert!(check.message.contains("1 corrupted"));
        let detail = check.detail.unwrap();
        assert_eq!(detail["total_snapshots"], 1);
        assert_eq!(detail["corrupt_snapshots"], 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_check_with_all_corrupt_reports_fail() {
        let root = temp_root("snap-all-corr");
        let sessions_dir = root.join(".forge").join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();

        fs::write(sessions_dir.join("broken.json"), "{ nope").unwrap();

        let check = check_session_snapshots_isolated(&root);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.message.contains("corrupted"));

        let _ = fs::remove_dir_all(&root);
    }

    // ── App metadata check shape ──────────────────────────────────────────

    #[test]
    fn app_metadata_check_with_no_file_reports_pass() {
        let root = temp_root("meta-empty");
        fs::create_dir_all(&root).unwrap();
        // No app-state.json

        let check = check_app_metadata_isolated(&root);
        assert_eq!(check.id, "app_metadata");
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.message.contains("not present"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn app_metadata_check_with_corrupted_file_reports_fail() {
        let root = temp_root("meta-corr");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("app-state.json"), "{ not json").unwrap();

        let check = check_app_metadata_isolated(&root);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.message.contains("corrupted"));

        let _ = fs::remove_dir_all(&root);
    }

    // ── Log directory check shape ─────────────────────────────────────────

    #[test]
    fn log_directory_check_uses_correct_id() {
        let root = temp_root("log-dir");
        let forge_dir = root.join(".forge");
        fs::create_dir_all(&forge_dir).unwrap();
        // Create a log file so the check reports Pass
        fs::write(forge_dir.join("app.log"), b"test log entry\n").unwrap();

        let check = check_log_directory_isolated(&root);
        assert_eq!(check.id, "log_directory");
        assert_eq!(check.status, CheckStatus::Pass);

        let _ = fs::remove_dir_all(&root);
    }

    // ── Capability inventory check shape ──────────────────────────────────

    #[test]
    fn capability_check_without_data_reports_warn() {
        let check = check_capability_inventory(None);
        assert_eq!(check.id, "capability_inventory");
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("not available"));
        assert!(check.remediation.is_some());
    }

    #[test]
    fn capability_check_with_empty_list_reports_warn() {
        let check = check_capability_inventory(Some(vec![]));
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("empty"));
    }

    #[test]
    fn capability_check_with_data_reports_pass() {
        let caps = vec![
            CapabilitySummary {
                id: "tool-1".into(),
                name: "read_file".into(),
                kind: "tool".into(),
                enabled: true,
                status: None,
                status_message: None,
            },
            CapabilitySummary {
                id: "skill-1".into(),
                name: "My Skill".into(),
                kind: "skill".into(),
                enabled: false,
                status: None,
                status_message: None,
            },
        ];
        let check = check_capability_inventory(Some(caps));
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.message.contains("2 total"));
        assert!(check.message.contains("1 enabled"));
        let detail = check.detail.unwrap();
        assert_eq!(detail["total"], 2);
        assert_eq!(detail["enabled"], 1);
    }

    #[test]
    fn capability_check_with_unhealthy_items_reports_warn() {
        let caps = vec![
            CapabilitySummary {
                id: "tool-1".into(),
                name: "read_file".into(),
                kind: "tool".into(),
                enabled: true,
                status: Some("healthy".into()),
                status_message: None,
            },
            CapabilitySummary {
                id: "mcp:broken".into(),
                name: "Broken MCP".into(),
                kind: "mcp_server".into(),
                enabled: true,
                status: Some("unavailable".into()),
                status_message: Some("Connection refused".into()),
            },
            CapabilitySummary {
                id: "mcp:slow".into(),
                name: "Slow MCP".into(),
                kind: "mcp_server".into(),
                enabled: true,
                status: Some("warning".into()),
                status_message: Some("High latency".into()),
            },
        ];
        let check = check_capability_inventory(Some(caps));
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("3 total"));
        assert!(check.message.contains("2 unhealthy"));
        let detail = check.detail.unwrap();
        assert_eq!(detail["unhealthy_count"], 2);
        // Should list both unhealthy items
        let unhealthy = detail["unhealthy_items"].as_array().unwrap();
        assert!(unhealthy.iter().any(|n| n == "Broken MCP"));
        assert!(unhealthy.iter().any(|n| n == "Slow MCP"));
    }

    #[test]
    fn capability_check_with_all_healthy_reports_pass() {
        let caps = vec![
            CapabilitySummary {
                id: "tool-1".into(),
                name: "read_file".into(),
                kind: "tool".into(),
                enabled: true,
                status: Some("healthy".into()),
                status_message: None,
            },
            CapabilitySummary {
                id: "skill-1".into(),
                name: "My Skill".into(),
                kind: "skill".into(),
                enabled: true,
                status: Some("healthy".into()),
                status_message: None,
            },
        ];
        let check = check_capability_inventory(Some(caps));
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(!check.message.contains("unhealthy"));
    }

    // ── Project runtime check shape ───────────────────────────────────────

    #[test]
    fn project_runtime_check_without_metadata_reports_warn() {
        let root = temp_root("rt-empty");
        fs::create_dir_all(&root).unwrap();

        let check = check_project_runtime_isolated(&root);
        assert_eq!(check.id, "project_runtime");
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("No working directory"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn project_runtime_check_resolves_active_workspace_id_to_path() {
        let root = temp_root("rt-active-id");
        let workspace = temp_root("rt-active-workspace");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("package.json"), "{}\n").unwrap();
        fs::write(
            root.join("app-state.json"),
            serde_json::json!({
                "workspaces": [{
                    "id": "workspace-1",
                    "name": "Workspace",
                    "path": workspace.to_string_lossy(),
                    "lastOpenedAt": 1
                }],
                "activeWorkspaceId": "workspace-1"
            })
            .to_string(),
        )
        .unwrap();

        let check = check_project_runtime_isolated(&root);
        assert_eq!(check.status, CheckStatus::Pass);
        assert_eq!(
            check.detail.as_ref().unwrap()["working_dir"],
            workspace.to_string_lossy().as_ref()
        );

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&workspace);
    }

    // ── Complete report JSON shape ────────────────────────────────────────

    #[test]
    fn full_report_serializes_camelcase() {
        let report = run_diagnostics_basic();
        let json = serde_json::to_value(&report).unwrap();
        assert!(json["ok"].as_bool().is_some());
        assert!(json["generatedAtMs"].is_u64());
        assert!(json["checks"].is_array());
    }

    // ── Isolated check variants (use custom data dir) ─────────────────────

    /// Run `check_session_snapshots` against a custom data root.
    fn check_session_snapshots_isolated(root: &std::path::Path) -> DiagnosticCheck {
        isolated_forge_check(root, |_| check_session_snapshots_inner(root))
    }

    fn check_app_metadata_isolated(root: &std::path::Path) -> DiagnosticCheck {
        isolated_forge_check(root, |_| check_app_metadata_inner(root))
    }

    fn check_log_directory_isolated(root: &std::path::Path) -> DiagnosticCheck {
        isolated_forge_check(root, |_| check_log_directory_inner(root))
    }

    fn check_project_runtime_isolated(root: &std::path::Path) -> DiagnosticCheck {
        isolated_forge_check(root, |_| check_project_runtime_inner(root))
    }

    fn isolated_forge_check<F>(root: &std::path::Path, f: F) -> DiagnosticCheck
    where
        F: FnOnce(&std::path::Path) -> DiagnosticCheck,
    {
        f(root)
    }

    // ── Inner implementations that take an explicit root path ─────────────

    fn check_session_snapshots_inner(root: &std::path::Path) -> DiagnosticCheck {
        let snapshots_dir = root.join(".forge").join("sessions");

        let (total_files, corrupt_count, readable_snapshots) = if snapshots_dir.exists() {
            match std::fs::read_dir(&snapshots_dir) {
                Ok(entries) => {
                    let json_files: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            e.path().extension().and_then(|ext| ext.to_str()) == Some("json")
                        })
                        .collect();
                    let total = json_files.len();

                    let mut corrupt = 0u64;
                    let mut snapshots = Vec::new();
                    for entry in &json_files {
                        match std::fs::read_to_string(entry.path()) {
                            Ok(json) => match serde_json::from_str::<
                                crate::agent::snapshot::AgentSessionSnapshot,
                            >(&json)
                            {
                                Ok(snapshot) => snapshots.push(snapshot),
                                Err(_) => corrupt += 1,
                            },
                            Err(_) => corrupt += 1,
                        }
                    }
                    (total, corrupt, snapshots)
                }
                Err(_) => (0, 0, Vec::new()),
            }
        } else {
            (0, 0, Vec::new())
        };

        let now = now_ms();
        let snapshot_count = readable_snapshots.len();
        let newest_age_secs = readable_snapshots
            .iter()
            .map(|s| {
                let age_ms = now.saturating_sub(s.updated_at_ms);
                age_ms / 1000
            })
            .min();

        let detail = serde_json::json!({
            "total_snapshots": snapshot_count,
            "corrupt_snapshots": corrupt_count,
            "total_json_files": total_files,
            "newest_age_secs": newest_age_secs,
        });

        if total_files == 0 {
            DiagnosticCheck::pass(
                "session_snapshots",
                "Session snapshots",
                "No session snapshots found (fresh install or all sessions cleaned).",
            )
            .with_detail(detail)
        } else if corrupt_count > 0 && snapshot_count == 0 {
            DiagnosticCheck::fail(
                "session_snapshots",
                "Session snapshots",
                format!(
                    "All {total_files} snapshot file{} are corrupted.",
                    if total_files == 1 { "" } else { "s" }
                ),
            )
            .with_detail(detail)
            .with_remediation(
                "Corrupted snapshots cannot be recovered. Delete them from the sessions directory to start fresh.",
            )
        } else if corrupt_count > 0 {
            DiagnosticCheck::warn(
                "session_snapshots",
                "Session snapshots",
                format!(
                    "{snapshot_count} readable snapshot{}, {corrupt_count} corrupted.",
                    if snapshot_count == 1 { "" } else { "s" }
                ),
            )
            .with_detail(detail)
            .with_remediation(
                "Corrupted snapshots will be skipped during restore. Delete the corrupted files to silence this warning.",
            )
        } else if let Some(age_secs) = newest_age_secs {
            if age_secs > 86_400 {
                DiagnosticCheck::warn(
                    "session_snapshots",
                    "Session snapshots",
                    format!(
                        "{snapshot_count} snapshot{} — newest is {:.1} hours old.",
                        if snapshot_count == 1 { "" } else { "s" },
                        age_secs as f64 / 3600.0
                    ),
                )
                .with_detail(detail)
            } else {
                DiagnosticCheck::pass(
                    "session_snapshots",
                    "Session snapshots",
                    format!(
                        "{snapshot_count} snapshot{} — newest is {} seconds old.",
                        if snapshot_count == 1 { "" } else { "s" },
                        age_secs
                    ),
                )
                .with_detail(detail)
            }
        } else {
            DiagnosticCheck::pass(
                "session_snapshots",
                "Session snapshots",
                format!(
                    "{snapshot_count} snapshot{} readable.",
                    if snapshot_count == 1 { "" } else { "s" }
                ),
            )
            .with_detail(detail)
        }
    }

    fn check_app_metadata_inner(root: &std::path::Path) -> DiagnosticCheck {
        let metadata_path = root.join("app-state.json");
        if !metadata_path.exists() {
            DiagnosticCheck::pass(
                "app_metadata",
                "App metadata",
                "App metadata file not present (fresh install).",
            )
            .with_detail(serde_json::json!({"path": metadata_path.to_string_lossy()}))
        } else {
            match std::fs::read_to_string(&metadata_path) {
                Ok(json) => match serde_json::from_str::<crate::app_metadata::AppMetadata>(&json) {
                    Ok(metadata) => {
                        let workspace_count = metadata.workspaces.len();
                        DiagnosticCheck::pass(
                            "app_metadata",
                            "App metadata",
                            format!(
                                "App metadata readable — {workspace_count} workspace{}.",
                                if workspace_count == 1 { "" } else { "s" }
                            ),
                        )
                        .with_detail(serde_json::json!({
                            "path": metadata_path.to_string_lossy(),
                            "workspace_count": workspace_count,
                            "has_active_workspace": metadata.active_workspace_id.is_some(),
                            "has_active_session": metadata.active_session_id.is_some(),
                        }))
                    }
                    Err(e) => DiagnosticCheck::fail(
                        "app_metadata",
                        "App metadata",
                        format!("App metadata is corrupted: {e}"),
                    )
                    .with_detail(serde_json::json!({"path": metadata_path.to_string_lossy()}))
                    .with_remediation("Delete the app-state.json file to start fresh."),
                },
                Err(e) => DiagnosticCheck::fail(
                    "app_metadata",
                    "App metadata",
                    format!("Cannot read app metadata: {e}"),
                )
                .with_detail(serde_json::json!({"path": metadata_path.to_string_lossy()})),
            }
        }
    }

    fn check_log_directory_inner(root: &std::path::Path) -> DiagnosticCheck {
        let forge_dir = root.join(".forge");
        let data_dir_exists = forge_dir.exists();
        let data_dir_writable = if data_dir_exists {
            can_write_marker_file(&forge_dir)
        } else {
            std::fs::create_dir_all(&forge_dir).is_ok()
        };

        let log_path = forge_dir.join("app.log");
        let log_exists = log_path.exists();

        let detail = serde_json::json!({
            "data_dir": forge_dir.to_string_lossy(),
            "data_dir_exists": data_dir_exists,
            "data_dir_writable": data_dir_writable,
            "log_path": log_path.to_string_lossy(),
            "log_exists": log_exists,
        });

        if !data_dir_writable {
            DiagnosticCheck::fail(
                "log_directory",
                "Log / data directory",
                format!("Data directory {} is not writable.", forge_dir.display()),
            )
            .with_detail(detail)
            .with_remediation("Check file permissions on ~/.forge/.")
        } else if !data_dir_exists {
            DiagnosticCheck::warn(
                "log_directory",
                "Log / data directory",
                "Data directory created during check (was missing).",
            )
            .with_detail(detail)
        } else if !log_exists {
            DiagnosticCheck::warn(
                "log_directory",
                "Log / data directory",
                "Log file not yet created (no sessions have run).",
            )
            .with_detail(detail)
        } else {
            DiagnosticCheck::pass(
                "log_directory",
                "Log / data directory",
                "Data directory exists and is writable; log file present.",
            )
            .with_detail(detail)
        }
    }

    fn check_project_runtime_inner(root: &std::path::Path) -> DiagnosticCheck {
        let metadata_path = root.join("app-state.json");
        let metadata: Option<crate::app_metadata::AppMetadata> =
            std::fs::read_to_string(&metadata_path)
                .ok()
                .and_then(|json| serde_json::from_str(&json).ok());

        let active_workspace = metadata.as_ref().and_then(configured_workspace_path);

        match active_workspace {
            Some(working_dir) => {
                let pkg_json_path = std::path::Path::new(working_dir).join("package.json");
                let has_package_json = pkg_json_path.exists();
                let detail = serde_json::json!({
                    "working_dir": working_dir,
                    "has_package_json": has_package_json,
                });

                if has_package_json {
                    DiagnosticCheck::pass(
                        "project_runtime",
                        "Project runtime",
                        format!("Working directory '{}' has a package.json.", working_dir),
                    )
                    .with_detail(detail)
                } else {
                    DiagnosticCheck::warn(
                        "project_runtime",
                        "Project runtime",
                        format!(
                            "Working directory '{}' has no package.json — runtime status unavailable.",
                            working_dir
                        ),
                    )
                    .with_detail(detail)
                }
            }
            None => DiagnosticCheck::warn(
                "project_runtime",
                "Project runtime",
                "No working directory configured — cannot check project runtime.",
            )
            .with_remediation(
                "Open a project workspace in Forge to enable project runtime checks.",
            ),
        }
    }
}
