use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use std::io::Read;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Emitter;

/// Parse stream-json output from Claude Code.
/// Each line is a JSON object with a "type" field.
/// We extract text from assistant messages and tool results.
fn parse_line(line: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = parsed.get("type")?.as_str()?;

    match event_type {
        // Assistant text content
        "assistant" => {
            let content = parsed.get("message")?.get("content")?;
            if let Some(items) = content.as_array() {
                let texts: Vec<&str> = items
                    .iter()
                    .filter_map(|item| {
                        item.get("type")
                            .and_then(|t| t.as_str())
                            .and_then(|t| match t {
                                "text" => item.get("text")?.as_str(),
                                _ => None,
                            })
                    })
                    .collect();
                if texts.is_empty() { None } else { Some(texts.join("")) }
            } else {
                // String content
                content.as_str().map(|s| s.to_string())
            }
        }
        // Tool usage
        "tool_use" => {
            let name = parsed.get("name")?.as_str().unwrap_or("tool");
            let input = parsed.get("input").map(|i| i.to_string()).unwrap_or_default();
            Some(format!("\n> Running: {}\n  Input: {}", name, input))
        }
        // Tool result
        "tool_result" => {
            let content = parsed.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if content.is_empty() { None } else { Some(format!("> Result:\n{}", content)) }
        }
        // User message echo
        "user" => None, // skip echoing user's own messages
        // System messages
        "system" => {
            parsed.get("message")?.as_str().map(|s| format!("# {}\n", s))
        }
        // Error
        "error" => {
            let msg = parsed.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            Some(format!("[Error] {}", msg))
        }
        _ => None,
    }
}

pub fn read_loop(
    session_id: String,
    mut reader: Box<dyn Read + Send>,
    running: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
) {
    let block_id = BlockId::new().to_string();
    let mut error_count = 0u32;
    let mut started = false;
    let mut line_buf = String::new();

    let mut buf = [0u8; 4096];

    loop {
        if !running.load(Ordering::SeqCst) {
            break;
        }

        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => { error_count = 0; n }
            Err(e) => {
                error_count += 1;
                crate::app_log!("ERROR", "PTY read ({}/10): {}", error_count, e);
                if error_count > 10 { break; }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
        };

        let chunk = String::from_utf8_lossy(&buf[..n]);
        line_buf.push_str(&chunk);

        // Process complete JSON lines
        while let Some(pos) = line_buf.find('\n') {
            let line = line_buf[..pos].trim().to_string();
            line_buf = line_buf[pos + 1..].to_string();

            let text = match parse_line(&line) {
                Some(t) => t,
                None => continue,
            };

            if !started {
                started = true;
                let _ = app_handle.emit(
                    "session-output",
                    StreamEvent::TextStart {
                        session_id: session_id.clone(),
                        block_id: block_id.clone(),
                    },
                );
            }

            let _ = app_handle.emit(
                "session-output",
                StreamEvent::TextChunk {
                    session_id: session_id.clone(),
                    block_id: block_id.clone(),
                    content: text,
                },
            );
        }
    }

    if started {
        let _ = app_handle.emit(
            "session-output",
            StreamEvent::TextEnd {
                session_id: session_id.clone(),
                block_id: block_id.clone(),
            },
        );
    }

    let _ = app_handle.emit(
        "session-output",
        StreamEvent::SessionStopped {
            session_id: session_id.clone(),
            reason: "process_exited".to_string(),
        },
    );
}
