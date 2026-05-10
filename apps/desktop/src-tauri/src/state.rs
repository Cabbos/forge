use crate::agent::session::AgentSession;
use crate::harness::Harness;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub sessions: RwLock<HashMap<String, Arc<AgentSession>>>,
    pub pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    pub harness: Arc<Harness>,
}

impl AppState {
    pub fn new(harness: Arc<Harness>) -> Self {
        let pending_confirms = harness.pending_confirms.clone();
        Self {
            sessions: RwLock::new(HashMap::new()),
            pending_confirms,
            harness,
        }
    }
}
