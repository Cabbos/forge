//! Session attach/registry handlers, session snapshots, and session event tails.

use super::*;

pub(super) fn handle_attach_session(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<AttachSessionParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(state.attach_session(&session_id)).unwrap(),
    })
}

pub(super) fn handle_list_sessions(state: &GatewayState, id: String) -> GatewayReply {
    let sessions = state.list_sessions();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(sessions).unwrap(),
    })
}

pub(super) fn handle_register_session(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    match request.params {
        Some(params) => match serde_json::from_value::<GatewaySessionInfo>(params) {
            Ok(info) => {
                state.register_session(info);
                GatewayReply::Ok(GatewayResponse {
                    id: request.id,
                    result: serde_json::json!({"ok": true}),
                })
            }
            Err(e) => GatewayReply::Err(GatewayError {
                id: request.id,
                error: GatewayErrorBody {
                    code: -32602,
                    message: format!("invalid params: {e}"),
                },
            }),
        },
        None => GatewayReply::Err(GatewayError {
            id: request.id,
            error: GatewayErrorBody {
                code: -32602,
                message: "missing params".to_string(),
            },
        }),
    }
}

pub(super) fn handle_get_session_snapshot(request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<GetSessionSnapshotParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }

    let snapshot = match crate::agent::snapshot::load_session_snapshot(&session_id) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return invalid_params(
                request.id,
                format!("session snapshot not available: {error}"),
            );
        }
    };
    let snapshot = match serde_json::to_value(snapshot) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return invalid_params(request.id, format!("serialize snapshot: {error}"));
        }
    };

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(GetSessionSnapshotResult {
            ok: true,
            session_id,
            snapshot,
        })
        .unwrap(),
    })
}

pub(super) fn handle_tail_session_events(request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<TailSessionEventsParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }

    let tail = match crate::transcript::tail_transcript_events(
        &session_id,
        params.after_cursor,
        params.limit.unwrap_or(100),
    ) {
        Ok(tail) => tail,
        Err(error) => {
            return invalid_params(request.id, format!("session events unavailable: {error}"));
        }
    };

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(TailSessionEventsResult {
            ok: true,
            session_id: tail.session_id,
            events: tail.events,
            next_cursor: tail.next_cursor,
            total_events: tail.total_events,
            cursor_reset: tail.cursor_reset,
        })
        .unwrap(),
    })
}

pub(super) fn gateway_session_attach_status_label(
    status: GatewaySessionAttachStatus,
) -> &'static str {
    match status {
        GatewaySessionAttachStatus::Live => "live",
        GatewaySessionAttachStatus::Restored => "restored",
        GatewaySessionAttachStatus::Stale => "stale",
        GatewaySessionAttachStatus::Missing => "missing",
    }
}

pub(super) fn handle_unregister_session(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    match request.params {
        Some(params) => {
            if let Some(session_id) = params.get("session_id").and_then(|v| v.as_str()) {
                state.unregister_session(session_id);
                GatewayReply::Ok(GatewayResponse {
                    id: request.id,
                    result: serde_json::json!({"ok": true}),
                })
            } else {
                GatewayReply::Err(GatewayError {
                    id: request.id,
                    error: GatewayErrorBody {
                        code: -32602,
                        message: "missing session_id".to_string(),
                    },
                })
            }
        }
        None => GatewayReply::Err(GatewayError {
            id: request.id,
            error: GatewayErrorBody {
                code: -32602,
                message: "missing params".to_string(),
            },
        }),
    }
}

pub(super) fn default_session_registry_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".forge")
        .join("gateway-sessions.json")
}

pub(super) fn load_session_registry(path: &Path) -> HashMap<String, GatewaySessionInfo> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
        Err(error) => {
            log::warn!("failed to read gateway session registry: {error}");
            return HashMap::new();
        }
    };
    match serde_json::from_str::<Vec<GatewaySessionInfo>>(&raw) {
        Ok(sessions) => sessions
            .into_iter()
            .filter(|session| !session.session_id.trim().is_empty())
            .map(|mut session| {
                session.restored_from_registry = true;
                (session.session_id.clone(), session)
            })
            .collect(),
        Err(error) => {
            log::warn!("failed to parse gateway session registry: {error}");
            HashMap::new()
        }
    }
}

pub(super) fn save_session_registry(
    path: &Path,
    sessions: &HashMap<String, GatewaySessionInfo>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("create session dir: {error}"))?;
    }
    let sessions = sorted_sessions(sessions.values().cloned());
    let json = serde_json::to_string_pretty(&sessions)
        .map_err(|error| format!("serialize sessions: {error}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json.as_bytes())
        .map_err(|error| format!("write session registry tmp: {error}"))?;
    std::fs::rename(&tmp, path).map_err(|error| format!("replace session registry: {error}"))?;
    Ok(())
}

pub(super) fn sorted_sessions(
    sessions: impl IntoIterator<Item = GatewaySessionInfo>,
) -> Vec<GatewaySessionInfo> {
    let mut sessions = sessions.into_iter().collect::<Vec<_>>();
    sessions.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    sessions
}

pub(super) fn snapshot_backed_gateway_sessions() -> Vec<GatewaySessionInfo> {
    match crate::agent::snapshot::list_session_snapshots() {
        Ok(snapshots) => snapshots
            .into_iter()
            .map(|snapshot| GatewaySessionInfo {
                session_id: snapshot.session_id,
                provider: snapshot.provider,
                model: snapshot.model,
                workspace_path: snapshot.working_dir,
                created_at_ms: snapshot.created_at_ms,
                owner_pid: None,
                last_seen_at_ms: None,
                restored_from_registry: true,
            })
            .collect(),
        Err(error) => {
            log::warn!("failed to list gateway snapshot-backed sessions: {error}");
            Vec::new()
        }
    }
}

pub(super) fn active_session_count(sessions: &HashMap<String, GatewaySessionInfo>) -> usize {
    active_session_count_at(sessions, now_millis())
}

pub(super) fn active_session_count_at(
    sessions: &HashMap<String, GatewaySessionInfo>,
    now_ms: u64,
) -> usize {
    sessions
        .values()
        .filter(|session| session_counts_as_active_at(session, now_ms))
        .count()
}

pub(super) fn session_counts_as_active_at(session: &GatewaySessionInfo, now_ms: u64) -> bool {
    if session.restored_from_registry {
        return false;
    }

    let Some(last_seen_at_ms) = session.last_seen_at_ms else {
        return true;
    };

    now_ms.saturating_sub(last_seen_at_ms) <= SESSION_STALE_AFTER_MS
}

pub(super) fn session_attach_status_at(
    session: &GatewaySessionInfo,
    now_ms: u64,
) -> GatewaySessionAttachStatus {
    if session.restored_from_registry {
        return GatewaySessionAttachStatus::Restored;
    }

    if let Some(last_seen_at_ms) = session.last_seen_at_ms {
        if now_ms.saturating_sub(last_seen_at_ms) > SESSION_STALE_AFTER_MS {
            return GatewaySessionAttachStatus::Stale;
        }
    }

    GatewaySessionAttachStatus::Live
}

pub(super) fn session_attach_message(status: GatewaySessionAttachStatus) -> &'static str {
    match status {
        GatewaySessionAttachStatus::Live => "Session is live and attachable.",
        GatewaySessionAttachStatus::Restored => {
            "Session metadata was restored from the gateway registry; reopen the owning runtime before attaching."
        }
        GatewaySessionAttachStatus::Stale => {
            "Session heartbeat is stale; the owning runtime may have exited unexpectedly."
        }
        GatewaySessionAttachStatus::Missing => "Session is not registered with the gateway.",
    }
}

pub(super) fn session_attach_control(
    status: GatewaySessionAttachStatus,
    gateway_can_read_snapshot: bool,
) -> GatewaySessionControl {
    match status {
        GatewaySessionAttachStatus::Live => GatewaySessionControl {
            control_plane: GatewaySessionControlPlane::DesktopRuntimeRequired,
            ownership_mode: GatewayOwnershipMode::LocalDefault,
            gateway_can_own_session: false,
            gateway_can_stream: true,
            gateway_can_send_input: true,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action:
                "Queue input through the gateway; the owning desktop runtime will consume it."
                    .to_string(),
        },
        GatewaySessionAttachStatus::Restored => GatewaySessionControl {
            control_plane: GatewaySessionControlPlane::DesktopRestoreRequired,
            ownership_mode: GatewayOwnershipMode::LocalDefault,
            gateway_can_own_session: false,
            gateway_can_stream: gateway_can_read_snapshot,
            gateway_can_send_input: false,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action:
                "Restore the session in desktop first; gateway only has registry metadata."
                    .to_string(),
        },
        GatewaySessionAttachStatus::Stale => GatewaySessionControl {
            control_plane: GatewaySessionControlPlane::DesktopRestoreRequired,
            ownership_mode: GatewayOwnershipMode::LocalDefault,
            gateway_can_own_session: false,
            gateway_can_stream: gateway_can_read_snapshot,
            gateway_can_send_input: false,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action: "Reopen desktop to recover or clear the stale owner before attaching."
                .to_string(),
        },
        GatewaySessionAttachStatus::Missing => GatewaySessionControl {
            control_plane: if gateway_can_read_snapshot {
                GatewaySessionControlPlane::DesktopRestoreRequired
            } else {
                GatewaySessionControlPlane::Unavailable
            },
            ownership_mode: GatewayOwnershipMode::LocalDefault,
            gateway_can_own_session: false,
            gateway_can_stream: gateway_can_read_snapshot,
            gateway_can_send_input: false,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action: if gateway_can_read_snapshot {
                "Restore the session from snapshot before attaching.".to_string()
            } else {
                "Register or restore the session before attaching.".to_string()
            },
        },
    }
}

pub(super) fn gateway_snapshot_summary_for_session(
    session_id: &str,
) -> Option<GatewaySessionSnapshotSummary> {
    crate::session_store::get_summary(session_id)
        .ok()
        .flatten()
        .map(|summary| GatewaySessionSnapshotSummary {
            session_id: summary.session_id,
            provider: summary.provider,
            model: summary.model,
            working_dir: summary.working_dir,
            summary: summary.summary,
            created_at_ms: summary.created_at_ms,
            updated_at_ms: summary.updated_at_ms,
            message_count: summary.message_count,
        })
}
