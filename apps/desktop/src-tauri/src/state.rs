use crate::agent::session::{AgentSession, SessionStatus};
use crate::continuity::ContinuityService;
use crate::credential_store::{system_credential_store, CredentialStore};
use crate::forge_wiki::storage::ForgeWikiStore;
use crate::harness::Harness;
use crate::ipc::workspace_terminal::WorkspaceTerminalStore;
use crate::memory::facts::MemoryFactStore;
use crate::memory::WikiMemoryStore;
use crate::profile::ProfileStore;
use crate::protocol::events::DeliverySummary;
use crate::scheduler::SchedulerStore;
use crate::workflow::WorkflowState;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;

const MAX_ACTIVE_SESSIONS: usize = 64;

pub struct ManagedDevServer {
    pub child: Child,
    pub working_dir: std::path::PathBuf,
    pub port: u16,
    pub url: String,
    pub command: String,
    pub logs: Arc<RwLock<Vec<String>>>,
}

pub struct AppState {
    pub sessions: RwLock<HashMap<String, Arc<AgentSession>>>,
    session_order: RwLock<VecDeque<String>>,
    pub pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    pub harness: Arc<Harness>,
    pub dev_server: Arc<RwLock<Option<ManagedDevServer>>>,
    pub wiki_memory: Arc<WikiMemoryStore>,
    pub memory_facts: Arc<MemoryFactStore>,
    pub profiles: Arc<ProfileStore>,
    pub continuity: Arc<ContinuityService>,
    pub forge_wiki: Arc<ForgeWikiStore>,
    pub workflow_states: Arc<RwLock<HashMap<String, WorkflowState>>>,
    pub delivery_states: Arc<RwLock<HashMap<String, DeliverySummary>>>,
    pub scheduler: Arc<SchedulerStore>,
    pub workspace_terminals: Arc<WorkspaceTerminalStore>,
    pub(crate) credential_store: Arc<dyn CredentialStore>,
}

impl AppState {
    pub fn new(harness: Arc<Harness>) -> Self {
        Self::new_with_credential_store(harness, system_credential_store())
    }

    pub(crate) fn new_with_credential_store(
        harness: Arc<Harness>,
        credential_store: Arc<dyn CredentialStore>,
    ) -> Self {
        let pending_confirms = harness.pending_confirms.clone();
        let continuity = Arc::new(ContinuityService::new());
        Self {
            sessions: RwLock::new(HashMap::new()),
            session_order: RwLock::new(VecDeque::new()),
            pending_confirms,
            harness,
            dev_server: Arc::new(RwLock::new(None)),
            wiki_memory: Arc::new(WikiMemoryStore::default()),
            memory_facts: Arc::new(MemoryFactStore::new(MemoryFactStore::default_path())),
            profiles: Arc::new(ProfileStore::new(ProfileStore::default_path())),
            continuity,
            forge_wiki: Arc::new(ForgeWikiStore::new()),
            workflow_states: Arc::new(RwLock::new(HashMap::new())),
            delivery_states: Arc::new(RwLock::new(HashMap::new())),
            scheduler: Arc::new(SchedulerStore::new(SchedulerStore::default_path())),
            workspace_terminals: Arc::new(WorkspaceTerminalStore::default()),
            credential_store,
        }
    }

    pub(crate) fn credential_resolver(&self) -> crate::settings::CredentialResolver {
        crate::settings::CredentialResolver::new(Arc::clone(&self.credential_store))
    }

    pub async fn register_session(&self, session_id: String, session: Arc<AgentSession>) {
        self.register_session_with_limit(session_id, session, MAX_ACTIVE_SESSIONS)
            .await;
    }

    pub async fn unregister_session(&self, session_id: &str) -> Option<Arc<AgentSession>> {
        let mut sessions = self.sessions.write().await;
        let removed = sessions.remove(session_id);
        drop(sessions);

        let mut order = self.session_order.write().await;
        order.retain(|id| id != session_id);
        removed
    }

    async fn register_session_with_limit(
        &self,
        session_id: String,
        session: Arc<AgentSession>,
        max_sessions: usize,
    ) {
        let mut sessions = self.sessions.write().await;
        let mut order = self.session_order.write().await;

        sessions.insert(session_id.clone(), session);
        order.retain(|id| id != &session_id);
        order.push_back(session_id.clone());

        if max_sessions == 0 {
            return;
        }

        while sessions.len() > max_sessions {
            let Some(evict_id) = oldest_evictable_session_id(&sessions, &order, &session_id) else {
                break;
            };

            sessions.remove(&evict_id);
            order.retain(|id| id != &evict_id);
        }
    }
}

fn oldest_evictable_session_id(
    sessions: &HashMap<String, Arc<AgentSession>>,
    order: &VecDeque<String>,
    protected_session_id: &str,
) -> Option<String> {
    order
        .iter()
        .filter(|id| id.as_str() != protected_session_id)
        .find(|id| {
            sessions
                .get(*id)
                .is_some_and(|session| session_can_be_evicted(session))
        })
        .cloned()
}

fn session_can_be_evicted(session: &AgentSession) -> bool {
    matches!(
        &*session.status.lock(),
        SessionStatus::Stopped | SessionStatus::Error(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::session::{AgentSession, SessionStatus};

    fn temp_workspace(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-state-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&path).expect("workspace");
        path
    }

    fn test_session(id: &str, workspace: &std::path::Path) -> Arc<AgentSession> {
        Arc::new(AgentSession::new(
            id.to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            Arc::new(Harness::new(workspace.to_path_buf())),
            "system".to_string(),
            Some(128_000),
        ))
    }

    #[tokio::test]
    async fn register_session_prunes_oldest_stopped_session_when_limit_is_exceeded() {
        let workspace = temp_workspace("prune-stopped");
        let state = AppState::new(Arc::new(Harness::new(workspace.clone())));
        let first = test_session("session-1", &workspace);
        let second = test_session("session-2", &workspace);
        let third = test_session("session-3", &workspace);

        *first.status.lock() = SessionStatus::Stopped;
        *second.status.lock() = SessionStatus::Stopped;

        state
            .register_session_with_limit("session-1".to_string(), first, 2)
            .await;
        state
            .register_session_with_limit("session-2".to_string(), second, 2)
            .await;
        state
            .register_session_with_limit("session-3".to_string(), third, 2)
            .await;

        let sessions = state.sessions.read().await;
        assert!(!sessions.contains_key("session-1"));
        assert!(sessions.contains_key("session-2"));
        assert!(sessions.contains_key("session-3"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn register_session_does_not_prune_running_sessions() {
        let workspace = temp_workspace("keep-running");
        let state = AppState::new(Arc::new(Harness::new(workspace.clone())));

        state
            .register_session_with_limit(
                "session-1".to_string(),
                test_session("session-1", &workspace),
                1,
            )
            .await;
        state
            .register_session_with_limit(
                "session-2".to_string(),
                test_session("session-2", &workspace),
                1,
            )
            .await;

        let sessions = state.sessions.read().await;
        assert!(sessions.contains_key("session-1"));
        assert!(sessions.contains_key("session-2"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn app_state_initializes_project_continuity_service() {
        let workspace = temp_workspace("continuity-service");
        let state = AppState::new(Arc::new(Harness::new(workspace.clone())));
        let event = crate::continuity::ContinuityEvent::UserMessage {
            session_id: "session-1".to_string(),
            content: "继续".to_string(),
            timestamp_ms: 10,
        };

        state
            .continuity
            .record_event(&workspace.to_string_lossy(), &event)
            .expect("record continuity event");

        // DB should be created inside the project's own .forge/ directory,
        // not in the Forge application's working directory.
        assert!(workspace.join(".forge").join("continuity.db").exists());

        let _ = std::fs::remove_dir_all(&workspace);
    }
}
