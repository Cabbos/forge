//! Forge Session CLI — inspect active gateway sessions and local session store.
//!
//! Usage: `forge_session <list|stats|search|export|prune>`

use forge::gateway::client::GatewayClient;
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
            eprintln!("Usage: forge_session list|stats|search|export|prune");
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
}
