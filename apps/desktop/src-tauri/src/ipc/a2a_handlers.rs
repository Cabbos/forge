use std::collections::BTreeMap;
use std::sync::Arc;

use crate::agent::a2a::bus::{AgentA2ABus, AgentReviewDecision};
use crate::agent::a2a::ledger::{self, AgentA2ALedgerLoadError, AgentA2AProjectionList};
use crate::agent::a2a::projection::AgentA2AProjection;
use crate::agent::a2a::types::AgentTaskId;
use crate::agent::event_sink::TauriEventEmitter;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::snapshot::{load_session_snapshot, save_session_snapshot};
use crate::agent::time::now_ms;
use crate::ipc::session_lifecycle::save_session_snapshot_with_workflow;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentA2AStateSource {
    Live,
    Ledger,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ASessionState {
    pub session_id: String,
    pub source: AgentA2AStateSource,
    pub state: AgentA2AProjection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2AStatesPayload {
    pub states: Vec<AgentA2ASessionState>,
    pub load_errors: Vec<AgentA2ALedgerLoadError>,
}

#[tauri::command]
pub async fn get_agent_a2a_state(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<Option<AgentA2ASessionState>, String> {
    let live_session = state.sessions.read().await.get(&session_id).cloned();
    if let Some(session) = live_session {
        return Ok(Some(live_session_state(&session_id, &session)));
    }

    Ok(
        ledger::load_session_projection(&session_id)?.map(|projection| AgentA2ASessionState {
            session_id,
            source: AgentA2AStateSource::Ledger,
            state: projection,
        }),
    )
}

#[tauri::command]
pub async fn list_agent_a2a_states(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<AgentA2AStatesPayload, String> {
    let live_states = state
        .sessions
        .read()
        .await
        .iter()
        .map(|(session_id, session)| live_session_state(session_id, session))
        .collect::<Vec<_>>();
    let ledger_states = ledger::list_session_projections()?;

    Ok(merge_a2a_state_sources(ledger_states, live_states))
}

#[tauri::command]
pub async fn review_agent_a2a_tasks(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    task_ids: Vec<String>,
    decision: AgentReviewDecision,
    message: Option<String>,
) -> Result<AgentA2ASessionState, String> {
    let task_ids = normalize_review_task_ids(task_ids)?;
    let message = message.unwrap_or_default();
    let timestamp_ms = now_ms();
    let live_session = state.sessions.read().await.get(&session_id).cloned();
    if let Some(session) = live_session {
        {
            let mut bus = lock_unpoisoned(&session.a2a_bus);
            apply_review_decisions(&mut bus, &task_ids, decision, &message, timestamp_ms)?;
        }
        if let Err(error) = save_session_snapshot_with_workflow(&state, &session).await {
            crate::app_log!("WARN", "[session_snapshot] {}", error);
        }
        session.emit_a2a_projection(&TauriEventEmitter::new(app_handle));
        return Ok(live_session_state(&session_id, &session));
    }

    let mut snapshot = match load_session_snapshot(&session_id) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            crate::app_log!("WARN", "[a2a_review] snapshot load skipped: {}", error);
            None
        }
    };
    let mut bus = if let Some(bus) = snapshot
        .as_mut()
        .and_then(|snapshot| snapshot.a2a_state.take())
    {
        bus
    } else {
        ledger::load_session_ledger(&session_id)?
            .ok_or_else(|| format!("A2A state not found for session '{session_id}'"))?
    };
    apply_review_decisions(&mut bus, &task_ids, decision, &message, timestamp_ms)?;
    let projection = bus.projection();
    if let Some(snapshot) = snapshot {
        save_session_snapshot(&snapshot.with_a2a_state(bus))?;
    } else {
        ledger::save_session_ledger(&session_id, &bus)?;
    }

    Ok(AgentA2ASessionState {
        session_id,
        source: AgentA2AStateSource::Ledger,
        state: projection,
    })
}

fn live_session_state(session_id: &str, session: &Arc<AgentSession>) -> AgentA2ASessionState {
    AgentA2ASessionState {
        session_id: session_id.to_string(),
        source: AgentA2AStateSource::Live,
        state: session.a2a_projection(),
    }
}

fn normalize_review_task_ids(task_ids: Vec<String>) -> Result<Vec<AgentTaskId>, String> {
    let mut normalized = Vec::new();
    for raw in task_ids {
        let task_id = raw.trim();
        if task_id.is_empty() {
            continue;
        }
        if !normalized
            .iter()
            .any(|existing: &AgentTaskId| existing.as_str() == task_id)
        {
            normalized.push(AgentTaskId::new(task_id));
        }
    }
    if normalized.is_empty() {
        return Err("At least one task id is required".to_string());
    }
    Ok(normalized)
}

fn apply_review_decisions(
    bus: &mut AgentA2ABus,
    task_ids: &[AgentTaskId],
    decision: AgentReviewDecision,
    message: &str,
    timestamp_ms: u64,
) -> Result<(), String> {
    let mut next = bus.clone();
    for task_id in task_ids {
        next.record_review_decision(task_id, decision, message.to_string(), timestamp_ms)?;
    }
    *bus = next;
    Ok(())
}

fn merge_a2a_state_sources(
    ledger: AgentA2AProjectionList,
    live: Vec<AgentA2ASessionState>,
) -> AgentA2AStatesPayload {
    let mut states = BTreeMap::new();
    for item in ledger.states {
        states.insert(
            item.session_id.clone(),
            AgentA2ASessionState {
                session_id: item.session_id,
                source: AgentA2AStateSource::Ledger,
                state: item.projection,
            },
        );
    }
    for item in live {
        states.insert(item.session_id.clone(), item);
    }

    AgentA2AStatesPayload {
        states: states.into_values().collect(),
        load_errors: ledger.load_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::ledger::{
        AgentA2ALedgerLoadError, AgentA2AProjectionList, AgentA2ASessionProjection,
    };
    use crate::agent::a2a::projection::AgentA2AProjection;

    #[test]
    fn merge_a2a_state_sources_prefers_live_state_over_ledger() {
        let ledger_projection = AgentA2AProjection {
            completed_count: 1,
            ..AgentA2AProjection::default()
        };
        let live_projection = AgentA2AProjection {
            running_count: 1,
            ..AgentA2AProjection::default()
        };
        let ledger = AgentA2AProjectionList {
            states: vec![AgentA2ASessionProjection {
                session_id: "session-1".to_string(),
                projection: ledger_projection,
            }],
            load_errors: Vec::new(),
        };
        let live = vec![AgentA2ASessionState {
            session_id: "session-1".to_string(),
            source: AgentA2AStateSource::Live,
            state: live_projection,
        }];

        let payload = merge_a2a_state_sources(ledger, live);

        assert_eq!(payload.states.len(), 1);
        assert_eq!(payload.states[0].source, AgentA2AStateSource::Live);
        assert_eq!(payload.states[0].state.running_count, 1);
    }

    #[test]
    fn merge_a2a_state_sources_keeps_ledger_load_errors() {
        let ledger = AgentA2AProjectionList {
            states: Vec::new(),
            load_errors: vec![AgentA2ALedgerLoadError {
                session_id: "session-bad".to_string(),
                message: "parse failed".to_string(),
            }],
        };

        let payload = merge_a2a_state_sources(ledger, Vec::new());

        assert!(payload.states.is_empty());
        assert_eq!(payload.load_errors.len(), 1);
        assert_eq!(payload.load_errors[0].session_id, "session-bad");
    }

    #[test]
    fn normalize_review_task_ids_trims_and_dedupes() {
        let task_ids = normalize_review_task_ids(vec![
            " review-task-1 ".to_string(),
            "".to_string(),
            "review-task-1".to_string(),
            "review-task-2".to_string(),
        ])
        .expect("task ids");

        assert_eq!(
            task_ids
                .iter()
                .map(|task_id| task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["review-task-1", "review-task-2"]
        );
    }

    #[test]
    fn apply_review_decisions_is_all_or_nothing() {
        use crate::agent::a2a::bus::{AgentA2ABus, AgentReviewDecision};
        use crate::agent::a2a::types::{
            AgentArtifact, AgentArtifactKind, AgentExecutionMode, AgentRole, AgentTaskId,
        };

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Reviewable task",
            "Do work",
            10,
        );
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "needs_human_review": true,
                    "worktree_path": "/tmp/reviewable",
                })
                .to_string(),
                created_at_ms: 20,
            },
            20,
        );
        bus.complete_task(&task_id, "Ready", 30);

        let result = apply_review_decisions(
            &mut bus,
            &[task_id.clone(), AgentTaskId::new("missing-task")],
            AgentReviewDecision::Approve,
            "",
            40,
        );

        assert!(result.is_err());
        let projection = bus.projection();
        assert_eq!(projection.tasks[0].needs_human_review, Some(true));
        assert_eq!(projection.tasks[0].review_decision, None);
    }
}
