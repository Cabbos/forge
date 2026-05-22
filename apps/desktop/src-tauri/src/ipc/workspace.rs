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
    }

    if let Some(working_dir) = resolve_optional_workspace_path(working_dir)? {
        return Ok(working_dir);
    }

    Err(MISSING_WORKSPACE_MESSAGE.to_string())
}
