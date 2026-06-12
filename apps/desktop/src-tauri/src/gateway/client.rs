//! Gateway client — connect to the gateway Unix socket, send JSON-line
//! requests, and parse responses.

use crate::gateway::protocol::{GatewayReply, GatewayRequest};
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
}
