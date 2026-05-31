use crate::protocol::events::StreamEvent;

/// Abstraction over event emission so the agent loop can run without a real
/// Tauri `AppHandle`.  Production code wraps `AppHandle`; tests use
/// [`NoopEventEmitter`] or [`CollectingEventEmitter`].
pub trait EventEmitter: Send + Sync {
    fn emit(&self, event: StreamEvent);
}

/// Production emitter that delegates to `transcript::emit_stream_event`.
pub(crate) struct TauriEventEmitter {
    app_handle: tauri::AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

impl EventEmitter for TauriEventEmitter {
    fn emit(&self, event: StreamEvent) {
        crate::transcript::emit_stream_event(&self.app_handle, event);
    }
}

/// No-op emitter for tests that only care about message history correctness.
pub struct NoopEventEmitter;

impl EventEmitter for NoopEventEmitter {
    fn emit(&self, _event: StreamEvent) {
        // discard
    }
}

/// Collecting emitter for tests that need to assert on emitted events.
#[allow(dead_code)]
pub(crate) struct CollectingEventEmitter {
    pub events: parking_lot::Mutex<Vec<StreamEvent>>,
}

#[allow(dead_code)]
impl CollectingEventEmitter {
    pub fn new() -> Self {
        Self {
            events: parking_lot::Mutex::new(Vec::new()),
        }
    }

    pub fn drain(&self) -> Vec<StreamEvent> {
        std::mem::take(&mut *self.events.lock())
    }
}

impl EventEmitter for CollectingEventEmitter {
    fn emit(&self, event: StreamEvent) {
        self.events.lock().push(event);
    }
}
