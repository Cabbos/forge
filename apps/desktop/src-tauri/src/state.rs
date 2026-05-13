use crate::agent::session::AgentSession;
use crate::harness::Harness;
use crate::memory::WikiMemoryStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;

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
    pub pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    pub harness: Arc<Harness>,
    pub dev_server: Arc<RwLock<Option<ManagedDevServer>>>,
    pub wiki_memory: Arc<WikiMemoryStore>,
}

impl AppState {
    pub fn new(harness: Arc<Harness>) -> Self {
        let pending_confirms = harness.pending_confirms.clone();
        Self {
            sessions: RwLock::new(HashMap::new()),
            pending_confirms,
            harness,
            dev_server: Arc::new(RwLock::new(None)),
            wiki_memory: Arc::new(WikiMemoryStore::default()),
        }
    }
}
