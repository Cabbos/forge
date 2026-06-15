//! Self-healing repair actions triggered from diagnostics.
//!
//! Each action is idempotent and safe to run multiple times.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// A repair action the user or system can trigger.
#[derive(Debug, Clone, Serialize)]
pub struct RepairAction {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

/// Available repair actions.
pub const REPAIR_ACTIONS: &[RepairAction] = &[
    RepairAction {
        id: "restart_gateway",
        label: "重启 Gateway",
        description: "停止并重新安装 Gateway 后台服务",
    },
    RepairAction {
        id: "clear_snapshot_cache",
        label: "清除快照缓存",
        description: "删除所有会话快照文件，下次启动将重新开始",
    },
    RepairAction {
        id: "clear_a2a_ledger_cache",
        label: "清除 A2A 任务账本",
        description: "删除所有持久化子任务状态，下次启动将重新建立",
    },
    RepairAction {
        id: "reinstall_service",
        label: "重新安装服务",
        description: "卸载并重新安装 launchd 服务",
    },
    RepairAction {
        id: "clear_logs",
        label: "清除日志",
        description: "归档并清空当前日志文件",
    },
    RepairAction {
        id: "check_config",
        label: "检查配置",
        description: "验证 ~/.forge/config.json 可读且格式正确",
    },
];

/// Result of running a repair action.
#[derive(Debug, Clone, Serialize)]
pub struct RepairResult {
    pub action_id: String,
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<RepairVerification>,
}

/// Post-action verification attached to a repair result.
#[derive(Debug, Clone, Serialize)]
pub struct RepairVerification {
    pub label: String,
    pub ok: bool,
    pub message: String,
}

/// Run a specific repair action by id.  Returns the result.
pub fn run_repair(action_id: &str) -> RepairResult {
    match action_id {
        "restart_gateway" => restart_gateway(),
        "clear_snapshot_cache" => clear_snapshot_cache(),
        "clear_a2a_ledger_cache" => clear_a2a_ledger_cache(),
        "reinstall_service" => reinstall_service(),
        "clear_logs" => clear_logs(),
        "check_config" => check_config(),
        _ => RepairResult {
            action_id: action_id.to_string(),
            success: false,
            message: format!("Unknown repair action: {action_id}"),
            verification: None,
        },
    }
}

fn restart_gateway() -> RepairResult {
    let restart_result = crate::service::restart().map_err(|e| format!("restart failed: {e}"));
    let status_result = status_after_gateway_repair(&restart_result);
    gateway_service_repair_result("restart_gateway", restart_result, status_result)
}

fn status_after_gateway_repair(repair_result: &Result<String, String>) -> Result<String, String> {
    if repair_result.is_err() {
        return Err("verification skipped because repair command failed".to_string());
    }

    crate::service::status()
}

fn gateway_service_repair_result(
    action_id: &str,
    repair_result: Result<String, String>,
    status_result: Result<String, String>,
) -> RepairResult {
    match repair_result {
        Err(error) => RepairResult {
            action_id: action_id.into(),
            success: false,
            message: error,
            verification: None,
        },
        Ok(message) => match status_result {
            Ok(status_message) if gateway_status_message_is_running(&status_message) => {
                RepairResult {
                    action_id: action_id.into(),
                    success: true,
                    message,
                    verification: Some(RepairVerification {
                        label: "Gateway service".into(),
                        ok: true,
                        message: status_message,
                    }),
                }
            }
            Ok(status_message) => RepairResult {
                action_id: action_id.into(),
                success: false,
                message: format!("Gateway repair verification failed: {status_message}"),
                verification: Some(RepairVerification {
                    label: "Gateway service".into(),
                    ok: false,
                    message: status_message,
                }),
            },
            Err(error) => RepairResult {
                action_id: action_id.into(),
                success: false,
                message: format!("Gateway repair verification failed: {error}"),
                verification: Some(RepairVerification {
                    label: "Gateway service".into(),
                    ok: false,
                    message: error,
                }),
            },
        },
    }
}

fn gateway_status_message_is_running(message: &str) -> bool {
    message.contains(" is running.")
}

fn clear_snapshot_cache() -> RepairResult {
    clear_snapshot_cache_at(&snapshot_cache_dir())
}

fn clear_snapshot_cache_at(snapshots_dir: &Path) -> RepairResult {
    if !snapshots_dir.exists() {
        return RepairResult {
            action_id: "clear_snapshot_cache".into(),
            success: true,
            message: "No snapshot cache to clear.".into(),
            verification: Some(verify_cache_dir_empty("Snapshot cache", snapshots_dir)),
        };
    }

    match std::fs::remove_dir_all(snapshots_dir) {
        Ok(()) => {
            let _ = std::fs::create_dir_all(snapshots_dir);
            let verification = verify_cache_dir_empty("Snapshot cache", snapshots_dir);
            RepairResult {
                action_id: "clear_snapshot_cache".into(),
                success: verification.ok,
                message: "Snapshot cache cleared.".into(),
                verification: Some(verification),
            }
        }
        Err(e) => RepairResult {
            action_id: "clear_snapshot_cache".into(),
            success: false,
            message: format!("Failed: {e}"),
            verification: None,
        },
    }
}

fn clear_a2a_ledger_cache() -> RepairResult {
    clear_a2a_ledger_cache_at(&a2a_ledger_cache_dir())
}

fn clear_a2a_ledger_cache_at(ledger_dir: &Path) -> RepairResult {
    if !ledger_dir.exists() {
        return RepairResult {
            action_id: "clear_a2a_ledger_cache".into(),
            success: true,
            message: "No A2A ledger cache to clear.".into(),
            verification: Some(verify_cache_dir_empty("A2A ledger cache", ledger_dir)),
        };
    }

    match std::fs::remove_dir_all(ledger_dir) {
        Ok(()) => {
            let _ = std::fs::create_dir_all(ledger_dir);
            let verification = verify_cache_dir_empty("A2A ledger cache", ledger_dir);
            RepairResult {
                action_id: "clear_a2a_ledger_cache".into(),
                success: verification.ok,
                message: "A2A ledger cache cleared.".into(),
                verification: Some(verification),
            }
        }
        Err(e) => RepairResult {
            action_id: "clear_a2a_ledger_cache".into(),
            success: false,
            message: format!("Failed: {e}"),
            verification: None,
        },
    }
}

fn verify_cache_dir_empty(label: &str, dir: &Path) -> RepairVerification {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                RepairVerification {
                    label: label.into(),
                    ok: true,
                    message: format!("{} is empty.", dir.display()),
                }
            } else {
                RepairVerification {
                    label: label.into(),
                    ok: false,
                    message: format!("{} still contains files.", dir.display()),
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => RepairVerification {
            label: label.into(),
            ok: true,
            message: format!("{} does not exist.", dir.display()),
        },
        Err(error) => RepairVerification {
            label: label.into(),
            ok: false,
            message: format!("Cannot inspect {}: {error}", dir.display()),
        },
    }
}

fn snapshot_cache_dir() -> PathBuf {
    snapshot_cache_dir_for_home(home_dir())
}

fn snapshot_cache_dir_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref().join(".forge").join("sessions")
}

fn a2a_ledger_cache_dir() -> PathBuf {
    a2a_ledger_cache_dir_for_home(home_dir())
}

fn a2a_ledger_cache_dir_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref().join(".forge").join("a2a")
}

fn reinstall_service() -> RepairResult {
    // Uninstall then install.
    let _ = crate::service::uninstall();
    let reinstall_result = crate::service::install().map_err(|e| format!("reinstall failed: {e}"));
    let status_result = status_after_gateway_repair(&reinstall_result);
    gateway_service_repair_result("reinstall_service", reinstall_result, status_result)
}

fn clear_logs() -> RepairResult {
    let log_path = home_dir().join(".forge").join("logs").join("forge.log");

    if !log_path.exists() {
        return RepairResult {
            action_id: "clear_logs".into(),
            success: true,
            message: "No log file to clear.".into(),
            verification: None,
        };
    }

    // Rotate: rename current to .old, start fresh.
    let archived = log_path.with_extension("log.old");
    match std::fs::rename(&log_path, &archived) {
        Ok(()) => RepairResult {
            action_id: "clear_logs".into(),
            success: true,
            message: "Log file archived and cleared.".into(),
            verification: None,
        },
        Err(e) => RepairResult {
            action_id: "clear_logs".into(),
            success: false,
            message: format!("Failed: {e}"),
            verification: None,
        },
    }
}

fn check_config() -> RepairResult {
    let config_path = home_dir().join(".forge").join("config.json");

    if !config_path.exists() {
        return RepairResult {
            action_id: "check_config".into(),
            success: true,
            message: "Config file not found (will be created on first use).".into(),
            verification: None,
        };
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(_) => RepairResult {
                action_id: "check_config".into(),
                success: true,
                message: "Config file is valid JSON.".into(),
                verification: None,
            },
            Err(e) => RepairResult {
                action_id: "check_config".into(),
                success: false,
                message: format!("Config file is corrupt: {e}"),
                verification: None,
            },
        },
        Err(e) => RepairResult {
            action_id: "check_config".into(),
            success: false,
            message: format!("Cannot read config: {e}"),
            verification: None,
        },
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_actions_list_is_non_empty() {
        assert!(!REPAIR_ACTIONS.is_empty());
    }

    #[test]
    fn unknown_action_returns_error() {
        let result = run_repair("nonexistent_action");
        assert!(!result.success);
        assert!(result.message.contains("Unknown"));
    }

    #[test]
    fn repair_result_serializes() {
        let result = RepairResult {
            action_id: "test".into(),
            success: true,
            message: "ok".into(),
            verification: None,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn repair_result_serializes_verification_detail() {
        let result = RepairResult {
            action_id: "restart_gateway".into(),
            success: false,
            message: "Gateway repair verification failed.".into(),
            verification: Some(RepairVerification {
                label: "Gateway service".into(),
                ok: false,
                message: "Service 'com.forge.gateway' status unknown.".into(),
            }),
        };

        let json = serde_json::to_value(&result).expect("serialize");

        assert_eq!(json["verification"]["ok"], false);
        assert_eq!(json["verification"]["label"], "Gateway service");
    }

    #[test]
    fn gateway_service_repair_result_fails_when_status_is_not_running() {
        let result = gateway_service_repair_result(
            "restart_gateway",
            Ok("Service installed.".to_string()),
            Ok("Service 'com.forge.gateway' status unknown.".to_string()),
        );

        assert!(!result.success);
        assert_eq!(result.verification.as_ref().map(|v| v.ok), Some(false));
        assert!(result.message.contains("verification failed"));
    }

    #[test]
    fn gateway_service_repair_result_passes_when_status_is_running() {
        let result = gateway_service_repair_result(
            "restart_gateway",
            Ok("Service installed.".to_string()),
            Ok("Service 'com.forge.gateway' is running.".to_string()),
        );

        assert!(result.success, "{}", result.message);
        assert_eq!(result.verification.as_ref().map(|v| v.ok), Some(true));
    }

    #[test]
    fn gateway_service_repair_result_accepts_platform_running_statuses() {
        for status in [
            "Service 'forge-gateway.service' is running.",
            "Service 'ForgeGateway' is running.",
        ] {
            let result = gateway_service_repair_result(
                "restart_gateway",
                Ok("Service restarted.".to_string()),
                Ok(status.to_string()),
            );

            assert!(result.success, "{status}: {}", result.message);
            assert_eq!(result.verification.as_ref().map(|v| v.ok), Some(true));
        }
    }

    #[test]
    fn all_action_ids_have_known_dispatch_contracts() {
        let dispatchable = [
            "restart_gateway",
            "clear_snapshot_cache",
            "clear_a2a_ledger_cache",
            "reinstall_service",
            "clear_logs",
            "check_config",
        ];
        for action in REPAIR_ACTIONS {
            assert!(
                dispatchable.contains(&action.id),
                "action '{}' must be handled by run_repair",
                action.id
            );
        }
    }

    #[test]
    fn snapshot_cache_dir_points_to_session_snapshots() {
        let root = std::path::PathBuf::from("/tmp/forge-home");

        assert_eq!(
            snapshot_cache_dir_for_home(&root),
            root.join(".forge").join("sessions")
        );
    }

    #[test]
    fn clear_snapshot_cache_removes_session_snapshot_files() {
        let root = std::env::temp_dir().join(format!(
            "forge-repair-snapshots-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        let snapshots_dir = root.join(".forge").join("sessions");
        std::fs::create_dir_all(&snapshots_dir).expect("snapshot dir");
        std::fs::write(snapshots_dir.join("session.json"), "{}").expect("snapshot");

        let result = clear_snapshot_cache_at(&snapshots_dir);

        assert!(result.success, "{}", result.message);
        assert_eq!(result.verification.as_ref().map(|v| v.ok), Some(true));
        assert_eq!(
            result.verification.as_ref().map(|v| v.label.as_str()),
            Some("Snapshot cache")
        );
        assert!(snapshots_dir.exists());
        assert!(
            std::fs::read_dir(&snapshots_dir)
                .expect("read snapshot dir")
                .next()
                .is_none(),
            "snapshot files should be removed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a2a_ledger_cache_dir_points_to_a2a_ledgers() {
        let root = std::path::PathBuf::from("/tmp/forge-home");

        assert_eq!(
            a2a_ledger_cache_dir_for_home(&root),
            root.join(".forge").join("a2a")
        );
    }

    #[test]
    fn clear_a2a_ledger_cache_removes_ledger_files() {
        let root = std::env::temp_dir().join(format!(
            "forge-repair-a2a-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        let ledger_dir = root.join(".forge").join("a2a");
        std::fs::create_dir_all(&ledger_dir).expect("ledger dir");
        std::fs::write(ledger_dir.join("session.json"), "{}").expect("ledger");

        let result = clear_a2a_ledger_cache_at(&ledger_dir);

        assert!(result.success, "{}", result.message);
        assert_eq!(result.verification.as_ref().map(|v| v.ok), Some(true));
        assert_eq!(
            result.verification.as_ref().map(|v| v.label.as_str()),
            Some("A2A ledger cache")
        );
        assert!(ledger_dir.exists());
        assert!(
            std::fs::read_dir(&ledger_dir)
                .expect("read ledger dir")
                .next()
                .is_none(),
            "A2A ledger files should be removed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
