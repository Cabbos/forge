use std::sync::Arc;

use crate::diagnostics::{self, CapabilitySummary};
use crate::gateway::client::GatewayClient;
use crate::gateway::protocol::{GatewayReply, GatewayRequest};
use crate::gateway::server::{default_socket_path, GatewayRuntimeStatus};
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
}
