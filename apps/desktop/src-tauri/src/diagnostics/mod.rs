//! Diagnostics / doctor module — runtime health checks for Forge desktop.
//!
//! Produces a structured `DiagnosticsReport` with ordered checks that can be
//! serialized to JSON for the Tauri IPC layer and the CLI doctor command.
//!
//! Sub-modules:
//! - `watchdog` — session event tracker and stale-session health alert emission.

pub mod repair;
pub mod update_repair;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_action_id: Option<String>,
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
            repair_action_id: None,
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
            repair_action_id: None,
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
            repair_action_id: None,
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

    pub fn with_repair_action_id(mut self, repair_action_id: impl Into<String>) -> Self {
        self.repair_action_id = Some(repair_action_id.into());
        self
    }
}

// ── Diagnostics runner ────────────────────────────────────────────────────

/// Run all health checks and return a report.
///
/// `capabilities` is optional — when `Some`, the tool-inventory check can
/// report real data; when `None`, it emits a `Warn` with a TODO message.
pub fn run_diagnostics(capabilities: Option<Vec<CapabilitySummary>>) -> DiagnosticsReport {
    let store = crate::credential_store::system_credential_store();
    run_diagnostics_with_store(capabilities, store.as_ref())
}

pub fn run_diagnostics_with_store(
    capabilities: Option<Vec<CapabilitySummary>>,
    store: &dyn crate::credential_store::CredentialStore,
) -> DiagnosticsReport {
    let checks: Vec<DiagnosticCheck> = vec![
        check_config_key_presence_with_store(store),
        check_session_snapshots(),
        check_session_journal_parity(),
        check_a2a_ledgers(),
        check_app_metadata(),
        check_log_directory(),
        check_gateway_service_status(),
        update_repair::check_update_repair_status(),
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
    let store = crate::credential_store::system_credential_store();
    check_config_key_presence_with_store(store.as_ref())
}

pub fn check_config_key_presence_with_store(
    store: &dyn crate::credential_store::CredentialStore,
) -> DiagnosticCheck {
    match crate::settings::Settings::load().key_status(store) {
        key_statuses if key_statuses.is_empty() => {
            DiagnosticCheck::warn(
                "config_settings",
                "Config / API keys",
                "No API keys configured for any provider.",
            )
            .with_remediation("Add an API key in Settings → Models or set an environment variable like DEEPSEEK_API_KEY.")
        }
        key_statuses => {
            let set_count = key_statuses
                .iter()
                .filter(|key| key.configured && key.status == "available")
                .count();
            let total = key_statuses.len();
            let details: Vec<serde_json::Value> = key_statuses
                .iter()
                .map(|k| {
                    serde_json::json!({
                        "provider": k.provider,
                        "configured": k.configured,
                        "source": k.source,
                        "status": k.status,
                        "error": k.error,
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

/// Check that persisted A2A task ledgers are readable and summarize task state.
pub fn check_a2a_ledgers() -> DiagnosticCheck {
    check_a2a_ledgers_at(&forge_data_dir())
}

fn check_a2a_ledgers_at(data_dir: &Path) -> DiagnosticCheck {
    let a2a_dir = data_dir.join("a2a");
    let list = match crate::agent::a2a::ledger::list_session_projections_at(data_dir) {
        Ok(list) => list,
        Err(error) => {
            return DiagnosticCheck::fail(
                "a2a_ledger",
                "A2A task ledger",
                format!("Cannot read A2A ledger directory: {error}"),
            )
            .with_detail(serde_json::json!({
                "path": a2a_dir.to_string_lossy(),
            }))
            .with_remediation(
                "Check file permissions on ~/.forge/a2a or remove the unreadable directory.",
            );
        }
    };

    let readable_count = list.states.len();
    let corrupt_count = list.load_errors.len();
    let total_files = readable_count + corrupt_count;
    let total_tasks: usize = list
        .states
        .iter()
        .map(|state| state.projection.tasks.len())
        .sum();
    let running_count: usize = list
        .states
        .iter()
        .map(|state| state.projection.running_count)
        .sum();
    let failed_count: usize = list
        .states
        .iter()
        .map(|state| state.projection.failed_count)
        .sum();
    let interrupted_count: usize = list
        .states
        .iter()
        .map(|state| state.projection.interrupted_count)
        .sum();

    let detail = serde_json::json!({
        "path": a2a_dir.to_string_lossy(),
        "total_ledgers": readable_count,
        "corrupt_ledgers": corrupt_count,
        "total_json_files": total_files,
        "total_tasks": total_tasks,
        "running_tasks": running_count,
        "failed_tasks": failed_count,
        "interrupted_tasks": interrupted_count,
        "load_errors": list.load_errors,
    });

    if total_files == 0 {
        DiagnosticCheck::pass(
            "a2a_ledger",
            "A2A task ledger",
            "No A2A ledgers found (no subagent tasks have been persisted yet).",
        )
        .with_detail(detail)
    } else if corrupt_count > 0 && readable_count == 0 {
        DiagnosticCheck::fail(
            "a2a_ledger",
            "A2A task ledger",
            format!(
                "All {total_files} A2A ledger file{} are corrupted.",
                if total_files == 1 { "" } else { "s" }
            ),
        )
        .with_detail(detail)
        .with_remediation(
            "Corrupted A2A ledgers cannot be used for task recovery. Inspect or delete the files in ~/.forge/a2a.",
        )
        .with_repair_action_id("clear_a2a_ledger_cache")
    } else if corrupt_count > 0 {
        DiagnosticCheck::warn(
            "a2a_ledger",
            "A2A task ledger",
            format!(
                "{readable_count} readable A2A ledger{}, {corrupt_count} corrupted.",
                if readable_count == 1 { "" } else { "s" }
            ),
        )
        .with_detail(detail)
        .with_remediation(
            "Corrupted A2A ledgers are skipped. Delete or repair the affected files in ~/.forge/a2a.",
        )
        .with_repair_action_id("clear_a2a_ledger_cache")
    } else {
        DiagnosticCheck::pass(
            "a2a_ledger",
            "A2A task ledger",
            format!(
                "{readable_count} A2A ledger{} readable with {total_tasks} task{}.",
                if readable_count == 1 { "" } else { "s" },
                if total_tasks == 1 { "" } else { "s" }
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

/// Check whether the Forge Gateway platform service is installed and running.
pub fn check_gateway_service_status() -> DiagnosticCheck {
    gateway_service_check_from_snapshot(gateway_service_snapshot())
}

#[derive(Debug, Clone)]
struct GatewayServiceSnapshot {
    supported: bool,
    installed: bool,
    running: bool,
    status_message: String,
    backend: String,
    service_id: String,
    label: String,
    launch_domain: String,
    service_path: String,
    plist_path: String,
    log_path: String,
    error_log_path: String,
}

fn gateway_service_snapshot() -> GatewayServiceSnapshot {
    match crate::service::query_status_snapshot() {
        Ok(status) => gateway_service_snapshot_from_service_status(status),
        Err(error) => gateway_service_snapshot_from_service_status(
            crate::service::unavailable_status_snapshot(error),
        ),
    }
}

fn gateway_service_snapshot_from_service_status(
    status: crate::service::ServiceStatusSnapshot,
) -> GatewayServiceSnapshot {
    GatewayServiceSnapshot {
        supported: status.supported,
        installed: status.installed,
        running: status.running,
        status_message: status.status_message,
        backend: status.backend,
        service_id: status.service_id,
        label: status.label,
        launch_domain: status.launch_domain,
        service_path: status.service_path,
        plist_path: status.plist_path,
        log_path: status.log_path,
        error_log_path: status.error_log_path,
    }
}

fn gateway_service_check_from_snapshot(snapshot: GatewayServiceSnapshot) -> DiagnosticCheck {
    let detail = serde_json::json!({
        "supported": snapshot.supported,
        "installed": snapshot.installed,
        "running": snapshot.running,
        "backend": snapshot.backend,
        "service_id": snapshot.service_id,
        "label": snapshot.label,
        "launch_domain": snapshot.launch_domain,
        "service_path": snapshot.service_path,
        "plist_path": snapshot.plist_path,
        "log_path": snapshot.log_path,
        "error_log_path": snapshot.error_log_path,
        "status_message": snapshot.status_message,
    });

    if !snapshot.supported {
        return DiagnosticCheck::pass(
            "gateway_service",
            "Gateway service",
            "Gateway service management is not supported on this platform.",
        )
        .with_detail(detail);
    }

    if snapshot.installed && snapshot.running {
        DiagnosticCheck::pass(
            "gateway_service",
            "Gateway service",
            "Gateway service is installed and running.",
        )
        .with_detail(detail)
    } else if snapshot.installed {
        DiagnosticCheck::warn(
            "gateway_service",
            "Gateway service",
            "Gateway service is installed but not running.",
        )
        .with_detail(detail)
        .with_remediation(
            "Restart Gateway from Settings → General or run the restart_gateway repair action.",
        )
        .with_repair_action_id("restart_gateway")
    } else {
        DiagnosticCheck::warn(
            "gateway_service",
            "Gateway service",
            "Gateway service is not installed.",
        )
        .with_detail(detail)
        .with_remediation(
            "Enable autostart in Settings → General or run the reinstall_service repair action.",
        )
        .with_repair_action_id("reinstall_service")
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

// ── Session journal parity summary (Task 5) ────────────────────────────────

/// Maximum number of sessions classified per parity scan. Bounds the
/// worst-case cost of `check_session_journal_parity`: each classified session
/// may require a full journal replay (journals grow up to the 32 MiB
/// truncation threshold), so an unbounded scan of thousands of sessions could
/// block the diagnostics runner for seconds. Sessions beyond the cap are
/// counted in `sessions_total` but not classified.
const MAX_JOURNAL_PARITY_SCAN_SESSIONS: usize = 200;

/// Aggregate restore-parity counts over every durable session (snapshot
/// and/or mutation journal). Counts ONLY — conversation body text never
/// appears here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionJournalParitySummary {
    /// Durable sessions classified this scan (capped at
    /// `MAX_JOURNAL_PARITY_SCAN_SESSIONS`).
    pub sessions_scanned: u64,
    /// All durable sessions found, including any beyond the scan cap.
    pub sessions_total: u64,
    /// Snapshot and journal agree (or a legacy snapshot matches journal
    /// content).
    pub healthy_parity: u64,
    /// Same recorded sequence but content differs — today only reachable via
    /// the known `repair_message_history` parity gap, so it is reported, not
    /// alarmed on.
    pub diverged: u64,
    /// Journal committed events beyond the snapshot's recorded sequence.
    pub snapshot_behind: u64,
    /// Sessions with a snapshot but no journal events.
    pub snapshot_only: u64,
    /// Sessions restorable only from the journal (snapshot missing/corrupt).
    pub journal_only: u64,
    /// Journals with a torn final line (interrupted append).
    pub torn_final_line: u64,
    /// Journals with a corrupt interior line or unreplayable events.
    pub corrupt_interior: u64,
    /// Journals the restore selector quarantined.
    pub quarantined: u64,
}

/// Scan durable sessions under `~/.forge` and classify their restore parity.
/// Read-only: the selector is pure and quarantine renames only happen in the
/// restore path, never in diagnostics. Cost is bounded by
/// `MAX_JOURNAL_PARITY_SCAN_SESSIONS` full journal replays.
pub fn session_journal_parity_summary() -> SessionJournalParitySummary {
    session_journal_parity_summary_at(&forge_data_dir())
}

fn session_journal_parity_summary_at(root: &Path) -> SessionJournalParitySummary {
    use crate::agent::session_journal::SessionJournalStore;
    use crate::agent::snapshot::try_load_session_snapshot_at;
    use crate::ipc::session_lifecycle::{
        choose_session_restore_source, SessionJournalLoad, SessionParityStatus,
        SessionRestoreNotice,
    };

    let session_ids = durable_session_ids(root);
    let mut summary = SessionJournalParitySummary {
        sessions_total: session_ids.len() as u64,
        ..SessionJournalParitySummary::default()
    };
    for session_id in session_ids
        .into_iter()
        .take(MAX_JOURNAL_PARITY_SCAN_SESSIONS)
    {
        let snapshot = try_load_session_snapshot_at(root, &session_id);
        let journal = match SessionJournalStore::new(root.to_path_buf(), session_id.clone()) {
            Ok(store) => match store.load() {
                Ok(result) if result.events.is_empty() => Ok(None),
                Ok(result) => Ok(Some(SessionJournalLoad::from(result))),
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        let decision = choose_session_restore_source(snapshot, journal);
        summary.sessions_scanned += 1;
        match decision.parity {
            SessionParityStatus::Healthy => summary.healthy_parity += 1,
            SessionParityStatus::Diverged => summary.diverged += 1,
            SessionParityStatus::SnapshotBehind => summary.snapshot_behind += 1,
            SessionParityStatus::SnapshotOnly => summary.snapshot_only += 1,
            SessionParityStatus::JournalOnly => summary.journal_only += 1,
            SessionParityStatus::TornFinalLine => summary.torn_final_line += 1,
            SessionParityStatus::CorruptInterior => summary.corrupt_interior += 1,
        }
        if decision
            .notices
            .contains(&SessionRestoreNotice::JournalQuarantined)
        {
            summary.quarantined += 1;
        }
    }
    summary
}

/// Session ids that have either a snapshot file or a journal directory under
/// `<root>/sessions/`.
fn durable_session_ids(root: &Path) -> std::collections::BTreeSet<String> {
    let mut ids = std::collections::BTreeSet::new();
    let sessions_dir = root.join("sessions");
    let Ok(entries) = std::fs::read_dir(&sessions_dir) else {
        return ids;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    ids.insert(stem.to_string());
                }
            }
        } else if path.is_dir() {
            let has_journal = std::fs::read_dir(&path)
                .map(|mut entries| {
                    entries.any(|entry| {
                        entry
                            .map(|entry| {
                                entry.file_name().to_string_lossy().starts_with("mutations")
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if has_journal {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    ids.insert(name.to_string());
                }
            }
        }
    }
    ids
}

/// Check snapshot/journal restore parity across all durable sessions.
///
/// Cost note: classification replays each session's journal in memory
/// (journals grow up to the 32 MiB truncation threshold), so the scan is
/// capped at `MAX_JOURNAL_PARITY_SCAN_SESSIONS` sessions per run —
/// `sessions_total` in the detail still reports every durable session found.
pub fn check_session_journal_parity() -> DiagnosticCheck {
    check_session_journal_parity_at(&forge_data_dir())
}

fn check_session_journal_parity_at(root: &Path) -> DiagnosticCheck {
    let summary = session_journal_parity_summary_at(root);
    let detail = serde_json::to_value(&summary).unwrap_or(serde_json::Value::Null);
    if summary.sessions_scanned == 0 {
        return DiagnosticCheck::pass(
            "session_journal_parity",
            "Session journal parity",
            "No durable sessions found (fresh install or all sessions cleaned).",
        )
        .with_detail(detail);
    }
    if summary.corrupt_interior > 0 || summary.quarantined > 0 {
        return DiagnosticCheck::warn(
            "session_journal_parity",
            "Session journal parity",
            format!(
                "{} durable session{} scanned — {} journal{} quarantined for corrupt interior lines.",
                summary.sessions_scanned,
                if summary.sessions_scanned == 1 { "" } else { "s" },
                summary.quarantined,
                if summary.quarantined == 1 { "" } else { "s" },
            ),
        )
        .with_detail(detail)
        .with_remediation(
            "Quarantined journals are renamed aside automatically on the next restore and journaling restarts on a fresh generation; delete old ~/.forge/sessions/<id>/mutations.gen*.jsonl files to silence this warning.",
        );
    }
    DiagnosticCheck::pass(
        "session_journal_parity",
        "Session journal parity",
        format!(
            "{} durable session{} scanned — {} healthy parity, {} snapshot-behind, {} journal-only, {} torn-final.",
            summary.sessions_scanned,
            if summary.sessions_scanned == 1 { "" } else { "s" },
            summary.healthy_parity,
            summary.snapshot_behind,
            summary.journal_only,
            summary.torn_final_line,
        ),
    )
    .with_detail(detail)
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
    use crate::credential_store::MemoryCredentialStore;
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
        let store = MemoryCredentialStore::default();
        let report = run_diagnostics_with_store(None, &store);
        assert!(report.generated_at_ms > 0);
        // The checks must appear in a stable order
        let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "config_settings",
                "session_snapshots",
                "session_journal_parity",
                "a2a_ledger",
                "app_metadata",
                "log_directory",
                "gateway_service",
                "update_repair",
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

    #[test]
    fn diagnostic_check_with_repair_action_serializes_camelcase() {
        let check = DiagnosticCheck::warn("gateway_service", "Gateway service", "Needs attention.")
            .with_repair_action_id("restart_gateway");
        let json = serde_json::to_value(&check).unwrap();

        assert_eq!(json["repairActionId"], "restart_gateway");
    }

    // ── Config check shape ────────────────────────────────────────────────

    #[test]
    fn config_check_uses_correct_id_and_label() {
        let store = MemoryCredentialStore::default();
        let check = check_config_key_presence_with_store(&store);
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

    // ── A2A ledger check shape ───────────────────────────────────────────

    #[test]
    fn a2a_ledger_check_with_no_ledgers_reports_pass() {
        let root = temp_root("a2a-empty");
        fs::create_dir_all(&root).unwrap();

        let check = check_a2a_ledgers_isolated(&root);

        assert_eq!(check.id, "a2a_ledger");
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.message.contains("No A2A ledgers"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a2a_ledger_check_reports_corrupt_count_without_dropping_valid_ledgers() {
        use crate::agent::a2a::bus::AgentA2ABus;
        use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

        let root = temp_root("a2a-corrupt");
        let forge_dir = root.join(".forge");
        let a2a_dir = forge_dir.join("a2a");
        fs::create_dir_all(&a2a_dir).unwrap();

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect worker",
            "Read A2A state",
            10,
        );
        bus.complete_task(&task_id, "done", 20);
        crate::agent::a2a::ledger::save_session_ledger_at(&forge_dir, "session-ok", &bus)
            .expect("save valid ledger");
        fs::write(a2a_dir.join("session-bad.json"), "{ nope").unwrap();

        let check = check_a2a_ledgers_isolated(&root);

        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.message.contains("1 readable A2A ledger"));
        assert!(check.message.contains("1 corrupted"));
        let detail = check.detail.unwrap();
        assert_eq!(detail["total_ledgers"], 1);
        assert_eq!(detail["corrupt_ledgers"], 1);
        assert_eq!(detail["total_tasks"], 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a2a_ledger_check_with_all_corrupt_reports_fail() {
        let root = temp_root("a2a-all-corrupt");
        let a2a_dir = root.join(".forge").join("a2a");
        fs::create_dir_all(&a2a_dir).unwrap();
        fs::write(a2a_dir.join("session-bad.json"), "{ nope").unwrap();

        let check = check_a2a_ledgers_isolated(&root);

        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.message.contains("A2A ledger"));
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

    // ── Gateway service check ─────────────────────────────────────────────

    #[test]
    fn gateway_service_check_passes_when_installed_and_running() {
        let check = gateway_service_check_from_snapshot(GatewayServiceSnapshot {
            supported: true,
            installed: true,
            running: true,
            status_message: "Service 'com.forge.gateway' is running.".into(),
            backend: "launchd".into(),
            service_id: "com.forge.gateway".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "gui/123".into(),
            service_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            log_path: "/Users/test/.forge/logs/gateway.log".into(),
            error_log_path: "/Users/test/.forge/logs/gateway-error.log".into(),
        });

        assert_eq!(check.id, "gateway_service");
        assert_eq!(check.status, CheckStatus::Pass);
        assert_eq!(check.detail.as_ref().unwrap()["launch_domain"], "gui/123");
    }

    #[test]
    fn gateway_service_check_warns_when_installed_but_not_running() {
        let check = gateway_service_check_from_snapshot(GatewayServiceSnapshot {
            supported: true,
            installed: true,
            running: false,
            status_message: "Service 'com.forge.gateway' is installed but not running.".into(),
            backend: "launchd".into(),
            service_id: "com.forge.gateway".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "gui/123".into(),
            service_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            log_path: "/Users/test/.forge/logs/gateway.log".into(),
            error_log_path: "/Users/test/.forge/logs/gateway-error.log".into(),
        });

        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.remediation.as_deref().unwrap().contains("Restart"));
        assert_eq!(check.repair_action_id.as_deref(), Some("restart_gateway"));
    }

    #[test]
    fn gateway_service_check_warns_when_not_installed() {
        let check = gateway_service_check_from_snapshot(GatewayServiceSnapshot {
            supported: true,
            installed: false,
            running: false,
            status_message: "Service 'com.forge.gateway' is not installed.".into(),
            backend: "launchd".into(),
            service_id: "com.forge.gateway".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "gui/123".into(),
            service_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist".into(),
            log_path: "/Users/test/.forge/logs/gateway.log".into(),
            error_log_path: "/Users/test/.forge/logs/gateway-error.log".into(),
        });

        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check
            .remediation
            .as_deref()
            .unwrap()
            .contains("reinstall_service"));
        assert_eq!(check.repair_action_id.as_deref(), Some("reinstall_service"));
    }

    #[test]
    fn gateway_service_check_passes_on_unsupported_platforms() {
        let check = gateway_service_check_from_snapshot(GatewayServiceSnapshot {
            supported: false,
            installed: false,
            running: false,
            status_message: "Service management is only supported on macOS.".into(),
            backend: "unsupported".into(),
            service_id: "".into(),
            label: "com.forge.gateway".into(),
            launch_domain: "unsupported".into(),
            service_path: "".into(),
            plist_path: "".into(),
            log_path: "".into(),
            error_log_path: "".into(),
        });

        assert_eq!(check.status, CheckStatus::Pass);
        assert_eq!(check.detail.as_ref().unwrap()["supported"], false);
    }

    #[test]
    fn gateway_service_snapshot_preserves_cross_platform_service_status() {
        let snapshot =
            gateway_service_snapshot_from_service_status(crate::service::ServiceStatusSnapshot {
                supported: true,
                installed: true,
                running: false,
                message: "Gateway systemd user service is installed but not running.".into(),
                backend: "systemd".into(),
                service_id: "forge-gateway.service".into(),
                label: "forge-gateway.service".into(),
                launch_domain: "systemd-user".into(),
                service_path: "/home/alice/.config/systemd/user/forge-gateway.service".into(),
                plist_path: "/home/alice/.config/systemd/user/forge-gateway.service".into(),
                log_path: "/home/alice/.forge/logs/gateway.log".into(),
                error_log_path: "/home/alice/.forge/logs/gateway-error.log".into(),
                status_message: "Service 'forge-gateway.service' is not running: inactive".into(),
            });

        assert_eq!(snapshot.backend, "systemd");
        assert_eq!(snapshot.service_id, "forge-gateway.service");
        assert_eq!(
            snapshot.service_path,
            "/home/alice/.config/systemd/user/forge-gateway.service"
        );
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
        let store = MemoryCredentialStore::default();
        let report = run_diagnostics_with_store(None, &store);
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

    fn check_a2a_ledgers_isolated(root: &std::path::Path) -> DiagnosticCheck {
        isolated_forge_check(root, |_| check_a2a_ledgers_inner(root))
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

    fn check_a2a_ledgers_inner(root: &std::path::Path) -> DiagnosticCheck {
        check_a2a_ledgers_at(&root.join(".forge"))
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

// ── Task 5: session journal parity summary tests ────────────────────────────

#[cfg(test)]
mod journal_parity_tests {
    use super::*;
    use crate::adapters::base::ChatMessage;
    use crate::agent::session_journal::{
        SessionJournalStore, SessionMutation, SessionMutationEnvelope,
        SESSION_JOURNAL_SCHEMA_VERSION,
    };
    use crate::agent::session_projection::SessionProjection;
    use crate::agent::snapshot::AgentSessionSnapshot;
    use std::fs;

    fn journal_root(prefix: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forge-diag-journal-parity-{}-{}",
            prefix,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("sessions")).expect("sessions dir");
        root
    }

    fn envelope(session_id: &str, mutation: SessionMutation) -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: String::new(),
            session_id: session_id.to_string(),
            sequence: 0,
            created_at_ms: 1,
            mutation,
        }
    }

    /// Write a journal of one init event plus one message per text; returns the
    /// committed events.
    fn write_journal(
        root: &std::path::Path,
        session_id: &str,
        texts: &[&str],
    ) -> Vec<SessionMutationEnvelope> {
        let store = SessionJournalStore::new(root.to_path_buf(), session_id.to_string())
            .expect("journal store");
        store
            .append(envelope(
                session_id,
                SessionMutation::SessionInitialized {
                    provider: "deepseek".to_string(),
                    model: "deepseek-chat".to_string(),
                    working_dir: "/tmp/workspace".to_string(),
                },
            ))
            .expect("append init");
        for text in texts {
            store
                .append(envelope(
                    session_id,
                    SessionMutation::MessageAppended {
                        message: ChatMessage::user(text),
                    },
                ))
                .expect("append message");
        }
        store.load().expect("load").events
    }

    fn journal_path(root: &std::path::Path, session_id: &str) -> std::path::PathBuf {
        root.join("sessions")
            .join(session_id)
            .join("mutations.jsonl")
    }

    fn append_raw(root: &std::path::Path, session_id: &str, bytes: &[u8]) {
        use std::io::Write;
        let path = journal_path(root, session_id);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open journal");
        file.write_all(bytes).expect("append raw");
        file.sync_all().expect("sync");
    }

    fn snapshot_matching(
        events: &[SessionMutationEnvelope],
        journal_sequence: u64,
    ) -> AgentSessionSnapshot {
        let mut snapshot = SessionProjection::from_events(events)
            .expect("projection")
            .to_snapshot();
        snapshot.journal_sequence = journal_sequence;
        snapshot
    }

    fn save_snapshot(root: &std::path::Path, snapshot: &AgentSessionSnapshot) {
        fs::write(
            root.join("sessions")
                .join(format!("{}.json", snapshot.session_id)),
            serde_json::to_string(snapshot).expect("snapshot json"),
        )
        .expect("write snapshot");
    }

    #[test]
    fn journal_parity_summary_empty_root_is_all_zeros() {
        let root = journal_root("empty");

        let summary = session_journal_parity_summary_at(&root);

        assert_eq!(summary, SessionJournalParitySummary::default());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_parity_summary_counts_each_restore_category() {
        let root = journal_root("categories");

        // Healthy: snapshot and journal at the same sequence with equal content.
        let events = write_journal(&root, "s-healthy", &["alpha", "beta"]);
        save_snapshot(&root, &snapshot_matching(&events, 3));

        // Snapshot behind: journal has newer events than the snapshot recorded.
        let events = write_journal(&root, "s-behind", &["alpha", "beta"]);
        save_snapshot(&root, &snapshot_matching(&events[..2], 1));

        // Snapshot only: no journal at all.
        save_snapshot(
            &root,
            &AgentSessionSnapshot::new(
                "s-snap-only".to_string(),
                "deepseek".to_string(),
                "deepseek-chat".to_string(),
                "/tmp/workspace".to_string(),
                Vec::new(),
                None,
                None,
            ),
        );

        // Journal only: no snapshot file.
        write_journal(&root, "s-journal-only", &["alpha"]);

        // Corrupt interior: valid snapshot, journal has a garbage interior line.
        let events = write_journal(&root, "s-corrupt", &["alpha"]);
        append_raw(&root, "s-corrupt", b"this is not json\n");
        save_snapshot(&root, &snapshot_matching(&events, 2));

        // Torn final line: snapshot behind the valid prefix.
        let events = write_journal(&root, "s-torn", &["alpha", "beta"]);
        append_raw(&root, "s-torn", br#"{"schema_version":1"#);
        save_snapshot(&root, &snapshot_matching(&events[..2], 1));

        let summary = session_journal_parity_summary_at(&root);

        assert_eq!(summary.sessions_total, 6);
        assert_eq!(summary.sessions_scanned, 6);
        assert_eq!(summary.healthy_parity, 1);
        assert_eq!(summary.snapshot_behind, 1);
        assert_eq!(summary.snapshot_only, 1);
        assert_eq!(summary.journal_only, 1);
        assert_eq!(summary.corrupt_interior, 1);
        assert_eq!(summary.quarantined, 1);
        assert_eq!(summary.torn_final_line, 1);
        assert_eq!(summary.diverged, 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_parity_summary_never_contains_conversation_text() {
        let root = journal_root("privacy");
        let events = write_journal(&root, "s-private", &["top-secret-body-text"]);
        save_snapshot(&root, &snapshot_matching(&events, 2));

        let summary = session_journal_parity_summary_at(&root);
        let json = serde_json::to_string(&summary).expect("serialize summary");
        assert!(
            !json.contains("top-secret-body-text"),
            "diagnostics summary must never include conversation body text: {json}"
        );
        assert!(json.contains("healthyParity"), "counts only: {json}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_parity_check_warns_when_journal_quarantined() {
        let root = journal_root("warn");
        let events = write_journal(&root, "s-corrupt", &["alpha"]);
        append_raw(&root, "s-corrupt", b"not json\n");
        save_snapshot(&root, &snapshot_matching(&events, 2));

        let check = check_session_journal_parity_at(&root);

        assert_eq!(check.id, "session_journal_parity");
        assert_eq!(check.status, CheckStatus::Warn);
        let detail = check.detail.expect("detail");
        assert_eq!(detail["corruptInterior"], 1);
        assert_eq!(detail["quarantined"], 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_parity_check_passes_for_healthy_sessions() {
        let root = journal_root("pass");
        let events = write_journal(&root, "s-healthy", &["alpha"]);
        save_snapshot(&root, &snapshot_matching(&events, 2));

        let check = check_session_journal_parity_at(&root);

        assert_eq!(check.status, CheckStatus::Pass);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_parity_summary_caps_classified_sessions() {
        let root = journal_root("cap");
        for index in 0..(MAX_JOURNAL_PARITY_SCAN_SESSIONS + 5) {
            save_snapshot(
                &root,
                &AgentSessionSnapshot::new(
                    format!("s-cap-{index:03}"),
                    "deepseek".to_string(),
                    "deepseek-chat".to_string(),
                    "/tmp/workspace".to_string(),
                    Vec::new(),
                    None,
                    None,
                ),
            );
        }

        let summary = session_journal_parity_summary_at(&root);

        assert_eq!(
            summary.sessions_total,
            (MAX_JOURNAL_PARITY_SCAN_SESSIONS + 5) as u64
        );
        assert_eq!(
            summary.sessions_scanned,
            MAX_JOURNAL_PARITY_SCAN_SESSIONS as u64
        );
        assert_eq!(
            summary.snapshot_only,
            MAX_JOURNAL_PARITY_SCAN_SESSIONS as u64
        );
        let _ = fs::remove_dir_all(&root);
    }
}
