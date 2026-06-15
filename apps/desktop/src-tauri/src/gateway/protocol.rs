//! Gateway IPC protocol — JSON-line message types.
//!
//! The gateway listens on a Unix domain socket and accepts one JSON object
//! per line.  Every request carries a unique `id`; the response echoes that
//! `id` so clients can correlate.

use serde::{Deserialize, Serialize};

use crate::gateway::runner::TriggerRunRecord;
use crate::gateway::session_input::SessionInputRecord;

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

/// Lightweight session record tracked by the gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewaySessionInfo {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub workspace_path: String,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at_ms: Option<u64>,
    #[serde(default)]
    pub restored_from_registry: bool,
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

/// Parameters for attaching to a session known by the gateway registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachSessionParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewaySessionAttachStatus {
    Live,
    Restored,
    Stale,
    Missing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewaySessionControlPlane {
    DesktopRuntimeRequired,
    DesktopRestoreRequired,
    Unavailable,
}

/// Gateway-side control capabilities for a session attach attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewaySessionControl {
    pub control_plane: GatewaySessionControlPlane,
    pub gateway_can_stream: bool,
    pub gateway_can_send_input: bool,
    pub gateway_can_resume: bool,
    pub gateway_can_read_snapshot: bool,
    pub required_action: String,
}

/// Lightweight snapshot summary available to gateway attach callers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewaySessionSnapshotSummary {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub working_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
}

/// Result returned by `attach_session`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachSessionResult {
    pub ok: bool,
    pub session_id: String,
    pub status: GatewaySessionAttachStatus,
    pub message: String,
    pub control: GatewaySessionControl,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<GatewaySessionSnapshotSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<GatewaySessionInfo>,
}

/// Parameters for queueing a gateway trigger through the Unix socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnqueueTriggerParams {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
}

/// Result returned after a trigger is accepted into the gateway queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnqueueTriggerResult {
    pub ok: bool,
    pub trigger_id: String,
    pub pending_triggers: usize,
}

/// Parameters for queueing input addressed to an existing gateway session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnqueueSessionInputParams {
    pub session_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_id: Option<String>,
}

/// Result returned after input is accepted into the gateway session inbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnqueueSessionInputResult {
    pub ok: bool,
    pub input_id: String,
    pub session_id: String,
    pub pending_inputs: usize,
}

/// Parameters for listing queued input addressed to live sessions owned by a runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSessionInputsParams {
    pub session_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Result returned after listing queued session input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSessionInputsResult {
    pub ok: bool,
    pub inputs: Vec<SessionInputRecord>,
    pub pending_inputs: usize,
}

/// Parameters for marking a queued session input as accepted by the owner runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompleteSessionInputParams {
    pub input_id: String,
}

/// Result returned after marking a queued session input complete.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompleteSessionInputResult {
    pub ok: bool,
    pub input_id: String,
    pub removed: bool,
    pub pending_inputs: usize,
}

/// Parameters for removing a queued gateway trigger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelTriggerParams {
    pub trigger_id: String,
}

/// Result returned after a cancel request is processed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelTriggerResult {
    pub ok: bool,
    pub trigger_id: String,
    pub removed: bool,
    pub pending_triggers: usize,
}

/// Parameters for replaying a previous trigger run into the pending queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayTriggerRunParams {
    pub run_id: String,
}

/// Result returned after a previous trigger run is re-queued.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayTriggerRunResult {
    pub ok: bool,
    pub run_id: String,
    pub trigger_id: String,
    pub pending_triggers: usize,
}

/// Parameters for reading a previous trigger run by id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetTriggerRunParams {
    pub run_id: String,
}

/// Result returned after reading a previous trigger run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetTriggerRunResult {
    pub ok: bool,
    pub run: TriggerRunRecord,
}

/// Parameters for reading a saved session snapshot by id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetSessionSnapshotParams {
    pub session_id: String,
}

/// Result returned after reading a saved session snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetSessionSnapshotResult {
    pub ok: bool,
    pub session_id: String,
    pub snapshot: serde_json::Value,
}

/// Parameters for tailing persisted session transcript events through the gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TailSessionEventsParams {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_cursor: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Result returned by `tail_session_events`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TailSessionEventsResult {
    pub ok: bool,
    pub session_id: String,
    pub events: Vec<serde_json::Value>,
    pub next_cursor: usize,
    pub total_events: usize,
    #[serde(default)]
    pub cursor_reset: bool,
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

    #[test]
    fn get_trigger_run_params_and_result_roundtrip() {
        let params = GetTriggerRunParams {
            run_id: " run-1 ".into(),
        };
        let json = serde_json::to_string(&params).expect("serialize params");
        let back: GetTriggerRunParams = serde_json::from_str(&json).expect("deserialize params");
        assert_eq!(back, params);

        let result = GetTriggerRunResult {
            ok: true,
            run: crate::gateway::runner::TriggerRunRecord {
                id: "run-1".into(),
                trigger_id: "trigger-1".into(),
                session_id: Some("session-1".into()),
                attempt: 2,
                status: "dead_letter".into(),
                message: "provider offline".into(),
                started_at_ms: 10,
                ended_at_ms: 20,
                trigger_message: Some("run digest".into()),
                profile_id: Some("ops".into()),
                provider: Some("openai".into()),
                model: Some("gpt-5".into()),
                workspace_path: Some("/repo".into()),
            },
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        assert!(json.contains("\"session_id\":\"session-1\""));
        let back: GetTriggerRunResult = serde_json::from_str(&json).expect("deserialize result");
        assert_eq!(back, result);
    }

    #[test]
    fn get_session_snapshot_params_and_result_roundtrip() {
        let params = GetSessionSnapshotParams {
            session_id: " session-1 ".into(),
        };
        let json = serde_json::to_string(&params).expect("serialize params");
        let back: GetSessionSnapshotParams =
            serde_json::from_str(&json).expect("deserialize params");
        assert_eq!(back, params);

        let result = GetSessionSnapshotResult {
            ok: true,
            session_id: "session-1".into(),
            snapshot: serde_json::json!({
                "session_id": "session-1",
                "provider": "deepseek",
                "model": "deepseek-v4-flash",
                "messages": [{"role": "user", "content": "hello"}]
            }),
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        assert!(json.contains("\"session_id\":\"session-1\""));
        assert!(json.contains("\"messages\""));
        let back: GetSessionSnapshotResult =
            serde_json::from_str(&json).expect("deserialize result");
        assert_eq!(back, result);
    }

    #[test]
    fn enqueue_session_input_params_and_result_roundtrip() {
        let params = EnqueueSessionInputParams {
            session_id: " session-1 ".into(),
            message: " continue ".into(),
            input_id: Some("input-1".into()),
        };
        let json = serde_json::to_string(&params).expect("serialize params");
        let back: EnqueueSessionInputParams =
            serde_json::from_str(&json).expect("deserialize params");
        assert_eq!(back, params);

        let result = EnqueueSessionInputResult {
            ok: true,
            input_id: "input-1".into(),
            session_id: "session-1".into(),
            pending_inputs: 2,
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        assert!(json.contains("\"pending_inputs\":2"));
        let back: EnqueueSessionInputResult =
            serde_json::from_str(&json).expect("deserialize result");
        assert_eq!(back, result);
    }

    #[test]
    fn list_and_complete_session_input_roundtrip() {
        let params = ListSessionInputsParams {
            session_ids: vec!["session-1".into(), " session-2 ".into()],
            limit: Some(3),
        };
        let json = serde_json::to_string(&params).expect("serialize list params");
        let back: ListSessionInputsParams =
            serde_json::from_str(&json).expect("deserialize list params");
        assert_eq!(back, params);

        let result = ListSessionInputsResult {
            ok: true,
            inputs: vec![SessionInputRecord {
                id: "input-1".into(),
                session_id: "session-1".into(),
                message: "continue".into(),
                received_at_ms: 123,
            }],
            pending_inputs: 2,
        };
        let json = serde_json::to_string(&result).expect("serialize list result");
        assert!(json.contains("\"input-1\""));
        let back: ListSessionInputsResult =
            serde_json::from_str(&json).expect("deserialize list result");
        assert_eq!(back, result);

        let params = CompleteSessionInputParams {
            input_id: " input-1 ".into(),
        };
        let json = serde_json::to_string(&params).expect("serialize complete params");
        let back: CompleteSessionInputParams =
            serde_json::from_str(&json).expect("deserialize complete params");
        assert_eq!(back, params);

        let result = CompleteSessionInputResult {
            ok: true,
            input_id: "input-1".into(),
            removed: true,
            pending_inputs: 1,
        };
        let json = serde_json::to_string(&result).expect("serialize complete result");
        assert!(json.contains("\"removed\":true"));
        let back: CompleteSessionInputResult =
            serde_json::from_str(&json).expect("deserialize complete result");
        assert_eq!(back, result);
    }

    #[test]
    fn tail_session_events_params_and_result_roundtrip() {
        let params = TailSessionEventsParams {
            session_id: " session-1 ".into(),
            after_cursor: Some(2),
            limit: Some(10),
        };
        let json = serde_json::to_string(&params).expect("serialize params");
        let back: TailSessionEventsParams =
            serde_json::from_str(&json).expect("deserialize params");
        assert_eq!(back, params);

        let result = TailSessionEventsResult {
            ok: true,
            session_id: "session-1".into(),
            events: vec![serde_json::json!({
                "event_type": "user_message",
                "session_id": "session-1",
                "content": "hello"
            })],
            next_cursor: 3,
            total_events: 3,
            cursor_reset: false,
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        assert!(json.contains("\"next_cursor\":3"));
        let back: TailSessionEventsResult =
            serde_json::from_str(&json).expect("deserialize result");
        assert_eq!(back, result);
    }

    #[test]
    fn attach_session_params_and_result_roundtrip() {
        let params = AttachSessionParams {
            session_id: " session-1 ".into(),
        };
        let json = serde_json::to_string(&params).expect("serialize params");
        let back: AttachSessionParams = serde_json::from_str(&json).expect("deserialize params");
        assert_eq!(back, params);

        let result = AttachSessionResult {
            ok: true,
            session_id: "session-1".into(),
            status: GatewaySessionAttachStatus::Live,
            message: "Session is live and attachable.".into(),
            control: GatewaySessionControl {
                control_plane: GatewaySessionControlPlane::DesktopRuntimeRequired,
                gateway_can_stream: false,
                gateway_can_send_input: false,
                gateway_can_resume: false,
                gateway_can_read_snapshot: true,
                required_action: "Open the owning desktop runtime to continue this session.".into(),
            },
            snapshot: Some(GatewaySessionSnapshotSummary {
                session_id: "session-1".into(),
                provider: "openai".into(),
                model: "gpt-5".into(),
                working_dir: "/repo".into(),
                summary: Some("latest summary".into()),
                created_at_ms: 1,
                updated_at_ms: 2,
                message_count: 3,
            }),
            session: Some(GatewaySessionInfo {
                session_id: "session-1".into(),
                provider: "openai".into(),
                model: "gpt-5".into(),
                workspace_path: "/repo".into(),
                created_at_ms: 10,
                owner_pid: Some(42),
                last_seen_at_ms: Some(20),
                restored_from_registry: false,
            }),
        };
        let json = serde_json::to_string(&result).expect("serialize result");
        assert!(json.contains("\"status\":\"live\""));
        assert!(json.contains("\"control_plane\":\"desktop_runtime_required\""));
        assert!(json.contains("\"gateway_can_read_snapshot\":true"));
        assert!(json.contains("\"message_count\":3"));
        let back: AttachSessionResult = serde_json::from_str(&json).expect("deserialize result");
        assert_eq!(back, result);
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

    #[test]
    fn enqueue_trigger_params_roundtrip_with_metadata() {
        let params = EnqueueTriggerParams {
            message: "run daily digest".into(),
            trigger_id: Some("trigger-cli-1".into()),
            profile_id: Some("ops".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5".into()),
            workspace_path: Some("/tmp/forge-workspace".into()),
        };

        let json = serde_json::to_string(&params).expect("serialize");
        let restored: EnqueueTriggerParams = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.message, "run daily digest");
        assert_eq!(restored.trigger_id.as_deref(), Some("trigger-cli-1"));
        assert_eq!(restored.profile_id.as_deref(), Some("ops"));
        assert_eq!(restored.provider.as_deref(), Some("openai"));
        assert_eq!(restored.model.as_deref(), Some("gpt-5"));
        assert_eq!(
            restored.workspace_path.as_deref(),
            Some("/tmp/forge-workspace")
        );
    }

    #[test]
    fn cancel_trigger_result_roundtrip() {
        let result = CancelTriggerResult {
            ok: true,
            trigger_id: "trigger-1".into(),
            removed: true,
            pending_triggers: 0,
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let restored: CancelTriggerResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored, result);
    }

    #[test]
    fn replay_trigger_run_params_and_result_roundtrip() {
        let params = ReplayTriggerRunParams {
            run_id: "run-1".into(),
        };
        let params_json = serde_json::to_string(&params).expect("serialize params");
        let restored_params: ReplayTriggerRunParams =
            serde_json::from_str(&params_json).expect("deserialize params");
        assert_eq!(restored_params, params);

        let result = ReplayTriggerRunResult {
            ok: true,
            run_id: "run-1".into(),
            trigger_id: "trigger-replay".into(),
            pending_triggers: 2,
        };
        let result_json = serde_json::to_string(&result).expect("serialize result");
        let restored_result: ReplayTriggerRunResult =
            serde_json::from_str(&result_json).expect("deserialize result");
        assert_eq!(restored_result, result);
    }
}
