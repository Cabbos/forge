//! Self-healing repair actions triggered from diagnostics.
//!
//! Each action is idempotent and safe to run multiple times.

use serde::Serialize;

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
}

/// Run a specific repair action by id.  Returns the result.
pub fn run_repair(action_id: &str) -> RepairResult {
    match action_id {
        "restart_gateway" => restart_gateway(),
        "clear_snapshot_cache" => clear_snapshot_cache(),
        "reinstall_service" => reinstall_service(),
        "clear_logs" => clear_logs(),
        "check_config" => check_config(),
        _ => RepairResult {
            action_id: action_id.to_string(),
            success: false,
            message: format!("Unknown repair action: {action_id}"),
        },
    }
}

fn restart_gateway() -> RepairResult {
    // Stop → start via launchctl.
    let plist_path = crate::service::launchd::plist_path();
    if !plist_path.exists() {
        // Try installing first.
        match crate::service::launchd::install() {
            Ok(msg) => RepairResult {
                action_id: "restart_gateway".into(),
                success: true,
                message: msg,
            },
            Err(e) => RepairResult {
                action_id: "restart_gateway".into(),
                success: false,
                message: format!("install failed: {e}"),
            },
        }
    } else {
        // Bootout then bootstrap.
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", "gui/501", plist_path.to_str().unwrap_or("")])
            .output();
        match crate::service::launchd::install() {
            Ok(msg) => RepairResult {
                action_id: "restart_gateway".into(),
                success: true,
                message: msg,
            },
            Err(e) => RepairResult {
                action_id: "restart_gateway".into(),
                success: false,
                message: format!("restart failed: {e}"),
            },
        }
    }
}

fn clear_snapshot_cache() -> RepairResult {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let snapshots_dir = std::path::PathBuf::from(home)
        .join(".forge")
        .join("snapshots");

    if !snapshots_dir.exists() {
        return RepairResult {
            action_id: "clear_snapshot_cache".into(),
            success: true,
            message: "No snapshot cache to clear.".into(),
        };
    }

    match std::fs::remove_dir_all(&snapshots_dir) {
        Ok(()) => {
            let _ = std::fs::create_dir_all(&snapshots_dir);
            RepairResult {
                action_id: "clear_snapshot_cache".into(),
                success: true,
                message: "Snapshot cache cleared.".into(),
            }
        }
        Err(e) => RepairResult {
            action_id: "clear_snapshot_cache".into(),
            success: false,
            message: format!("Failed: {e}"),
        },
    }
}

fn reinstall_service() -> RepairResult {
    // Uninstall then install.
    let _ = crate::service::launchd::uninstall();
    match crate::service::launchd::install() {
        Ok(msg) => RepairResult {
            action_id: "reinstall_service".into(),
            success: true,
            message: msg,
        },
        Err(e) => RepairResult {
            action_id: "reinstall_service".into(),
            success: false,
            message: format!("reinstall failed: {e}"),
        },
    }
}

fn clear_logs() -> RepairResult {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let log_path = std::path::PathBuf::from(home)
        .join(".forge")
        .join("logs")
        .join("forge.log");

    if !log_path.exists() {
        return RepairResult {
            action_id: "clear_logs".into(),
            success: true,
            message: "No log file to clear.".into(),
        };
    }

    // Rotate: rename current to .old, start fresh.
    let archived = log_path.with_extension("log.old");
    match std::fs::rename(&log_path, &archived) {
        Ok(()) => RepairResult {
            action_id: "clear_logs".into(),
            success: true,
            message: "Log file archived and cleared.".into(),
        },
        Err(e) => RepairResult {
            action_id: "clear_logs".into(),
            success: false,
            message: format!("Failed: {e}"),
        },
    }
}

fn check_config() -> RepairResult {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_path = std::path::PathBuf::from(home)
        .join(".forge")
        .join("config.json");

    if !config_path.exists() {
        return RepairResult {
            action_id: "check_config".into(),
            success: true,
            message: "Config file not found (will be created on first use).".into(),
        };
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(_) => RepairResult {
                action_id: "check_config".into(),
                success: true,
                message: "Config file is valid JSON.".into(),
            },
            Err(e) => RepairResult {
                action_id: "check_config".into(),
                success: false,
                message: format!("Config file is corrupt: {e}"),
            },
        },
        Err(e) => RepairResult {
            action_id: "check_config".into(),
            success: false,
            message: format!("Cannot read config: {e}"),
        },
    }
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
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn all_action_ids_are_valid_function_dispatches() {
        for action in REPAIR_ACTIONS {
            let result = run_repair(action.id);
            // Unknown actions are caught by the `_` arm.
            assert_eq!(result.action_id, action.id);
            // Should not contain "Unknown" for valid actions.
            assert!(
                !result.message.contains("Unknown repair action"),
                "action '{}' should be handled: {}",
                action.id,
                result.message
            );
        }
    }
}
