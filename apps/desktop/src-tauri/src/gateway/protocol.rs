//! Gateway IPC protocol — JSON-line message types.
//!
//! The gateway listens on a Unix domain socket and accepts one JSON object
//! per line.  Every request carries a unique `id`; the response echoes that
//! `id` so clients can correlate.

use serde::{Deserialize, Serialize};

/// An incoming request from a gateway client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRequest {
    /// Client-generated correlation id.
    pub id: String,
    /// Method name, e.g. "ping", "health".
    pub method: String,
    /// Optional parameters (serde_json::Value lets each method define its
    /// own shape).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A successful gateway response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayResponse {
    pub id: String,
    pub result: serde_json::Value,
}

/// An error gateway response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayError {
    pub id: String,
    pub error: GatewayErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayErrorBody {
    pub code: i32,
    pub message: String,
}

/// Union of possible gateway replies produced by a handler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum GatewayReply {
    Ok(GatewayResponse),
    Err(GatewayError),
}

// ── Well-known methods ──────────────────────────────────────────────────────

/// Result of the `ping` method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingResult {
    pub ok: bool,
    pub gateway_version: String,
}

/// Result of the `health` method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResult {
    pub ok: bool,
    pub uptime_seconds: u64,
    pub active_sessions: usize,
    pub gateway_version: String,
}

/// Gateway version string.
pub const GATEWAY_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── Serialization helpers ───────────────────────────────────────────────────

/// Serialize a GatewayReply to a JSON line (one JSON object + newline).
pub fn serialize_reply(reply: &GatewayReply) -> Result<String, String> {
    let mut json = serde_json::to_string(reply).map_err(|e| format!("serialize: {e}"))?;
    json.push('\n');
    Ok(json)
}

/// Deserialize a GatewayRequest from a JSON line.
pub fn deserialize_request(line: &str) -> Result<GatewayRequest, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("empty line".to_string());
    }
    serde_json::from_str(trimmed).map_err(|e| format!("deserialize: {e}"))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Request deserialization ──────────────────────────────────────────

    #[test]
    fn deserialize_ping_request() {
        let json = r#"{"id":"1","method":"ping"}"#;
        let req = deserialize_request(json).expect("deserialize");
        assert_eq!(req.id, "1");
        assert_eq!(req.method, "ping");
        assert_eq!(req.params, None);
    }

    #[test]
    fn deserialize_request_with_params() {
        let json = r#"{"id":"2","method":"create_session","params":{"provider":"deepseek"}}"#;
        let req = deserialize_request(json).expect("deserialize");
        assert_eq!(req.id, "2");
        assert_eq!(req.method, "create_session");
        assert!(req.params.is_some());
    }

    #[test]
    fn deserialize_rejects_empty_line() {
        let err = deserialize_request("").expect_err("empty");
        assert!(err.contains("empty"));
    }

    #[test]
    fn deserialize_rejects_whitespace_only() {
        let err = deserialize_request("   ").expect_err("whitespace");
        assert!(err.contains("empty"));
    }

    #[test]
    fn deserialize_ignores_leading_trailing_whitespace() {
        let json = "  {\"id\":\"3\",\"method\":\"ping\"}\n";
        let req = deserialize_request(json).expect("deserialize");
        assert_eq!(req.id, "3");
        assert_eq!(req.method, "ping");
    }

    // ── Reply serialization ──────────────────────────────────────────────

    #[test]
    fn serialize_ok_reply_as_json_line() {
        let reply = GatewayReply::Ok(GatewayResponse {
            id: "1".into(),
            result: serde_json::json!({"ok": true}),
        });
        let line = serialize_reply(&reply).expect("serialize");
        assert!(line.ends_with('\n'));
        // Should round-trip.
        let parsed: GatewayReply = serde_json::from_str(&line).expect("parse");
        assert_eq!(parsed, reply);
    }

    #[test]
    fn serialize_error_reply_as_json_line() {
        let reply = GatewayReply::Err(GatewayError {
            id: "2".into(),
            error: GatewayErrorBody {
                code: -1,
                message: "unknown method".into(),
            },
        });
        let line = serialize_reply(&reply).expect("serialize");
        assert!(line.ends_with('\n'));
        let parsed: GatewayReply = serde_json::from_str(&line).expect("parse");
        assert_eq!(parsed, reply);
    }

    // ── PingResult ──────────────────────────────────────────────────────

    #[test]
    fn ping_result_roundtrip() {
        let ping = PingResult {
            ok: true,
            gateway_version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&ping).expect("serialize");
        let back: PingResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, ping);
    }

    // ── HealthResult ────────────────────────────────────────────────────

    #[test]
    fn health_result_roundtrip() {
        let health = HealthResult {
            ok: true,
            uptime_seconds: 42,
            active_sessions: 3,
            gateway_version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&health).expect("serialize");
        let back: HealthResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, health);
    }

    // ── GatewayRequest roundtrip ─────────────────────────────────────────

    #[test]
    fn request_roundtrip_via_json() {
        let req = GatewayRequest {
            id: "abc".into(),
            method: "list_sessions".into(),
            params: None,
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: GatewayRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, req);
    }
}
