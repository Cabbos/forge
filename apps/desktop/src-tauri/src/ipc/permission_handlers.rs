use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::harness::db::PermissionRuleRow;
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

fn clean_tool_name(tool_name: String) -> Result<String, String> {
    let trimmed = tool_name.trim();
    if trimmed.is_empty() {
        Err("Tool name is required.".to_string())
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
}
