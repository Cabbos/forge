use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc, Mutex};

use tauri::Emitter;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::harness::Harness;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

const MAX_HISTORY_TURNS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Starting => "starting",
            SessionStatus::Running => "running",
            SessionStatus::Stopped => "stopped",
            SessionStatus::Error(_) => "error",
        }
    }
}

pub struct AgentSession {
    pub id: String,
    pub agent_type: String,
    pub model: String,
    pub status: Arc<Mutex<SessionStatus>>,
    pub(crate) adapter: Box<dyn AiAdapter>,
    pub(crate) messages: Arc<Mutex<Vec<ChatMessage>>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) harness: Arc<Harness>,
    pub(crate) system_prompt: Mutex<String>,
    pub(crate) summary: Mutex<Option<String>>,
}

impl AgentSession {
    pub fn new(
        id: String,
        agent_type: String,
        adapter: Box<dyn AiAdapter>,
        harness: Arc<Harness>,
        system_prompt: String,
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
            harness,
            system_prompt: Mutex::new(system_prompt),
            summary: Mutex::new(None),
        };

        *session.status.lock().unwrap() = SessionStatus::Running;
        session
    }

    pub fn set_system_prompt(&self, prompt: String) {
        *self.system_prompt.lock().unwrap() = prompt;
    }

    /// Send a user message and run the agent loop through the harness.
    pub async fn send_message(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }

        crate::app_log!("INFO", "Agent received user message, history size: {}", self.messages.lock().unwrap().len());

        // Add user message to history
        self.messages.lock().unwrap().push(ChatMessage::user(text));

        // Agent loop: up to 10 tool-call round-trips with final text summary fallback
        for _round in 0..10 {
            if !self.running.load(Ordering::SeqCst) { break; }

            let all_messages = self.messages.lock().unwrap().clone();
            let (messages, new_summary) = window_messages(all_messages);

            if let Some(s) = new_summary {
                let mut current = self.summary.lock().unwrap();
                *current = Some(match current.take() {
                    Some(old) => format!("{} {}", old, s),
                    None => s,
                });
                let mut msgs = self.messages.lock().unwrap();
                let keep = MAX_HISTORY_TURNS.min(msgs.len());
                let drop_count = msgs.len().saturating_sub(keep);
                if drop_count > 0 { msgs.drain(0..drop_count); }
            }

            let summary_ctx = self.summary.lock().unwrap().clone();
            let msgs_with_context = if let Some(ref s) = summary_ctx {
                let mut m = messages.clone();
                m.insert(0, ChatMessage::user(&format!("## Previous conversation summary\n{}", s)));
                m
            } else { messages };

            let mut retries = 0;
            let result = loop {
                match self.adapter.stream_message(&self.id, &msgs_with_context, app_handle).await {
                    Ok(r) => break r,
                    Err(e) => {
                        let msg = e.to_string();
                        if retries < 2 && (msg.contains("500") || msg.contains("503") || msg.contains("429") || msg.contains("timed out")) {
                            retries += 1;
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                        let err_msg = format!("API error: {}", msg);
                        // Don't stop the session — let user retry
                        let _ = app_handle.emit("session-output", StreamEvent::Error {
                            session_id: self.id.clone(),
                            block_id: BlockId::new().to_string(),
                            message: err_msg.clone(),
                            code: "api_error".to_string(),
                        });
                        return Err(err_msg);
                    }
                }
            };

            // Save assistant response
            if !result.assistant_content.is_empty() {
                self.messages.lock().unwrap().push(
                    ChatMessage::assistant(serde_json::Value::Array(result.assistant_content.clone()))
                );
            }

            // No tool calls = done
            if result.tool_calls.is_empty() {
                crate::app_log!("INFO", "Agent turn {}: no tool calls, done", _round);
                break;
            }

            crate::app_log!("INFO", "Agent turn {}: {} tool calls to execute: {:?}", _round, result.tool_calls.len(), result.tool_calls.iter().map(|tc| tc.name.clone()).collect::<Vec<_>>());

            // Execute all tools through the harness (with hooks + permission gating)
            let (reads, writes): (Vec<_>, Vec<_>) = result.tool_calls.iter()
                .partition(|tc| is_read_only_tool(&tc.name));

            // Run reads in parallel through harness
            let mut read_results: Vec<String> = Vec::new();
            {
                let mut handles = Vec::new();
                for tc in &reads {
                    let h = self.harness.clone();
                    let sid = self.id.clone();
                    let name = tc.name.clone();
                    let input = tc.input.clone();
                    let app = app_handle.clone();
                    handles.push(tokio::spawn(async move {
                        h.execute_tool(&sid, &name, &input, &app).await
                    }));
                }
                for handle in handles {
                    if let Ok(result) = handle.await {
                        read_results.push(result);
                    }
                }
            }

            // Run writes sequentially through harness
            let mut write_results: Vec<String> = Vec::new();
            for tc in &writes {
                let result = self.harness.execute_tool(
                    &self.id, &tc.name, &tc.input, app_handle
                ).await;
                write_results.push(result);
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
                crate::app_log!("INFO", "Agent tool '{}' result ({} chars)", tc.name, exec_result.len());
                self.messages.lock().unwrap().push(ChatMessage::tool(&tc.id, &exec_result));
            }

            // Yield briefly so frontend receives & renders events before next API call
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Ensure final text response: append instruction, call API one more time if needed
        {
            let mut msgs = self.messages.lock().unwrap().clone();
            let last_role = msgs.last().map(|m| m.role.clone()).unwrap_or_default();
            if last_role == "tool" || last_role == "user" {
                msgs.push(ChatMessage::user("Based on the above, provide your final answer as plain text. Do not use tools."));
                crate::app_log!("INFO", "Agent loop complete — requesting text-only summary");
                let _ = self.adapter.stream_message(&self.id, &msgs, app_handle).await;
            }
        }

        crate::app_log!("INFO", "Agent loop complete");
        Ok(())
    }

    pub fn kill(&self, app_handle: &tauri::AppHandle) {
        self.running.store(false, Ordering::SeqCst);
        *self.status.lock().unwrap() = SessionStatus::Stopped;
        let _ = app_handle.emit("session-output",
            StreamEvent::SessionStopped { session_id: self.id.clone(), reason: "killed".to_string() });
    }
}

fn window_messages(mut msgs: Vec<ChatMessage>) -> (Vec<ChatMessage>, Option<String>) {
    if msgs.len() <= MAX_HISTORY_TURNS { return (msgs, None); }
    let split_at = msgs.len().saturating_sub(MAX_HISTORY_TURNS);
    let start = (split_at..msgs.len())
        .find(|&i| msgs[i].role == "user" && !is_tool_result(&msgs[i]))
        .unwrap_or(split_at);
    let dropped: Vec<_> = msgs.iter().take(start).collect();
    let summary = build_summary(&dropped);
    msgs.drain(0..start);
    (msgs, summary)
}

fn build_summary(msgs: &[&ChatMessage]) -> Option<String> {
    let user_msgs: Vec<&str> = msgs.iter()
        .filter(|m| m.role == "user" && !is_tool_result(m))
        .filter_map(|m| {
            if let serde_json::Value::String(ref s) = m.content { Some(s.as_str()) }
            else if let serde_json::Value::Array(ref blocks) = m.content {
                blocks.iter().filter_map(|b| b.get("text").and_then(|t| t.as_str())).next()
            } else { None }
        }).collect();
    if user_msgs.is_empty() { return None; }
    let mut summary = String::from("[Earlier conversation:\n");
    for msg in user_msgs.iter().take(10) {
        summary.push_str(&format!("- {}\n", &msg.chars().take(200).collect::<String>()));
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
    } else { false }
}
