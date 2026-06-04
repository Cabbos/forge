use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Notify;

use super::base::{AdapterError, AiAdapter, ChatMessage, StreamResult};
use crate::agent::event_sink::EventEmitter;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

pub struct MissingKeyAdapter {
    provider_label: String,
    model: String,
}

impl MissingKeyAdapter {
    pub fn new(provider_label: &str, model: &str) -> Self {
        Self {
            provider_label: provider_label.to_string(),
            model: model.to_string(),
        }
    }

    fn message(&self) -> String {
        format!(
            "还没有配置 {} API Key。请打开设置，粘贴密钥后就可以开始发送。",
            self.provider_label
        )
    }
}

#[async_trait]
impl AiAdapter for MissingKeyAdapter {
    fn model_id(&self) -> &str {
        &self.model
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn is_missing_api_key_adapter(&self) -> bool {
        true
    }

    async fn call(
        &self,
        _messages: &[ChatMessage],
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        Err(AdapterError::MissingApiKey)
    }

    async fn stream_message_with_emitter(
        &self,
        session_id: &str,
        _messages: &[ChatMessage],
        emitter: &dyn EventEmitter,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        emitter.emit(StreamEvent::Error {
            session_id: session_id.to_string(),
            block_id: BlockId::new().to_string(),
            message: self.message(),
            code: "missing_api_key".to_string(),
        });

        Ok(StreamResult {
            assistant_content: Vec::new(),
            tool_calls: Vec::new(),
            stop_reason: Some("missing_api_key".to_string()),
        })
    }
}
