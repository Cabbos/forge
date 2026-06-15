use std::sync::Arc;

use crate::diagnostics::{self, CapabilitySummary};
use crate::gateway::client::{
    build_attach_session_request, build_get_session_snapshot_request, GatewayClient,
};
use crate::gateway::protocol::{
    AttachSessionResult, CancelTriggerParams, CancelTriggerResult, EnqueueTriggerParams,
    EnqueueTriggerResult, GatewayReply, GatewayRequest, GatewaySessionInfo,
    GetSessionSnapshotResult, GetTriggerRunParams, GetTriggerRunResult, ReplayTriggerRunParams,
    ReplayTriggerRunResult,
};
use crate::gateway::server::{default_socket_path, GatewayRuntimeStatus};
use crate::gateway::webhook::PendingTrigger;
use crate::harness::skills::SkillLoader;
use crate::ipc::capability_handlers::{
    capability_kind_label, ecosystem_status_for_capability, ecosystem_status_label,
    is_hidden_global_capability,
};
use crate::state::AppState;

#[tauri::command]
pub async fn get_diagnostics_report(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<diagnostics::DiagnosticsReport, String> {
    // Collect capabilities from the harness registry + skill loader
    let capabilities = Some(collect_capabilities(&state).await);

    Ok(diagnostics::run_diagnostics(capabilities))
}

#[tauri::command]
pub async fn get_gateway_runtime_status() -> Result<GatewayRuntimeStatus, String> {
    Ok(read_gateway_runtime_status().await)
}

#[tauri::command]
pub async fn enqueue_gateway_trigger(
    input: EnqueueTriggerParams,
) -> Result<EnqueueTriggerResult, String> {
    let request = build_enqueue_gateway_trigger_request(input)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<EnqueueTriggerResult>(response.result)
                .map_err(|error| format!("Gateway returned invalid enqueue result: {error}"))
        }
        Ok(GatewayReply::Err(error)) => {
            Err(format!("Gateway enqueue error: {}", error.error.message))
        }
        Err(error) => Err(format!("Gateway enqueue request failed: {error}")),
    }
}

#[tauri::command]
pub async fn list_gateway_triggers() -> Result<Vec<PendingTrigger>, String> {
    let request = build_list_gateway_triggers_request();
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<Vec<PendingTrigger>>(response.result)
                .map_err(|error| format!("Gateway returned invalid trigger list: {error}"))
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway trigger list error: {}",
            error.error.message
        )),
        Err(error) => Err(format!("Gateway trigger list request failed: {error}")),
    }
}

#[tauri::command]
pub async fn list_gateway_sessions() -> Result<Vec<GatewaySessionInfo>, String> {
    let request = build_list_gateway_sessions_request();
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<Vec<GatewaySessionInfo>>(response.result)
                .map_err(|error| format!("Gateway returned invalid session list: {error}"))
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway session list error: {}",
            error.error.message
        )),
        Err(error) => Err(format!("Gateway session list request failed: {error}")),
    }
}

#[tauri::command]
pub async fn cancel_gateway_trigger(trigger_id: String) -> Result<CancelTriggerResult, String> {
    let request = build_cancel_gateway_trigger_request(trigger_id)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<CancelTriggerResult>(response.result)
                .map_err(|error| format!("Gateway returned invalid cancel result: {error}"))
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway trigger cancel error: {}",
            error.error.message
        )),
        Err(error) => Err(format!("Gateway trigger cancel request failed: {error}")),
    }
}

#[tauri::command]
pub async fn replay_gateway_trigger_run(run_id: String) -> Result<ReplayTriggerRunResult, String> {
    let request = build_replay_gateway_trigger_run_request(run_id)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<ReplayTriggerRunResult>(response.result)
                .map_err(|error| format!("Gateway returned invalid replay result: {error}"))
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway trigger replay error: {}",
            error.error.message
        )),
        Err(error) => Err(format!("Gateway trigger replay request failed: {error}")),
    }
}

#[tauri::command]
pub async fn get_gateway_trigger_run(run_id: String) -> Result<GetTriggerRunResult, String> {
    let request = build_get_gateway_trigger_run_request(run_id)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<GetTriggerRunResult>(response.result)
                .map_err(|error| format!("Gateway returned invalid trigger run detail: {error}"))
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway trigger run detail error: {}",
            error.error.message
        )),
        Err(error) => Err(format!(
            "Gateway trigger run detail request failed: {error}"
        )),
    }
}

#[tauri::command]
pub async fn attach_gateway_session(session_id: String) -> Result<AttachSessionResult, String> {
    let request = build_attach_gateway_session_request(session_id)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<AttachSessionResult>(response.result)
                .map_err(|error| format!("Gateway returned invalid session attach result: {error}"))
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway session attach error: {}",
            error.error.message
        )),
        Err(error) => Err(format!("Gateway session attach request failed: {error}")),
    }
}

#[tauri::command]
pub async fn get_gateway_session_snapshot(
    session_id: String,
) -> Result<GetSessionSnapshotResult, String> {
    let request = build_get_gateway_session_snapshot_request(session_id)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => {
            serde_json::from_value::<GetSessionSnapshotResult>(response.result).map_err(|error| {
                format!("Gateway returned invalid session snapshot detail: {error}")
            })
        }
        Ok(GatewayReply::Err(error)) => Err(format!(
            "Gateway session snapshot detail error: {}",
            error.error.message
        )),
        Err(error) => Err(format!(
            "Gateway session snapshot detail request failed: {error}"
        )),
    }
}

async fn read_gateway_runtime_status() -> GatewayRuntimeStatus {
    let socket_path = default_socket_path();
    let mut client = match GatewayClient::connect(&socket_path).await {
        Ok(client) => client,
        Err(error) => {
            return unavailable_gateway_runtime_status(format!("Gateway unavailable: {error}"));
        }
    };

    let request = GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "runtime_status".to_string(),
        params: None,
    };

    match client.send(request).await {
        Ok(GatewayReply::Ok(response)) => serde_json::from_value::<GatewayRuntimeStatus>(
            response.result,
        )
        .unwrap_or_else(|error| {
            unavailable_gateway_runtime_status(format!("Gateway returned invalid status: {error}"))
        }),
        Ok(GatewayReply::Err(error)) => unavailable_gateway_runtime_status(format!(
            "Gateway status error: {}",
            error.error.message
        )),
        Err(error) => {
            unavailable_gateway_runtime_status(format!("Gateway status request failed: {error}"))
        }
    }
}

fn build_list_gateway_triggers_request() -> GatewayRequest {
    GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "list_pending_triggers".to_string(),
        params: None,
    }
}

fn build_list_gateway_sessions_request() -> GatewayRequest {
    GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "list_sessions".to_string(),
        params: None,
    }
}

fn build_cancel_gateway_trigger_request(trigger_id: String) -> Result<GatewayRequest, String> {
    let trigger_id = trigger_id.trim().to_string();
    if trigger_id.is_empty() {
        return Err("trigger_id must not be empty".to_string());
    }

    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "cancel_trigger".to_string(),
        params: Some(
            serde_json::to_value(CancelTriggerParams { trigger_id })
                .map_err(|error| format!("serialize cancel params: {error}"))?,
        ),
    })
}

fn build_replay_gateway_trigger_run_request(run_id: String) -> Result<GatewayRequest, String> {
    let run_id = run_id.trim().to_string();
    if run_id.is_empty() {
        return Err("run_id must not be empty".to_string());
    }

    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "replay_trigger_run".to_string(),
        params: Some(
            serde_json::to_value(ReplayTriggerRunParams { run_id })
                .map_err(|error| format!("serialize replay params: {error}"))?,
        ),
    })
}

fn build_get_gateway_trigger_run_request(run_id: String) -> Result<GatewayRequest, String> {
    let run_id = run_id.trim().to_string();
    if run_id.is_empty() {
        return Err("run_id must not be empty".to_string());
    }

    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "get_trigger_run".to_string(),
        params: Some(
            serde_json::to_value(GetTriggerRunParams { run_id })
                .map_err(|error| format!("serialize get trigger run params: {error}"))?,
        ),
    })
}

fn build_attach_gateway_session_request(session_id: String) -> Result<GatewayRequest, String> {
    build_attach_session_request(&session_id)
}

fn build_get_gateway_session_snapshot_request(
    session_id: String,
) -> Result<GatewayRequest, String> {
    build_get_session_snapshot_request(&session_id)
}

fn build_enqueue_gateway_trigger_request(
    input: EnqueueTriggerParams,
) -> Result<GatewayRequest, String> {
    let message = input.message.trim().to_string();
    if message.is_empty() {
        return Err("message must not be empty".to_string());
    }

    let params = EnqueueTriggerParams {
        message,
        trigger_id: clean_optional_string(input.trigger_id),
        profile_id: clean_optional_string(input.profile_id),
        provider: clean_optional_string(input.provider),
        model: clean_optional_string(input.model),
        workspace_path: clean_optional_string(input.workspace_path),
    };

    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "enqueue_trigger".to_string(),
        params: Some(serde_json::to_value(params).unwrap()),
    })
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn unavailable_gateway_runtime_status(message: impl Into<String>) -> GatewayRuntimeStatus {
    GatewayRuntimeStatus {
        ok: false,
        message: message.into(),
        uptime_seconds: 0,
        active_sessions: 0,
        pending_triggers: 0,
        claimed_triggers: 0,
        dead_letter_runs: 0,
        recent_runs: Vec::new(),
        runtime_tasks: crate::gateway::server::default_runtime_task_statuses(),
    }
}

async fn collect_capabilities(state: &AppState) -> Vec<CapabilitySummary> {
    let mut summaries: Vec<CapabilitySummary> = state
        .harness
        .capability_registry
        .all_entries()
        .into_iter()
        .filter(|entry| !is_hidden_global_capability(entry))
        .map(|entry| {
            let kind = capability_kind_label(&entry.metadata.kind);
            let (status, status_message) =
                ecosystem_status_for_capability(&entry.metadata, entry.enabled);
            CapabilitySummary {
                id: entry.metadata.id,
                name: entry.metadata.name,
                kind: kind.to_string(),
                enabled: entry.enabled,
                status: Some(ecosystem_status_label(status).to_string()),
                status_message,
            }
        })
        .collect();

    // Merge skills from SkillLoader
    let skill_loader = SkillLoader::new();
    skill_loader.attach_database(state.harness.database.clone());
    let skills = skill_loader.scan_all().await;
    for s in skills {
        if !summaries.iter().any(|c| c.id == s.id) {
            summaries.push(CapabilitySummary {
                id: s.id.clone(),
                name: s.name.clone(),
                kind: "skill".to_string(),
                enabled: s.enabled,
                status: Some("unknown".to_string()),
                status_message: None,
            });
        }
    }

    summaries
}

/// Read recent structured log entries from the global log store.
#[tauri::command]
pub async fn get_recent_logs(
    limit: Option<usize>,
    level: Option<String>,
) -> Result<Vec<crate::log_store::LogEntry>, String> {
    crate::log_store::read_recent_logs(limit.unwrap_or(50), level.as_deref())
}

/// Run a self-healing repair action by id.
#[tauri::command]
pub async fn run_repair_action(
    action_id: String,
) -> Result<crate::diagnostics::repair::RepairResult, String> {
    Ok(crate::diagnostics::repair::run_repair(&action_id))
}

/// List available repair actions.
#[tauri::command]
pub async fn list_repair_actions() -> Result<Vec<crate::diagnostics::repair::RepairAction>, String>
{
    Ok(crate::diagnostics::repair::REPAIR_ACTIONS.to_vec())
}

#[cfg(test)]
mod tests {
    use crate::gateway::protocol::EnqueueTriggerParams;

    #[test]
    fn unavailable_gateway_runtime_status_is_renderable() {
        let status = super::unavailable_gateway_runtime_status("gateway offline");

        assert!(!status.ok);
        assert_eq!(status.pending_triggers, 0);
        assert_eq!(status.claimed_triggers, 0);
        assert_eq!(status.dead_letter_runs, 0);
        assert!(status.recent_runs.is_empty());
        assert!(status.message.contains("gateway offline"));
    }

    #[test]
    fn build_enqueue_gateway_trigger_request_serializes_metadata() {
        let request = super::build_enqueue_gateway_trigger_request(EnqueueTriggerParams {
            message: "run dashboard digest".to_string(),
            trigger_id: None,
            profile_id: Some("ops".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-5".to_string()),
            workspace_path: Some("/repo/workspace".to_string()),
        })
        .expect("request");

        assert_eq!(request.method, "enqueue_trigger");
        let params =
            serde_json::from_value::<EnqueueTriggerParams>(request.params.expect("params"))
                .expect("params");
        assert_eq!(params.message, "run dashboard digest");
        assert_eq!(params.profile_id.as_deref(), Some("ops"));
        assert_eq!(params.provider.as_deref(), Some("openai"));
        assert_eq!(params.model.as_deref(), Some("gpt-5"));
        assert_eq!(params.workspace_path.as_deref(), Some("/repo/workspace"));
    }

    #[test]
    fn build_enqueue_gateway_trigger_request_rejects_blank_message() {
        let error = super::build_enqueue_gateway_trigger_request(EnqueueTriggerParams {
            message: "  ".to_string(),
            trigger_id: None,
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: None,
        })
        .expect_err("blank message");

        assert!(error.contains("message must not be empty"));
    }

    #[test]
    fn build_list_gateway_triggers_request_uses_gateway_method() {
        let request = super::build_list_gateway_triggers_request();

        assert_eq!(request.method, "list_pending_triggers");
        assert!(request.params.is_none());
    }

    #[test]
    fn build_list_gateway_sessions_request_uses_gateway_method() {
        let request = super::build_list_gateway_sessions_request();

        assert_eq!(request.method, "list_sessions");
        assert!(request.params.is_none());
    }

    #[test]
    fn build_cancel_gateway_trigger_request_trims_trigger_id() {
        let request = super::build_cancel_gateway_trigger_request(" trigger-1 ".to_string())
            .expect("request");

        assert_eq!(request.method, "cancel_trigger");
        let params = request.params.expect("params");
        assert_eq!(params["trigger_id"], "trigger-1");
    }

    #[test]
    fn build_cancel_gateway_trigger_request_rejects_blank_id() {
        let error = super::build_cancel_gateway_trigger_request("   ".to_string())
            .expect_err("blank trigger id");

        assert!(error.contains("trigger_id must not be empty"));
    }

    #[test]
    fn build_replay_gateway_trigger_run_request_trims_run_id() {
        let request = super::build_replay_gateway_trigger_run_request(" run-1 ".to_string())
            .expect("request");

        assert_eq!(request.method, "replay_trigger_run");
        let params = request.params.expect("params");
        assert_eq!(params["run_id"], "run-1");
    }

    #[test]
    fn build_replay_gateway_trigger_run_request_rejects_blank_id() {
        let error = super::build_replay_gateway_trigger_run_request("   ".to_string())
            .expect_err("blank run id");

        assert!(error.contains("run_id must not be empty"));
    }

    #[test]
    fn build_get_gateway_trigger_run_request_trims_run_id() {
        let request =
            super::build_get_gateway_trigger_run_request(" run-1 ".to_string()).expect("request");

        assert_eq!(request.method, "get_trigger_run");
        let params = request.params.expect("params");
        assert_eq!(params["run_id"], "run-1");
    }

    #[test]
    fn build_get_gateway_trigger_run_request_rejects_blank_id() {
        let error = super::build_get_gateway_trigger_run_request("   ".to_string())
            .expect_err("blank run id");

        assert!(error.contains("run_id must not be empty"));
    }

    #[test]
    fn build_attach_gateway_session_request_trims_session_id() {
        let request = super::build_attach_gateway_session_request(" session-1 ".to_string())
            .expect("request");

        assert_eq!(request.method, "attach_session");
        let params = request.params.expect("params");
        assert_eq!(params["session_id"], "session-1");
    }

    #[test]
    fn build_attach_gateway_session_request_rejects_blank_id() {
        let error = super::build_attach_gateway_session_request("   ".to_string())
            .expect_err("blank session id");

        assert!(error.contains("session_id must not be empty"));
    }

    #[test]
    fn build_get_gateway_session_snapshot_request_trims_session_id() {
        let request = super::build_get_gateway_session_snapshot_request(" session-1 ".to_string())
            .expect("request");

        assert_eq!(request.method, "get_session_snapshot");
        let params = request.params.expect("params");
        assert_eq!(params["session_id"], "session-1");
    }

    #[test]
    fn build_get_gateway_session_snapshot_request_rejects_blank_id() {
        let error = super::build_get_gateway_session_snapshot_request("   ".to_string())
            .expect_err("blank session id");

        assert!(error.contains("session_id must not be empty"));
    }
}
