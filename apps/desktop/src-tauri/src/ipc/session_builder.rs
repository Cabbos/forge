use std::path::Path;
use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::build_adapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::provider_capabilities::{context_window_tokens, provider_label};
use crate::agent::session::AgentSession;
use crate::harness::Harness;

/// Build a fresh AgentSession from resolved configuration.
/// Returns the session and whether the API key is missing.
pub(crate) async fn build_agent_session(
    session_id: String,
    provider: String,
    model: String,
    api_key: &str,
    api_base: Option<&str>,
    working_dir: &Path,
    pending_confirms: Arc<tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    existing_context_window_tokens: Option<u32>,
) -> Result<(AgentSession, bool), String> {
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.to_path_buf(),
        pending_confirms,
    ));
    let context_window_tokens = existing_context_window_tokens
        .or_else(|| context_window_tokens(&provider, &model));
    let missing_api_key = api_key.trim().is_empty();
    let external_tools = if missing_api_key {
        Vec::new()
    } else {
        harness.external_mcp_tool_definitions().await
    };
    let adapter: Arc<dyn AiAdapter> = if missing_api_key {
        Arc::new(MissingKeyAdapter::new(provider_label(&provider), &model))
    } else {
        build_adapter(&provider, api_key, &model, api_base, external_tools)?
    };
    let system_prompt = harness.build_system_prompt(&provider, working_dir).await;
    let session = AgentSession::new(
        session_id,
        provider,
        adapter,
        harness,
        system_prompt,
        context_window_tokens,
    );
    Ok((session, missing_api_key))
}
