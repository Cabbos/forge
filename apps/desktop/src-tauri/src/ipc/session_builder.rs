use std::path::Path;
use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::build_adapter_with_profiles;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::provider_capabilities::{context_window_tokens, provider_label};
use crate::agent::session::AgentSession;
use crate::harness::Harness;
use crate::settings;

type PendingConfirms =
    Arc<tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>;

pub(crate) struct BuildAgentSessionRequest<'a> {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub api_key: &'a str,
    pub api_base: Option<&'a str>,
    pub working_dir: &'a Path,
    pub pending_confirms: PendingConfirms,
    pub existing_context_window_tokens: Option<u32>,
}

/// Build a fresh AgentSession from resolved configuration.
/// Returns the session and whether the API key is missing.
pub(crate) async fn build_agent_session(
    request: BuildAgentSessionRequest<'_>,
) -> Result<(AgentSession, bool), String> {
    build_agent_session_with_registry_path(request, None).await
}

pub(crate) async fn build_agent_session_with_registry_path(
    request: BuildAgentSessionRequest<'_>,
    registry_path: Option<std::path::PathBuf>,
) -> Result<(AgentSession, bool), String> {
    let BuildAgentSessionRequest {
        session_id,
        provider,
        model,
        api_key,
        api_base,
        working_dir,
        pending_confirms,
        existing_context_window_tokens,
    } = request;

    let harness = Arc::new(match registry_path {
        Some(registry_path) => Harness::new_with_pending_and_registry_path(
            working_dir.to_path_buf(),
            pending_confirms,
            registry_path,
        ),
        None => Harness::new_with_pending(working_dir.to_path_buf(), pending_confirms),
    });
    let context_window_tokens =
        existing_context_window_tokens.or_else(|| context_window_tokens(&provider, &model));
    let provider_profiles = settings::load_configured_provider_profiles();
    let missing_api_key =
        api_key.trim().is_empty() && settings::provider_requires_api_key(&provider);
    let external_tools = if missing_api_key {
        Vec::new()
    } else {
        harness.external_mcp_tool_definitions().await
    };
    let adapter: Arc<dyn AiAdapter> = if missing_api_key {
        Arc::new(MissingKeyAdapter::new(provider_label(&provider), &model))
    } else {
        build_adapter_with_profiles(
            &provider,
            api_key,
            &model,
            api_base,
            &provider_profiles,
            external_tools,
        )
        .map_err(|error| error.to_string())?
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
