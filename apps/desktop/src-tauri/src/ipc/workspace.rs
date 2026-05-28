use std::path::PathBuf;
use std::sync::Arc;

use crate::state::AppState;
use crate::workspace_safety::resolve_optional_workspace_path;

const MISSING_WORKSPACE_MESSAGE: &str = "当前请求没有绑定工作空间，请先选择项目或从已有会话发起。";

pub(crate) async fn resolve_bound_working_dir(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(session_id) = session_id {
        if let Some(session) = state.sessions.read().await.get(session_id).cloned() {
            return Ok(session.harness.working_dir.clone());
        }
        return Err("当前会话不可用，请重新打开对话或重新选择项目。".to_string());
    }

    if let Some(working_dir) = resolve_optional_workspace_path(working_dir)? {
        return Ok(working_dir);
    }

    Err(MISSING_WORKSPACE_MESSAGE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::session::AgentSession;
    use crate::harness::Harness;

    fn temp_workspace(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-workspace-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&path).expect("workspace");
        path
    }

    fn canonical_or_self(path: &std::path::Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    #[tokio::test]
    async fn unknown_session_id_does_not_fallback_to_explicit_working_dir() {
        let workspace = temp_workspace("stale-session");
        let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

        let error = resolve_bound_working_dir(
            &state,
            Some("missing-session"),
            Some(workspace.to_str().expect("utf8")),
        )
        .await
        .expect_err("stale session should not fall back to explicit workspace");

        assert!(error.contains("会话"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn session_workspace_wins_over_explicit_working_dir() {
        let session_workspace = temp_workspace("session-bound");
        let explicit_workspace = temp_workspace("explicit-ignored");
        let state = Arc::new(AppState::new(Arc::new(Harness::new(
            explicit_workspace.clone(),
        ))));
        let session = Arc::new(AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            Arc::new(Harness::new(session_workspace.clone())),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;

        let resolved = resolve_bound_working_dir(
            &state,
            Some("session-1"),
            Some(explicit_workspace.to_str().expect("utf8")),
        )
        .await
        .expect("session workspace should resolve");

        assert_eq!(
            canonical_or_self(&resolved),
            canonical_or_self(&session_workspace)
        );
        assert_ne!(
            canonical_or_self(&resolved),
            canonical_or_self(&explicit_workspace)
        );

        let _ = std::fs::remove_dir_all(session_workspace);
        let _ = std::fs::remove_dir_all(explicit_workspace);
    }
}
