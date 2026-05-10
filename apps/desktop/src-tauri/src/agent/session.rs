use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc, Mutex};

use futures::future;
use tauri::Emitter;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::executor::ToolExecutor;
use crate::protocol::commands::AgentType;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

/// Maximum number of conversation turns to keep in working memory.
/// Older messages are summarized into a system prompt.
const MAX_HISTORY_TURNS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

pub struct AgentSession {
    pub id: String,
    pub agent_type: AgentType,
    pub model: String,
    pub status: Arc<Mutex<SessionStatus>>,
    pub(crate) adapter: Box<dyn AiAdapter>,
    pub(crate) messages: Arc<Mutex<Vec<ChatMessage>>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) executor: Option<ToolExecutor>,
    pub(crate) summary: Mutex<Option<String>>,
}

impl AgentSession {
    pub fn new(
        id: String,
        agent_type: AgentType,
        adapter: Box<dyn AiAdapter>,
        executor: Option<ToolExecutor>,
        _app_handle: &tauri::AppHandle,
    ) -> Self {
        let model = adapter.model_name().to_string();

        let session = AgentSession {
            id: id.clone(),
            agent_type: agent_type.clone(),
            model: model.clone(),
            status: Arc::new(Mutex::new(SessionStatus::Starting)),
            adapter,
            messages: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(true)),
            executor,
            summary: Mutex::new(None),
        };

        *session.status.lock().unwrap() = SessionStatus::Running;

        session
    }

    /// Send a user message and run the agent loop:
    ///   user_msg → API → (tool_use → execute → API →)* → final response
    pub async fn send_message(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }

        // Add user message to history
        {
            let mut messages = self.messages.lock().unwrap();
            messages.push(ChatMessage::user(text));
        }

        // Agent loop: up to 10 tool-call round-trips
        for _round in 0..10 {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            let all_messages = self.messages.lock().unwrap().clone();
            let (messages, new_summary) = window_messages(all_messages);

            // Accumulate summary across multiple window operations
            if let Some(s) = new_summary {
                let mut current = self.summary.lock().unwrap();
                *current = Some(match current.take() {
                    Some(old) => format!("{} {}", old, s),
                    None => s,
                });
                // Trim canonical messages to prevent unbounded growth
                let mut msgs = self.messages.lock().unwrap();
                let keep = MAX_HISTORY_TURNS.min(msgs.len());
                let drop_count = msgs.len().saturating_sub(keep);
                if drop_count > 0 {
                    msgs.drain(0..drop_count);
                }
            }

            // Add summary context to messages if available
            let summary_ctx = self.summary.lock().unwrap().clone();
            let msgs_with_context = if let Some(ref s) = summary_ctx {
                let mut m = messages.clone();
                m.insert(0, ChatMessage::user(&format!("## Previous conversation summary\n{}", s)));
                m
            } else { messages };

            let result = match self
                .adapter
                .stream_message(&self.id, &msgs_with_context, app_handle)
                .await
            {
                Ok(r) => {
                    crate::app_log!("INFO", "Agent turn {}: {} content blocks, {} tool calls",
                        _round, r.assistant_content.len(), r.tool_calls.len());
                    if !r.assistant_content.is_empty() {
                        crate::app_log!("INFO", "Content types: {:?}",
                            r.assistant_content.iter().map(|c| c.get("type").and_then(|t| t.as_str()).unwrap_or("?")).collect::<Vec<_>>());
                    }
                    r
                }
                Err(e) => {
                    let err_msg = format!("API error: {}", e);
                    crate::app_log!("ERROR", "Agent turn {} failed: {}", _round, err_msg);
                    *self.status.lock().unwrap() = SessionStatus::Error(err_msg.clone());
                    let _ = app_handle.emit("session-output", StreamEvent::Error {
                        session_id: self.id.clone(),
                        block_id: BlockId::new().to_string(),
                        message: err_msg.clone(),
                        code: "api_error".to_string(),
                    });
                    let _ = app_handle.emit("session-output", StreamEvent::SessionStopped {
                        session_id: self.id.clone(),
                        reason: err_msg.clone(),
                    });
                    return Err(err_msg);
                }
            };

            // Save assistant response to history
            let content_count = result.assistant_content.len();
            if content_count > 0 {
                let mut msgs = self.messages.lock().unwrap();
                msgs.push(ChatMessage::assistant(serde_json::Value::Array(result.assistant_content.clone())));
                crate::app_log!("INFO", "Saved {} content blocks to history, total msgs: {}", content_count, msgs.len());
            } else {
                crate::app_log!("WARN", "Agent turn {} returned NO assistant content", _round);
            }

            // If no tool calls, agent is done
            if result.tool_calls.is_empty() {
                break;
            }

            // Execute tool calls — read-only tools run in parallel
            if let Some(ref executor) = self.executor {
                // Split tools into read-only and write groups
                let (reads, writes): (Vec<_>, Vec<_>) = result.tool_calls.iter()
                    .partition(|tc| is_read_only_tool(&tc.name));

                // Run read-only tools in parallel
                let mut futures = Vec::new();
                for tc in &reads {
                    futures.push(executor.execute(&self.id, &tc.name, &tc.input, app_handle));
                }
                let read_results = future::join_all(futures).await;

                // Run write tools sequentially (each may modify state)
                let mut write_results = Vec::new();
                for tc in &writes {
                    write_results.push(executor.execute(&self.id, &tc.name, &tc.input, app_handle).await);
                }

                // Feed results back in original order
                let mut ri = 0usize;
                let mut wi = 0usize;
                for tc in &result.tool_calls {
                    let exec_result = if is_read_only_tool(&tc.name) {
                        let r = read_results[ri].clone();
                        ri += 1;
                        r
                    } else {
                        let r = write_results[wi].clone();
                        wi += 1;
                        r
                    };
                    let mut msgs = self.messages.lock().unwrap();
                    msgs.push(ChatMessage::tool_result(&tc.id, &exec_result));
                }
            } else {
                // No executor — just add placeholder tool results
                for tc in &result.tool_calls {
                    let mut msgs = self.messages.lock().unwrap();
                    msgs.push(ChatMessage::tool_result(
                        &tc.id,
                        &format!("Tool {} not available (no executor configured)", tc.name),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Kill the session.
    pub fn kill(&self, app_handle: &tauri::AppHandle) {
        self.running.store(false, Ordering::SeqCst);
        *self.status.lock().unwrap() = SessionStatus::Stopped;

        let _ = app_handle.emit(
            "session-output",
            StreamEvent::SessionStopped {
                session_id: self.id.clone(),
                reason: "killed".to_string(),
            },
        );
    }
}

/// Window messages to MAX_HISTORY_TURNS. Returns kept messages and a summary of dropped content.
fn window_messages(mut msgs: Vec<ChatMessage>) -> (Vec<ChatMessage>, Option<String>) {
    if msgs.len() <= MAX_HISTORY_TURNS {
        return (msgs, None);
    }
    let split_at = msgs.len().saturating_sub(MAX_HISTORY_TURNS);
    let start = (split_at..msgs.len())
        .find(|&i| {
            let role = msgs[i].role.as_str();
            role == "user" && !is_tool_result(&msgs[i])
        })
        .unwrap_or(split_at);

    // Extract key info from dropped messages for summarization
    let dropped: Vec<_> = msgs.iter().take(start).collect();
    let summary = build_summary(&dropped);

    msgs.drain(0..start);
    (msgs, summary)
}

/// Build a brief summary of dropped conversation content.
fn build_summary(msgs: &[&ChatMessage]) -> Option<String> {
    let user_msgs: Vec<&str> = msgs.iter()
        .filter(|m| m.role == "user" && !is_tool_result(m))
        .filter_map(|m| {
            if let serde_json::Value::String(ref s) = m.content {
                Some(s.as_str())
            } else if let serde_json::Value::Array(ref blocks) = m.content {
                blocks.iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .next()
            } else {
                None
            }
        })
        .collect();

    if user_msgs.is_empty() { return None; }

    let mut summary = String::from("[Earlier conversation:\n");
    for msg in user_msgs.iter().take(10) {
        let truncated: String = msg.chars().take(200).collect();
        summary.push_str(&format!("- {}\n", truncated));
    }
    summary.push(']');
    Some(summary)
}

fn is_read_only_tool(name: &str) -> bool {
    matches!(name, "read_file" | "read" | "list_directory" | "ls" | "list"
        | "search_files" | "glob" | "search_content" | "grep"
        | "web_search" | "web_fetch")
}

fn is_tool_result(msg: &ChatMessage) -> bool {
    if let serde_json::Value::Array(ref blocks) = msg.content {
        blocks.iter().any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
    } else {
        false
    }
}
