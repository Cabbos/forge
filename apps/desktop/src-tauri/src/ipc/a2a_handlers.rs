use std::collections::BTreeMap;
use std::sync::Arc;

use crate::agent::a2a::ledger::{self, AgentA2ALedgerLoadError, AgentA2AProjectionList};
use crate::agent::a2a::projection::AgentA2AProjection;
use crate::agent::session::AgentSession;
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

fn live_session_state(session_id: &str, session: &Arc<AgentSession>) -> AgentA2ASessionState {
    AgentA2ASessionState {
        session_id: session_id.to_string(),
        source: AgentA2AStateSource::Live,
        state: session.a2a_projection(),
    }
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
}
