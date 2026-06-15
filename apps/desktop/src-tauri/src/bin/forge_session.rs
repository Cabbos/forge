//! Forge Session CLI — inspect active gateway sessions and local session store.
//!
//! Usage: `forge_session <list|attach|show|stats|search|export|prune>`

use forge::gateway::client::{
    build_attach_session_request, build_get_session_snapshot_request, GatewayClient,
};
use forge::gateway::protocol::GatewayRequest;
use forge::gateway::server::{default_socket_path, SESSION_STALE_AFTER_MS};
use forge::session_store::{
    SessionSnapshotPruneReport, SessionSnapshotStoreStats, SessionSnapshotSummary,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("list");

    match cmd {
        "list" => list_gateway_sessions().await,
        "attach" => {
            let Some(session_id) = args.get(2) else {
                eprintln!("Usage: forge_session attach <session_id>");
                std::process::exit(1);
            };
            attach_gateway_session(session_id).await;
        }
        "show" => {
            let Some(session_id) = args.get(2) else {
                eprintln!("Usage: forge_session show <session_id>");
                std::process::exit(1);
            };
            show_gateway_session_snapshot(session_id).await;
        }
        "stats" => match forge::session_store::stats() {
            Ok(stats) => {
                for line in render_store_stats_lines(&stats) {
                    println!("{line}");
                }
            }
            Err(error) => exit_with_error(format!("Failed to read session store stats: {error}")),
        },
        "search" => {
            let query = args.get(2..).unwrap_or_default().join(" ");
            if query.trim().is_empty() {
                eprintln!("Usage: forge_session search <query>");
                std::process::exit(1);
            }
            match forge::session_store::search(&query) {
                Ok(snapshots) => {
                    for line in render_snapshot_lines("Session search results:", &snapshots) {
                        println!("{line}");
                    }
                }
                Err(error) => exit_with_error(format!("Failed to search session store: {error}")),
            }
        }
        "export" => match forge::session_store::export()
            .and_then(|export| serde_json::to_string_pretty(&export).map_err(|e| e.to_string()))
        {
            Ok(json) => println!("{json}"),
            Err(error) => exit_with_error(format!("Failed to export session store: {error}")),
        },
        "prune" => match parse_prune_args(&args[2..]) {
            Ok((keep_recent, older_than_ms)) => {
                match forge::session_store::prune(keep_recent, older_than_ms) {
                    Ok(report) => {
                        for line in render_prune_report_lines(&report) {
                            println!("{line}");
                        }
                    }
                    Err(error) => {
                        exit_with_error(format!("Failed to prune session store: {error}"))
                    }
                }
            }
            Err(error) => {
                eprintln!("{error}");
                eprintln!("Usage: forge_session prune --keep <count> [--older-than-ms <ms>]");
                std::process::exit(1);
            }
        },
        _ => {
            eprintln!("Usage: forge_session list|attach|show|stats|search|export|prune");
            std::process::exit(1);
        }
    }
}

async fn list_gateway_sessions() {
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

async fn attach_gateway_session(session_id: &str) {
    let socket_path = default_socket_path();
    if !socket_path.exists() {
        println!(
            "Gateway is not running (no socket at {}).",
            socket_path.display()
        );
        println!("Start it with: forge service start");
        return;
    }

    let request = match build_attach_session_request(session_id) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    match GatewayClient::connect(&socket_path).await {
        Ok(mut client) => match client.send(request).await {
            Ok(forge::gateway::protocol::GatewayReply::Ok(resp)) => {
                for line in render_attach_result_lines(&resp.result) {
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
            Err(error) => {
                eprintln!("Request failed: {error}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("Failed to connect to gateway: {error}");
            std::process::exit(1);
        }
    }
}

async fn show_gateway_session_snapshot(session_id: &str) {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        eprintln!("session_id must not be empty");
        std::process::exit(1);
    }

    let socket_path = default_socket_path();
    if !socket_path.exists() {
        show_local_session_snapshot(session_id, Some(socket_path.display().to_string()));
        return;
    }

    let request = match build_get_session_snapshot_request(session_id) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    match GatewayClient::connect(&socket_path).await {
        Ok(mut client) => match client.send(request).await {
            Ok(forge::gateway::protocol::GatewayReply::Ok(resp)) => {
                for line in render_session_snapshot_detail_lines(&resp.result) {
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
            Err(error) => {
                eprintln!("Request failed: {error}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("Failed to connect to gateway: {error}");
            std::process::exit(1);
        }
    }
}

fn show_local_session_snapshot(session_id: &str, missing_socket: Option<String>) {
    match forge::session_store::get_snapshot(session_id) {
        Ok(Some(snapshot)) => {
            for line in render_session_snapshot_detail_lines(&local_session_snapshot_result(
                session_id, snapshot,
            )) {
                println!("{line}");
            }
        }
        Ok(None) => {
            if let Some(socket_path) = missing_socket {
                println!("Gateway is not running (no socket at {socket_path}).");
            }
            println!("No local session snapshot found for {}.", session_id.trim());
            println!("Start the gateway with: forge service start");
        }
        Err(error) => exit_with_error(format!("Failed to read local session snapshot: {error}")),
    }
}

fn exit_with_error(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
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

fn render_attach_result_lines(result: &serde_json::Value) -> Vec<String> {
    let session_id = result["session_id"].as_str().unwrap_or("?");
    let status = result["status"].as_str().unwrap_or("unknown");
    let message = result["message"].as_str().unwrap_or("");
    let mut lines = vec![
        "Gateway session attach:".to_string(),
        format!("  {session_id}  {status}  {message}"),
    ];

    append_attach_control_lines(result, &mut lines);
    append_attach_snapshot_lines(result, &mut lines);

    if let Some(session) = result.get("session").filter(|session| session.is_object()) {
        let provider = session["provider"].as_str().unwrap_or("?");
        let model = session["model"].as_str().unwrap_or("?");
        let workspace = session["workspace_path"].as_str().unwrap_or("?");
        lines.push(format!("  {provider}/{model}  {workspace}"));
    }

    lines
}

fn append_attach_control_lines(result: &serde_json::Value, lines: &mut Vec<String>) {
    let Some(control) = result.get("control") else {
        return;
    };
    let control_plane = control["control_plane"].as_str().unwrap_or("unknown");
    let can_stream = control["gateway_can_stream"].as_bool().unwrap_or(false);
    let can_send_input = control["gateway_can_send_input"].as_bool().unwrap_or(false);
    let can_resume = control["gateway_can_resume"].as_bool().unwrap_or(false);
    let can_read_snapshot = control["gateway_can_read_snapshot"]
        .as_bool()
        .unwrap_or(false);
    lines.push(format!(
        "  control  {control_plane}  stream={can_stream} input={can_send_input} resume={can_resume} snapshot={can_read_snapshot}"
    ));
    if let Some(required_action) = control["required_action"].as_str() {
        if !required_action.trim().is_empty() {
            lines.push(format!("  action  {}", required_action.trim()));
        }
    }
}

fn append_attach_snapshot_lines(result: &serde_json::Value, lines: &mut Vec<String>) {
    let Some(snapshot) = result
        .get("snapshot")
        .filter(|snapshot| snapshot.is_object())
    else {
        return;
    };
    let provider = snapshot["provider"].as_str().unwrap_or("?");
    let model = snapshot["model"].as_str().unwrap_or("?");
    let working_dir = snapshot["working_dir"].as_str().unwrap_or("?");
    let updated_at_ms = snapshot["updated_at_ms"].as_u64().unwrap_or(0);
    let message_count = snapshot["message_count"].as_u64().unwrap_or(0);
    lines.push(format!(
        "  snapshot  updated={updated_at_ms} messages={message_count}  {provider}/{model}  {working_dir}"
    ));
    if let Some(summary) = snapshot["summary"].as_str() {
        if !summary.trim().is_empty() {
            lines.push(format!("  summary  {}", summary.trim()));
        }
    }
}

fn render_session_snapshot_detail_lines(result: &serde_json::Value) -> Vec<String> {
    let session_id = result["session_id"].as_str().unwrap_or("?");
    let snapshot = result.get("snapshot").unwrap_or(&serde_json::Value::Null);
    let provider = snapshot["provider"].as_str().unwrap_or("?");
    let model = snapshot["model"].as_str().unwrap_or("?");
    let working_dir = snapshot["working_dir"].as_str().unwrap_or("?");
    let created_at_ms = snapshot["created_at_ms"].as_u64().unwrap_or(0);
    let updated_at_ms = snapshot["updated_at_ms"].as_u64().unwrap_or(0);
    let message_count = snapshot
        .get("messages")
        .and_then(|messages| messages.as_array())
        .map(Vec::len)
        .unwrap_or(0);

    let mut lines = vec![
        "Gateway session snapshot:".to_string(),
        format!("  {session_id}  {provider}/{model}  {working_dir}"),
        format!("  created={created_at_ms} updated={updated_at_ms} messages={message_count}"),
    ];
    if let Some(summary) = snapshot["summary"].as_str() {
        if !summary.trim().is_empty() {
            lines.push(format!("  summary  {}", summary.trim()));
        }
    }
    lines
}

fn local_session_snapshot_result(
    session_id: &str,
    snapshot: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "session_id": session_id.trim(),
        "snapshot": snapshot,
    })
}

fn render_snapshot_lines(title: &str, snapshots: &[SessionSnapshotSummary]) -> Vec<String> {
    if snapshots.is_empty() {
        return vec!["No session snapshots.".to_string()];
    }

    let mut lines = Vec::with_capacity(snapshots.len() + 1);
    lines.push(title.to_string());
    for snapshot in snapshots {
        lines.push(format!(
            "  {}  {}  {}/{}  {}",
            snapshot.session_id,
            snapshot.updated_at_ms,
            snapshot.provider,
            snapshot.model,
            snapshot.working_dir
        ));
    }
    lines
}

fn render_store_stats_lines(stats: &SessionSnapshotStoreStats) -> Vec<String> {
    let mut lines = vec![
        "Session store stats:".to_string(),
        format!("  snapshots: {}", stats.total_snapshots),
        format!("  corrupted: {}", stats.corrupted_snapshots),
        format!("  bytes: {}", stats.total_bytes),
    ];
    if let Some(oldest) = stats.oldest_updated_at_ms {
        lines.push(format!("  oldest_updated_at_ms: {oldest}"));
    }
    if let Some(newest) = stats.newest_updated_at_ms {
        lines.push(format!("  newest_updated_at_ms: {newest}"));
    }
    if !stats.by_provider.is_empty() {
        lines.push("  by_provider:".to_string());
        for (provider, count) in &stats.by_provider {
            lines.push(format!("    {provider}: {count}"));
        }
    }
    if !stats.by_workspace.is_empty() {
        lines.push("  by_workspace:".to_string());
        for (workspace, count) in &stats.by_workspace {
            lines.push(format!("    {workspace}: {count}"));
        }
    }
    lines
}

fn render_prune_report_lines(report: &SessionSnapshotPruneReport) -> Vec<String> {
    let mut lines = vec![
        "Session prune report:".to_string(),
        format!("  deleted: {}", report.deleted_session_ids.len()),
        format!("  kept: {}", report.kept_session_ids.len()),
        format!("  skipped_corrupted: {}", report.skipped_corrupted),
    ];
    if !report.deleted_session_ids.is_empty() {
        lines.push(format!(
            "  deleted_session_ids: {}",
            report.deleted_session_ids.join(", ")
        ));
    }
    if !report.kept_session_ids.is_empty() {
        lines.push(format!(
            "  kept_session_ids: {}",
            report.kept_session_ids.join(", ")
        ));
    }
    lines
}

fn parse_prune_args(args: &[String]) -> Result<(usize, Option<u64>), String> {
    let mut keep_recent = None;
    let mut older_than_ms = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--keep" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--keep requires a count".to_string())?;
                keep_recent = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "--keep must be a non-negative integer".to_string())?,
                );
                index += 2;
            }
            "--older-than-ms" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--older-than-ms requires milliseconds".to_string())?;
                older_than_ms =
                    Some(value.parse::<u64>().map_err(|_| {
                        "--older-than-ms must be a non-negative integer".to_string()
                    })?);
                index += 2;
            }
            unknown => return Err(format!("Unknown prune option: {unknown}")),
        }
    }

    Ok((
        keep_recent.ok_or_else(|| "--keep is required".to_string())?,
        older_than_ms,
    ))
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
    use forge::session_store::{SessionSnapshotPruneReport, SessionSnapshotStoreStats};
    use std::collections::BTreeMap;

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

    #[test]
    fn render_store_stats_lines_shows_counts_and_facets() {
        let stats = SessionSnapshotStoreStats {
            total_snapshots: 2,
            corrupted_snapshots: 1,
            total_bytes: 4096,
            oldest_updated_at_ms: Some(10),
            newest_updated_at_ms: Some(20),
            by_provider: BTreeMap::from([("openai".to_string(), 2)]),
            by_workspace: BTreeMap::from([("/repo".to_string(), 2)]),
        };

        let lines = super::render_store_stats_lines(&stats);

        assert_eq!(lines[0], "Session store stats:");
        assert!(lines.iter().any(|line| line.contains("snapshots: 2")));
        assert!(lines.iter().any(|line| line.contains("corrupted: 1")));
        assert!(lines.iter().any(|line| line.contains("openai: 2")));
    }

    #[test]
    fn render_prune_report_lines_names_deleted_and_kept_sessions() {
        let report = SessionSnapshotPruneReport {
            deleted_session_ids: vec!["old-session".to_string()],
            kept_session_ids: vec!["new-session".to_string()],
            skipped_corrupted: 1,
        };

        let lines = super::render_prune_report_lines(&report);

        assert_eq!(lines[0], "Session prune report:");
        assert!(lines.iter().any(|line| line.contains("deleted: 1")));
        assert!(lines.iter().any(|line| line.contains("old-session")));
        assert!(lines.iter().any(|line| line.contains("kept: 1")));
    }

    #[test]
    fn parse_prune_args_requires_keep_and_accepts_optional_age() {
        let args = vec![
            "--keep".to_string(),
            "25".to_string(),
            "--older-than-ms".to_string(),
            "1000".to_string(),
        ];

        assert_eq!(super::parse_prune_args(&args).unwrap(), (25, Some(1000)));
        assert!(super::parse_prune_args(&[]).is_err());
    }

    #[test]
    fn render_attach_result_lines_shows_status_and_session() {
        let result = serde_json::json!({
            "ok": true,
            "session_id": "session-live",
            "status": "live",
            "message": "Session is live and attachable.",
            "control": {
                "control_plane": "desktop_runtime_required",
                "gateway_can_stream": false,
                "gateway_can_send_input": false,
                "gateway_can_resume": false,
                "gateway_can_read_snapshot": true,
                "required_action": "Open the owning desktop runtime to continue this session."
            },
            "snapshot": {
                "session_id": "session-live",
                "provider": "claude",
                "model": "opus",
                "working_dir": "/repo/live",
                "summary": "latest summary",
                "created_at_ms": 1,
                "updated_at_ms": 2,
                "message_count": 3
            },
            "session": {
                "session_id": "session-live",
                "provider": "claude",
                "model": "opus",
                "workspace_path": "/repo/live"
            }
        });

        let lines = super::render_attach_result_lines(&result);

        assert_eq!(lines[0], "Gateway session attach:");
        assert_eq!(
            lines[1],
            "  session-live  live  Session is live and attachable."
        );
        assert_eq!(
            lines[2],
            "  control  desktop_runtime_required  stream=false input=false resume=false snapshot=true"
        );
        assert_eq!(
            lines[3],
            "  action  Open the owning desktop runtime to continue this session."
        );
        assert_eq!(
            lines[4],
            "  snapshot  updated=2 messages=3  claude/opus  /repo/live"
        );
        assert_eq!(lines[5], "  summary  latest summary");
        assert_eq!(lines[6], "  claude/opus  /repo/live");
    }

    #[test]
    fn render_session_snapshot_detail_lines_shows_full_snapshot_metadata() {
        let result = serde_json::json!({
            "ok": true,
            "session_id": "session-1",
            "snapshot": {
                "session_id": "session-1",
                "provider": "deepseek",
                "model": "deepseek-v4-flash",
                "working_dir": "/repo/detail",
                "summary": "detail summary",
                "created_at_ms": 10,
                "updated_at_ms": 20,
                "messages": [
                    {"role": "user", "content": "show me"},
                    {"role": "assistant", "content": "done"}
                ]
            }
        });

        let lines = super::render_session_snapshot_detail_lines(&result);

        assert_eq!(lines[0], "Gateway session snapshot:");
        assert_eq!(
            lines[1],
            "  session-1  deepseek/deepseek-v4-flash  /repo/detail"
        );
        assert_eq!(lines[2], "  created=10 updated=20 messages=2");
        assert_eq!(lines[3], "  summary  detail summary");
    }

    #[test]
    fn local_session_snapshot_result_wraps_snapshot_for_detail_renderer() {
        let snapshot = serde_json::json!({
            "session_id": "session-1",
            "provider": "deepseek",
            "model": "deepseek-v4-flash",
            "working_dir": "/repo/detail",
            "messages": []
        });

        let result = super::local_session_snapshot_result(" session-1 ", snapshot.clone());

        assert_eq!(result["ok"], true);
        assert_eq!(result["session_id"], "session-1");
        assert_eq!(result["snapshot"], snapshot);
    }
}
