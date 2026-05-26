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
    use crate::harness::Harness;

    #[tokio::test]
    async fn unknown_session_id_does_not_fallback_to_explicit_working_dir() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-stale-session-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
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
}
