//! Forge Session CLI — list active gateway sessions.
//!
//! Usage: `forge_session <list>`

use forge::gateway::client::GatewayClient;
use forge::gateway::protocol::GatewayRequest;
use forge::gateway::server::{default_socket_path, SESSION_STALE_AFTER_MS};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("list");

    if cmd != "list" {
        eprintln!("Usage: forge_session list");
        std::process::exit(1);
    }

    let socket_path = default_socket_path();
    if !socket_path.exists() {
        println!(
            "Gateway is not running (no socket at {}).",
            socket_path.display()
        );
        println!("Start it with: forge service start");
        return;
    }

    match GatewayClient::connect(&socket_path).await {
        Ok(mut client) => {
            let reply = client
                .send(GatewayRequest {
                    id: uuid::Uuid::now_v7().simple().to_string(),
                    method: "list_sessions".to_string(),
                    params: None,
                })
                .await;

            match reply {
                Ok(forge::gateway::protocol::GatewayReply::Ok(resp)) => {
                    let sessions: Vec<serde_json::Value> =
                        serde_json::from_value(resp.result).unwrap_or_default();
                    for line in render_session_lines(&sessions) {
                        println!("{line}");
                    }
                }
                Ok(forge::gateway::protocol::GatewayReply::Err(err)) => {
                    eprintln!(
                        "Gateway error: {} (code: {})",
                        err.error.message, err.error.code
                    );
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Request failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to gateway: {e}");
            std::process::exit(1);
        }
    }
}

fn render_session_lines(sessions: &[serde_json::Value]) -> Vec<String> {
    if sessions.is_empty() {
        return vec!["No gateway sessions.".to_string()];
    }

    let mut lines = Vec::with_capacity(sessions.len() + 1);
    lines.push("Gateway sessions:".to_string());
    for session in sessions {
        let id = session["session_id"].as_str().unwrap_or("?");
        let provider = session["provider"].as_str().unwrap_or("?");
        let model = session["model"].as_str().unwrap_or("?");
        let workspace = session["workspace_path"].as_str().unwrap_or("?");
        let state = session_state_label(session);
        lines.push(format!("  {id}  {state}  {provider}/{model}  {workspace}"));
    }
    lines
}

fn session_state_label(session: &serde_json::Value) -> &'static str {
    if session["restored_from_registry"].as_bool().unwrap_or(false) {
        return "restored";
    }

    if let Some(last_seen_at_ms) = session["last_seen_at_ms"].as_u64() {
        if now_millis().saturating_sub(last_seen_at_ms) > SESSION_STALE_AFTER_MS {
            return "stale";
        }
    }

    "active"
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    #[test]
    fn render_session_lines_distinguishes_active_and_restored_sessions() {
        let sessions = vec![
            serde_json::json!({
                "session_id": "session-live",
                "provider": "claude",
                "model": "opus",
                "workspace_path": "/repo/live",
                "restored_from_registry": false
            }),
            serde_json::json!({
                "session_id": "session-restored",
                "provider": "codex",
                "model": "gpt-5",
                "workspace_path": "/repo/restored",
                "restored_from_registry": true
            }),
        ];

        let lines = super::render_session_lines(&sessions);

        assert_eq!(lines[0], "Gateway sessions:");
        assert_eq!(lines[1], "  session-live  active  claude/opus  /repo/live");
        assert_eq!(
            lines[2],
            "  session-restored  restored  codex/gpt-5  /repo/restored"
        );
    }

    #[test]
    fn render_session_lines_marks_stale_live_sessions() {
        let sessions = vec![serde_json::json!({
            "session_id": "session-stale",
            "provider": "claude",
            "model": "opus",
            "workspace_path": "/repo/stale",
            "last_seen_at_ms": 1,
            "restored_from_registry": false
        })];

        let lines = super::render_session_lines(&sessions);

        assert_eq!(lines[1], "  session-stale  stale  claude/opus  /repo/stale");
    }
}
