use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use std::sync::Arc;
use tauri::Emitter;

/// Unified event dispatch for the harness.
/// Wraps Tauri's event emitter with structured StreamEvent creation.
#[derive(Clone)]
pub struct EventBus {
    app_handle: Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            app_handle: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn set_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    fn emit(&self, event: StreamEvent) {
        if let Some(ref h) = *self.app_handle.lock().unwrap() {
            let _ = h.emit("session-output", event);
        }
    }

    pub fn thinking_start(&self, session_id: &str, block_id: &str) {
        self.emit(StreamEvent::ThinkingStart {
            session_id: session_id.into(),
            block_id: block_id.into(),
        });
    }

    pub fn thinking_chunk(&self, session_id: &str, block_id: &str, content: &str) {
        self.emit(StreamEvent::ThinkingChunk {
            session_id: session_id.into(),
            block_id: block_id.into(),
            content: content.into(),
        });
    }

    pub fn thinking_end(&self, session_id: &str, block_id: &str) {
        self.emit(StreamEvent::ThinkingEnd {
            session_id: session_id.into(),
            block_id: block_id.into(),
        });
    }

    pub fn text_start(&self, session_id: &str, block_id: &str) {
        self.emit(StreamEvent::TextStart {
            session_id: session_id.into(),
            block_id: block_id.into(),
        });
    }

    pub fn text_chunk(&self, session_id: &str, block_id: &str, content: &str) {
        self.emit(StreamEvent::TextChunk {
            session_id: session_id.into(),
            block_id: block_id.into(),
            content: content.into(),
        });
    }

    pub fn text_end(&self, session_id: &str, block_id: &str) {
        self.emit(StreamEvent::TextEnd {
            session_id: session_id.into(),
            block_id: block_id.into(),
        });
    }

    pub fn tool_start(
        &self,
        session_id: &str,
        block_id: &str,
        name: &str,
        input: serde_json::Value,
    ) {
        self.emit(StreamEvent::ToolCallStart {
            session_id: session_id.into(),
            block_id: block_id.into(),
            tool_name: name.into(),
            tool_input: input,
        });
    }

    pub fn tool_result(
        &self,
        session_id: &str,
        block_id: &str,
        result: &str,
        is_error: bool,
        duration_ms: u64,
    ) {
        self.emit(StreamEvent::ToolCallResult {
            session_id: session_id.into(),
            block_id: block_id.into(),
            result: result.into(),
            is_error,
            duration_ms,
        });
    }

    pub fn tool_end(&self, session_id: &str, block_id: &str) {
        self.emit(StreamEvent::ToolCallEnd {
            session_id: session_id.into(),
            block_id: block_id.into(),
        });
    }

    pub fn shell_start(&self, session_id: &str, block_id: &str, command: &str) {
        self.emit(StreamEvent::ShellStart {
            session_id: session_id.into(),
            block_id: block_id.into(),
            command: command.into(),
        });
    }

    pub fn shell_output(&self, session_id: &str, block_id: &str, content: &str) {
        self.emit(StreamEvent::ShellOutput {
            session_id: session_id.into(),
            block_id: block_id.into(),
            content: content.into(),
        });
    }

    pub fn shell_end(&self, session_id: &str, block_id: &str, exit_code: i32) {
        self.emit(StreamEvent::ShellEnd {
            session_id: session_id.into(),
            block_id: block_id.into(),
            exit_code,
        });
    }

    pub fn confirm_ask(&self, session_id: &str, block_id: &str, question: &str, kind: &str) {
        self.emit(StreamEvent::ConfirmAsk {
            session_id: session_id.into(),
            block_id: block_id.into(),
            question: question.into(),
            kind: kind.into(),
            boundary: None,
        });
    }

    pub fn session_started(&self, session_id: &str, agent_type: &str, model: &str) {
        self.emit(StreamEvent::SessionStarted {
            session_id: session_id.into(),
            agent_type: agent_type.into(),
            model: model.into(),
            context_window_tokens: None,
        });
    }

    pub fn session_stopped(&self, session_id: &str, reason: &str) {
        self.emit(StreamEvent::SessionStopped {
            session_id: session_id.into(),
            reason: reason.into(),
        });
    }

    pub fn error(&self, session_id: &str, message: &str, code: &str) {
        let block_id = BlockId::new().to_string();
        self.emit(StreamEvent::Error {
            session_id: session_id.into(),
            block_id,
            message: message.into(),
            code: code.into(),
        });
    }

    pub fn usage(&self, session_id: &str, input_tokens: u32, output_tokens: u32, cost_usd: f64) {
        self.emit(StreamEvent::Usage {
            session_id: session_id.into(),
            input_tokens,
            output_tokens,
            estimated_cost_usd: cost_usd,
        });
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
