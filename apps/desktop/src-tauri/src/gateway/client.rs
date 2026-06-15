//! Gateway client — connect to the gateway Unix socket, send JSON-line
//! requests, and parse responses.

use crate::gateway::protocol::{GatewayReply, GatewayRequest, GatewaySessionInfo};
use crate::gateway::server::default_socket_path;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// A connected gateway client.
pub struct GatewayClient {
    stream: UnixStream,
}

impl GatewayClient {
    /// Connect to the gateway at the given socket path.
    pub async fn connect(socket_path: &PathBuf) -> Result<Self, String> {
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| format!("connect to gateway: {e}"))?;
        Ok(Self { stream })
    }

    /// Send a request and wait for the reply.
    ///
    /// Serializes the request as a JSON line, writes it to the socket, then
    /// reads one JSON line back and deserializes it as a `GatewayReply`.
    pub async fn send(&mut self, request: GatewayRequest) -> Result<GatewayReply, String> {
        let mut json = serde_json::to_string(&request).map_err(|e| format!("serialize: {e}"))?;
        json.push('\n');

        self.stream
            .write_all(json.as_bytes())
            .await
            .map_err(|e| format!("write: {e}"))?;

        let (reader, _) = self.stream.split();
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        buf_reader
            .read_line(&mut line)
            .await
            .map_err(|e| format!("read: {e}"))?;

        // Need to re-join because split consumed the stream.
        // For now we re-create the split each time (simple, works for
        // connection-per-request pattern).
        serde_json::from_str::<GatewayReply>(line.trim())
            .map_err(|e| format!("deserialize reply: {e}"))
    }
}

/// Send a single `ping` request to the gateway and return the reply.
///
/// Convenience function for health checks.
pub async fn ping(socket_path: &PathBuf) -> Result<GatewayReply, String> {
    let mut client = GatewayClient::connect(socket_path).await?;
    client
        .send(GatewayRequest {
            id: uuid::Uuid::now_v7().simple().to_string(),
            method: "ping".to_string(),
            params: None,
        })
        .await
}

/// Send a single `health` request to the gateway and return the reply.
pub async fn health(socket_path: &PathBuf) -> Result<GatewayReply, String> {
    let mut client = GatewayClient::connect(socket_path).await?;
    client
        .send(GatewayRequest {
            id: uuid::Uuid::now_v7().simple().to_string(),
            method: "health".to_string(),
            params: None,
        })
        .await
}

pub fn build_register_session_request(info: GatewaySessionInfo) -> Result<GatewayRequest, String> {
    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "register_session".to_string(),
        params: Some(
            serde_json::to_value(info).map_err(|error| format!("serialize session: {error}"))?,
        ),
    })
}

pub fn build_unregister_session_request(session_id: &str) -> Result<GatewayRequest, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "unregister_session".to_string(),
        params: Some(serde_json::json!({ "session_id": session_id })),
    })
}

pub fn build_attach_session_request(session_id: &str) -> Result<GatewayRequest, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    Ok(GatewayRequest {
        id: uuid::Uuid::now_v7().simple().to_string(),
        method: "attach_session".to_string(),
        params: Some(serde_json::json!({ "session_id": session_id })),
    })
}

pub async fn try_register_session(info: GatewaySessionInfo) -> Result<(), String> {
    let request = build_register_session_request(info)?;
    send_best_effort_gateway_request(request).await
}

pub async fn try_unregister_session(session_id: &str) -> Result<(), String> {
    let request = build_unregister_session_request(session_id)?;
    send_best_effort_gateway_request(request).await
}

async fn send_best_effort_gateway_request(request: GatewayRequest) -> Result<(), String> {
    let socket_path = default_socket_path();
    if !socket_path.exists() {
        return Err(format!(
            "gateway socket is not available at {}",
            socket_path.display()
        ));
    }
    let mut client = GatewayClient::connect(&socket_path).await?;
    match client.send(request).await? {
        GatewayReply::Ok(_) => Ok(()),
        GatewayReply::Err(error) => Err(format!("gateway error: {}", error.error.message)),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_request_serializes_to_json_line() {
        let req = GatewayRequest {
            id: "test-1".into(),
            method: "ping".into(),
            params: None,
        };
        let mut json = serde_json::to_string(&req).expect("serialize");
        json.push('\n');
        assert!(json.ends_with('\n'));
        assert!(json.contains("\"ping\""));
        assert!(json.contains("\"test-1\""));
    }

    #[test]
    fn gateway_reply_deserializes_ok() {
        let json = r#"{"id":"1","result":{"ok":true,"gateway_version":"0.1.0"}}"#;
        let reply: GatewayReply = serde_json::from_str(json).expect("deserialize");
        match reply {
            GatewayReply::Ok(resp) => {
                assert_eq!(resp.id, "1");
            }
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn gateway_reply_deserializes_error() {
        let json = r#"{"id":"2","error":{"code":-1,"message":"bad"}}"#;
        let reply: GatewayReply = serde_json::from_str(json).expect("deserialize");
        match reply {
            GatewayReply::Err(err) => {
                assert_eq!(err.id, "2");
                assert_eq!(err.error.code, -1);
            }
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn register_session_request_carries_session_metadata() {
        let request = build_register_session_request(GatewaySessionInfo {
            session_id: "session-1".into(),
            provider: "openai".into(),
            model: "gpt-5".into(),
            workspace_path: "/repo".into(),
            created_at_ms: 1234,
            owner_pid: Some(42),
            last_seen_at_ms: Some(5678),
            restored_from_registry: false,
        })
        .expect("request");

        assert_eq!(request.method, "register_session");
        let params = request.params.expect("params");
        assert_eq!(params["session_id"], "session-1");
        assert_eq!(params["provider"], "openai");
        assert_eq!(params["model"], "gpt-5");
        assert_eq!(params["workspace_path"], "/repo");
        assert_eq!(params["created_at_ms"], 1234);
        assert_eq!(params["owner_pid"], 42);
        assert_eq!(params["last_seen_at_ms"], 5678);
        assert_eq!(params["restored_from_registry"], false);
    }

    #[test]
    fn unregister_session_request_trims_session_id() {
        let request = build_unregister_session_request(" session-1 ").expect("request");

        assert_eq!(request.method, "unregister_session");
        let params = request.params.expect("params");
        assert_eq!(params["session_id"], "session-1");
    }

    #[test]
    fn attach_session_request_trims_session_id() {
        let request = build_attach_session_request(" session-1 ").expect("request");

        assert_eq!(request.method, "attach_session");
        let params = request.params.expect("params");
        assert_eq!(params["session_id"], "session-1");
    }
}
