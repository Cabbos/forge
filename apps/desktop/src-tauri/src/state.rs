use crate::agent::session::AgentSession;
use crate::pty::session::CliSession;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub enum Session {
    /// PTY-based (bash, shell)
    Cli(Arc<CliSession>),
    /// SDK-based AI agent (Claude API)
    Agent(Arc<AgentSession>),
}

impl Session {
    pub fn id(&self) -> &str {
        match self {
            Session::Cli(s) => &s.id,
            Session::Agent(s) => &s.id,
        }
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        match self {
            Session::Cli(s) => Session::Cli(Arc::clone(s)),
            Session::Agent(s) => Session::Agent(Arc::clone(s)),
        }
    }
}

pub struct AppState {
    pub sessions: RwLock<HashMap<String, Session>>,
    pub pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            pending_confirms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
