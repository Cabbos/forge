//! Forge Session CLI — list active gateway sessions.
//!
//! Usage: `forge_session <list>`

use forge::gateway::client::GatewayClient;
use forge::gateway::protocol::GatewayRequest;
use forge::gateway::server::default_socket_path;

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
                    if sessions.is_empty() {
                        println!("No active sessions.");
                    } else {
                        println!("Active sessions:");
                        for s in &sessions {
                            let id = s["session_id"].as_str().unwrap_or("?");
                            let provider = s["provider"].as_str().unwrap_or("?");
                            let model = s["model"].as_str().unwrap_or("?");
                            let workspace = s["workspace_path"].as_str().unwrap_or("?");
                            println!("  {id}  {provider}/{model}  {workspace}");
                        }
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
