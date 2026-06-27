use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agent::session::AgentSession;
use crate::harness::db::PermissionRuleRow;
use crate::harness::permissions::{PermissionMode, PermissionModeState};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRuleDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PermissionRuleView {
    pub tool_name: String,
    pub decision: PermissionRuleDecision,
    pub created_at: String,
}

#[tauri::command]
pub async fn list_permission_rules(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<PermissionRuleView>, String> {
    list_permission_rules_for_state(state.inner().as_ref()).await
}

#[tauri::command]
pub async fn set_permission_rule(
    state: tauri::State<'_, Arc<AppState>>,
    tool_name: String,
    decision: PermissionRuleDecision,
) -> Result<Vec<PermissionRuleView>, String> {
    set_permission_rule_for_state(state.inner().as_ref(), tool_name, decision).await
}

#[tauri::command]
pub async fn reset_permission_rule(
    state: tauri::State<'_, Arc<AppState>>,
    tool_name: String,
) -> Result<Vec<PermissionRuleView>, String> {
    reset_permission_rule_for_state(state.inner().as_ref(), tool_name).await
}

#[tauri::command]
pub async fn get_permission_mode(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    workspace_path: Option<String>,
) -> Result<PermissionModeState, String> {
    get_permission_mode_for_state(
        state.inner().as_ref(),
        &session_id,
        workspace_path.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn set_permission_mode(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    mode: PermissionMode,
    workspace_path: Option<String>,
) -> Result<PermissionModeState, String> {
    set_permission_mode_for_state(state.inner().as_ref(), session_id, mode, workspace_path).await
}

pub(crate) async fn list_permission_rules_for_state(
    state: &AppState,
) -> Result<Vec<PermissionRuleView>, String> {
    state
        .harness
        .database
        .list_permission_rules()
        .map(|rules| rules.into_iter().map(PermissionRuleView::from).collect())
        .map_err(|error| error.to_string())
}

pub(crate) async fn set_permission_rule_for_state(
    state: &AppState,
    tool_name: String,
    decision: PermissionRuleDecision,
) -> Result<Vec<PermissionRuleView>, String> {
    let tool_name = clean_tool_name(tool_name)?;
    match decision {
        PermissionRuleDecision::Allow => {
            state
                .harness
                .permission_gate
                .approve_permanently(&tool_name)
                .await;
        }
        PermissionRuleDecision::Deny => {
            state
                .harness
                .permission_gate
                .deny_permanently(&tool_name)
                .await;
        }
    }
    list_permission_rules_for_state(state).await
}

pub(crate) async fn reset_permission_rule_for_state(
    state: &AppState,
    tool_name: String,
) -> Result<Vec<PermissionRuleView>, String> {
    let tool_name = clean_tool_name(tool_name)?;
    state
        .harness
        .permission_gate
        .reset_permission(&tool_name)
        .await;
    list_permission_rules_for_state(state).await
}

pub(crate) async fn get_permission_mode_for_state(
    state: &AppState,
    session_id: &str,
    workspace_path: Option<&str>,
) -> Result<PermissionModeState, String> {
    let mode = state
        .harness
        .permission_gate
        .permission_mode_state(session_id, workspace_path.map(std::path::Path::new))
        .await;
    sync_permission_mode_to_live_session(state, session_id, mode.mode, workspace_path).await;
    Ok(mode)
}

pub(crate) async fn set_permission_mode_for_state(
    state: &AppState,
    session_id: String,
    mode: PermissionMode,
    workspace_path: Option<String>,
) -> Result<PermissionModeState, String> {
    let session_id = clean_session_id(session_id)?;
    let workspace_for_state = workspace_path.as_deref();
    match mode {
        PermissionMode::ManualConfirm => {
            state
                .harness
                .permission_gate
                .restore_manual_confirm(&session_id, workspace_for_state.map(std::path::Path::new))
                .await;
            sync_permission_mode_to_live_session(
                state,
                &session_id,
                PermissionMode::ManualConfirm,
                workspace_for_state,
            )
            .await;
        }
        PermissionMode::TrustCurrentProject => {
            let workspace_path = workspace_path.as_deref().ok_or_else(|| {
                "Workspace path is required for trust_current_project.".to_string()
            })?;
            state
                .harness
                .permission_gate
                .trust_current_project(&session_id, std::path::Path::new(workspace_path))
                .await;
            sync_permission_mode_to_live_session(
                state,
                &session_id,
                PermissionMode::TrustCurrentProject,
                Some(workspace_path),
            )
            .await;
        }
        PermissionMode::FullAccess => {
            let workspace_path = workspace_path
                .as_deref()
                .ok_or_else(|| "Workspace path is required for full_access.".to_string())?;
            state
                .harness
                .permission_gate
                .full_access_current_project(&session_id, std::path::Path::new(workspace_path))
                .await;
            sync_permission_mode_to_live_session(
                state,
                &session_id,
                PermissionMode::FullAccess,
                Some(workspace_path),
            )
            .await;
        }
    }
    get_permission_mode_for_state(state, &session_id, workspace_for_state).await
}

pub(crate) async fn sync_app_permission_mode_to_session(
    state: &AppState,
    session: &AgentSession,
    session_id: &str,
    workspace_path: &std::path::Path,
) -> PermissionModeState {
    let mode = state
        .harness
        .permission_gate
        .permission_mode_state(session_id, Some(workspace_path))
        .await;
    sync_permission_mode_to_session(session, session_id, mode.mode, Some(workspace_path)).await;
    mode
}

async fn sync_permission_mode_to_live_session(
    state: &AppState,
    session_id: &str,
    mode: PermissionMode,
    workspace_path: Option<&str>,
) {
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(session_id).cloned()
    };
    let Some(session) = session else {
        return;
    };
    sync_permission_mode_to_session(
        &session,
        session_id,
        mode,
        workspace_path.map(std::path::Path::new),
    )
    .await;
}

async fn sync_permission_mode_to_session(
    session: &AgentSession,
    session_id: &str,
    mode: PermissionMode,
    workspace_path: Option<&std::path::Path>,
) {
    match mode {
        PermissionMode::ManualConfirm => {
            session
                .harness
                .permission_gate
                .restore_manual_confirm(session_id, workspace_path)
                .await;
        }
        PermissionMode::TrustCurrentProject => {
            let Some(workspace_path) = workspace_path else {
                return;
            };
            session
                .harness
                .permission_gate
                .trust_current_project(session_id, workspace_path)
                .await;
        }
        PermissionMode::FullAccess => {
            let Some(workspace_path) = workspace_path else {
                return;
            };
            session
                .harness
                .permission_gate
                .full_access_current_project(session_id, workspace_path)
                .await;
        }
    }
}

fn clean_tool_name(tool_name: String) -> Result<String, String> {
    let trimmed = tool_name.trim();
    if trimmed.is_empty() {
        Err("Tool name is required.".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn clean_session_id(session_id: String) -> Result<String, String> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        Err("Session id is required.".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

impl From<PermissionRuleRow> for PermissionRuleView {
    fn from(row: PermissionRuleRow) -> Self {
        Self {
            tool_name: row.tool_name,
            decision: if row.approved {
                PermissionRuleDecision::Allow
            } else {
                PermissionRuleDecision::Deny
            },
            created_at: row.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::session::AgentSession;
    use crate::harness::permissions::PermissionDecision as GateDecision;
    use crate::harness::Harness;
    use crate::state::AppState;
    use std::sync::Arc;

    fn temp_state() -> (AppState, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("forge-permission-ipc-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(dir.join("src")).expect("create workspace");
        std::fs::write(dir.join("src/main.rs"), "fn main() {}").expect("seed file");
        let state = AppState::new(Arc::new(Harness::new(dir.clone())));
        (state, dir)
    }

    #[tokio::test]
    async fn set_and_reset_permission_rule_for_state() {
        let (state, dir) = temp_state();

        let rules = set_permission_rule_for_state(
            &state,
            "write".to_string(),
            PermissionRuleDecision::Allow,
        )
        .await
        .expect("allow rule");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].tool_name, "write_to_file");
        assert_eq!(rules[0].decision, PermissionRuleDecision::Allow);

        let rules = set_permission_rule_for_state(
            &state,
            "write".to_string(),
            PermissionRuleDecision::Deny,
        )
        .await
        .expect("deny rule");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].decision, PermissionRuleDecision::Deny);

        let decision = state
            .harness
            .permission_gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(decision, GateDecision::Deny { .. }),
            "deny rule should feed the runtime permission gate: {:?}",
            decision
        );

        let rules = reset_permission_rule_for_state(&state, "write".to_string())
            .await
            .expect("reset rule");
        assert!(rules.is_empty());

        let decision = state
            .harness
            .permission_gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(decision, GateDecision::Ask { .. }),
            "reset should restore write_to_file to confirmation flow: {:?}",
            decision
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn permission_mode_for_state_trusts_and_restores_current_project() {
        let (state, dir) = temp_state();

        let initial = get_permission_mode_for_state(
            &state,
            "session-1",
            Some(dir.to_string_lossy().as_ref()),
        )
        .await
        .expect("initial mode");
        assert_eq!(initial.mode, PermissionMode::ManualConfirm);
        assert_eq!(initial.workspace_path, None);
        assert!(initial.session_scoped);

        let trusted = set_permission_mode_for_state(
            &state,
            "session-1".to_string(),
            PermissionMode::TrustCurrentProject,
            Some(dir.to_string_lossy().to_string()),
        )
        .await
        .expect("trust mode");
        let canonical_dir = dir.canonicalize().expect("canonical temp dir");
        assert_eq!(trusted.mode, PermissionMode::TrustCurrentProject);
        assert_eq!(
            trusted.workspace_path.as_deref(),
            Some(canonical_dir.to_string_lossy().as_ref())
        );
        assert!(!trusted.session_scoped);

        let decision = state
            .harness
            .permission_gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(decision, GateDecision::Allow),
            "trust mode should feed the runtime permission gate: {:?}",
            decision
        );

        let inherited = get_permission_mode_for_state(
            &state,
            "session-2",
            Some(dir.to_string_lossy().as_ref()),
        )
        .await
        .expect("inherited mode");
        assert_eq!(inherited.mode, PermissionMode::TrustCurrentProject);

        let decision = state
            .harness
            .permission_gate
            .check(
                "session-2",
                "write_to_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(decision, GateDecision::Allow),
            "trust mode should carry across sessions in the same workspace: {:?}",
            decision
        );

        let restored = set_permission_mode_for_state(
            &state,
            "session-2".to_string(),
            PermissionMode::ManualConfirm,
            Some(dir.to_string_lossy().to_string()),
        )
        .await
        .expect("manual mode");
        assert_eq!(restored.mode, PermissionMode::ManualConfirm);
        assert_eq!(restored.workspace_path, None);

        let decision = state
            .harness
            .permission_gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(decision, GateDecision::Ask { .. }),
            "manual mode should restore confirmation flow: {:?}",
            decision
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn permission_mode_for_state_enables_full_access_and_restores_manual() {
        let (state, dir) = temp_state();

        let full_access = set_permission_mode_for_state(
            &state,
            "session-1".to_string(),
            PermissionMode::FullAccess,
            Some(dir.to_string_lossy().to_string()),
        )
        .await
        .expect("full access mode");
        let canonical_dir = dir.canonicalize().expect("canonical temp dir");
        assert_eq!(full_access.mode, PermissionMode::FullAccess);
        assert_eq!(
            full_access.workspace_path.as_deref(),
            Some(canonical_dir.to_string_lossy().as_ref())
        );
        assert!(!full_access.session_scoped);

        let shell_decision = state
            .harness
            .permission_gate
            .check(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "npm install left-pad"}),
                &dir,
            )
            .await;
        assert!(
            matches!(shell_decision, GateDecision::Allow),
            "full access should feed the runtime permission gate: {:?}",
            shell_decision
        );

        let restored = set_permission_mode_for_state(
            &state,
            "session-1".to_string(),
            PermissionMode::ManualConfirm,
            Some(dir.to_string_lossy().to_string()),
        )
        .await
        .expect("manual mode");
        assert_eq!(restored.mode, PermissionMode::ManualConfirm);
        assert_eq!(restored.workspace_path, None);

        let shell_decision = state
            .harness
            .permission_gate
            .check(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "npm install left-pad"}),
                &dir,
            )
            .await;
        assert!(
            matches!(shell_decision, GateDecision::Ask { .. }),
            "manual mode should restore shell confirmation flow: {:?}",
            shell_decision
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn inherited_project_trust_syncs_to_live_session_harness() {
        let (state, dir) = temp_state();
        state
            .harness
            .permission_gate
            .trust_current_project("previous-session", &dir)
            .await;

        let session = Arc::new(AgentSession::new(
            "session-2".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            Arc::new(Harness::new_with_pending(
                dir.clone(),
                state.pending_confirms.clone(),
            )),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-2".to_string(), session.clone())
            .await;

        let before = session
            .harness
            .permission_gate
            .check(
                "session-2",
                "edit_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(before, GateDecision::Ask { .. }),
            "new session harness starts without app-level runtime trust: {:?}",
            before
        );

        let inherited = get_permission_mode_for_state(
            &state,
            "session-2",
            Some(dir.to_string_lossy().as_ref()),
        )
        .await
        .expect("inherited mode");
        assert_eq!(inherited.mode, PermissionMode::TrustCurrentProject);

        let after = session
            .harness
            .permission_gate
            .check(
                "session-2",
                "edit_file",
                &serde_json::json!({"path": "src/main.rs"}),
                &dir,
            )
            .await;
        assert!(
            matches!(after, GateDecision::Allow),
            "permission mode lookup should sync trust into live session harness: {:?}",
            after
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
