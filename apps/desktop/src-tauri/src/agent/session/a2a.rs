use crate::agent::event_sink::EventEmitter;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;

impl AgentSession {
    pub(crate) fn emit_a2a_projection(&self, emitter: &dyn EventEmitter) {
        let state = lock_unpoisoned(&self.a2a_bus).projection();
        emitter.emit(crate::protocol::events::StreamEvent::AgentA2AUpdated {
            session_id: self.id.clone(),
            state,
        });
    }
}
