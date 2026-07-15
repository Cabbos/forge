//! Forge Session CLI — inspect active gateway sessions and local session store.
//!
//! Usage: `forge_session <list|attach|show|events|input|stats|search|rename|export|export-eval|prune>`

use forge::gateway::client::{
    build_attach_session_request, build_enqueue_session_input_request,
    build_get_session_snapshot_request, build_tail_session_events_request, GatewayClient,
};
use forge::gateway::protocol::GatewayRequest;
use forge::gateway::server::{default_socket_path, SESSION_STALE_AFTER_MS};
use forge::session_store::{
    SessionSnapshotPruneReport, SessionSnapshotStoreStats, SessionSnapshotSummary,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
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
        "events" => {
            let Some(session_id) = args.get(2) else {
                eprintln!(
                    "Usage: forge_session events <session_id> [--after <cursor>] [--limit <count>]"
                );
                std::process::exit(1);
            };
            match parse_events_args(&args[3..]) {
                Ok((after_cursor, limit)) => {
                    tail_gateway_session_events(session_id, after_cursor, limit).await;
                }
                Err(error) => {
                    eprintln!("{error}");
                    eprintln!(
                        "Usage: forge_session events <session_id> [--after <cursor>] [--limit <count>]"
                    );
                    std::process::exit(1);
                }
            }
        }
        "input" => {
            let Some(session_id) = args.get(2) else {
                eprintln!("Usage: forge_session input <session_id> <message>");
                std::process::exit(1);
            };
            let message = args.get(3..).unwrap_or_default().join(" ");
            if message.trim().is_empty() {
                eprintln!("Usage: forge_session input <session_id> <message>");
                std::process::exit(1);
            }
            enqueue_gateway_session_input(session_id, &message).await;
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
        "rename" => {
            let Some(session_id) = args.get(2) else {
                eprintln!("Usage: forge_session rename <session_id> <summary>");
                std::process::exit(1);
            };
            let summary = args.get(3..).unwrap_or_default().join(" ");
            if summary.trim().is_empty() {
                eprintln!("Usage: forge_session rename <session_id> <summary>");
                std::process::exit(1);
            }
            match forge::session_store::rename(session_id, &summary) {
                Ok(Some(snapshot)) => {
                    for line in render_rename_result_lines(&snapshot) {
                        println!("{line}");
                    }
                }
                Ok(None) => exit_with_error("Session id must not be empty".to_string()),
                Err(error) => {
                    exit_with_error(format!("Failed to rename session snapshot: {error}"))
                }
            }
        }
        "export" => match forge::session_store::export()
            .and_then(|export| serde_json::to_string_pretty(&export).map_err(|e| e.to_string()))
        {
            Ok(json) => println!("{json}"),
            Err(error) => exit_with_error(format!("Failed to export session store: {error}")),
        },
        "export-eval" => match parse_export_eval_args(&args[2..]) {
            Ok(args) => export_session_eval_trace_command(args),
            Err(error) => {
                eprintln!("{error}");
                eprintln!(
                    "Usage: forge_session export-eval <session_id> [--output <path>] [--format agent-trace|forge-payload]"
                );
                std::process::exit(1);
            }
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
            eprintln!(
                "Usage: forge_session list|attach|show|events|input|stats|search|rename|export|export-eval|prune"
            );
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportEvalFormat {
    AgentTrace,
    ForgePayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportEvalArgs {
    session_id: String,
    output_path: Option<PathBuf>,
    format: ExportEvalFormat,
}

fn export_session_eval_trace_command(args: ExportEvalArgs) {
    let snapshot = match forge::session_store::get_snapshot(&args.session_id) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => exit_with_error(format!(
            "No local session snapshot found for {}.",
            args.session_id
        )),
        Err(error) => exit_with_error(format!("Failed to read local session snapshot: {error}")),
    };
    let (raw_events, transcript_error) =
        match forge::transcript::load_transcript_events(&args.session_id) {
            Ok(events) => (events, None),
            Err(error) => (Vec::new(), Some(error)),
        };
    let trace = build_session_eval_trace_payload(
        &args.session_id,
        &snapshot,
        &raw_events,
        transcript_error.as_deref(),
    );
    let payload = match args.format {
        ExportEvalFormat::AgentTrace => serde_json::json!({ "traces": [trace] }),
        ExportEvalFormat::ForgePayload => trace,
    };
    let json = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        exit_with_error(format!("Failed to serialize eval trace: {error}"))
    });

    if let Some(path) = args.output_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                exit_with_error(format!("Failed to create output directory: {error}"))
            });
        }
        std::fs::write(&path, format!("{json}\n"))
            .unwrap_or_else(|error| exit_with_error(format!("Failed to write output: {error}")));
    } else {
        println!("{json}");
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

async fn tail_gateway_session_events(
    session_id: &str,
    after_cursor: Option<usize>,
    limit: Option<usize>,
) {
    let socket_path = default_socket_path();
    if !socket_path.exists() {
        eprintln!(
            "Gateway is not running (no socket at {}).",
            socket_path.display()
        );
        eprintln!("Start it with: forge service start");
        std::process::exit(1);
    }

    let request = match build_tail_session_events_request(session_id, after_cursor, limit) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    match GatewayClient::connect(&socket_path).await {
        Ok(mut client) => match client.send(request).await {
            Ok(forge::gateway::protocol::GatewayReply::Ok(resp)) => {
                for line in render_session_event_tail_lines(&resp.result) {
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

async fn enqueue_gateway_session_input(session_id: &str, message: &str) {
    let socket_path = default_socket_path();
    if !socket_path.exists() {
        eprintln!(
            "Gateway is not running (no socket at {}).",
            socket_path.display()
        );
        eprintln!("Start it with: forge service start");
        std::process::exit(1);
    }

    let request = match build_enqueue_session_input_request(session_id, message) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    match GatewayClient::connect(&socket_path).await {
        Ok(mut client) => match client.send(request).await {
            Ok(forge::gateway::protocol::GatewayReply::Ok(resp)) => {
                let result = resp.result;
                let input_id = result["input_id"].as_str().unwrap_or("?");
                let session_id = result["session_id"].as_str().unwrap_or("?");
                let pending_inputs = result["pending_inputs"].as_u64().unwrap_or(0);
                println!(
                    "Queued session input {input_id} for {session_id}; pending inputs: {pending_inputs}"
                );
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

fn build_session_eval_trace_payload(
    session_id: &str,
    snapshot: &serde_json::Value,
    raw_events: &[serde_json::Value],
    transcript_error: Option<&str>,
) -> serde_json::Value {
    let latest_turn = snapshot.get("latest_turn").filter(|turn| turn.is_object());
    let user_prompt = latest_turn
        .and_then(|turn| turn.get("user_goal"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| last_message_content(snapshot, "user"))
        .unwrap_or_else(|| snapshot["summary"].as_str().unwrap_or("").to_string());
    let provider = latest_turn
        .and_then(|turn| turn.get("provider"))
        .and_then(|value| value.as_str())
        .or_else(|| snapshot["provider"].as_str())
        .unwrap_or("forge");
    let model = latest_turn
        .and_then(|turn| turn.get("model"))
        .and_then(|value| value.as_str())
        .or_else(|| snapshot["model"].as_str())
        .unwrap_or("local-forge");
    let workspace = latest_turn
        .and_then(|turn| turn.get("workspace_path"))
        .and_then(|value| value.as_str())
        .or_else(|| snapshot["working_dir"].as_str())
        .unwrap_or("");
    let tool_calls = latest_turn
        .and_then(|turn| turn.get("tools"))
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .map(tool_call_from_turn_tool_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let shell_outputs = shell_outputs_from_events(raw_events);
    let changed_files = changed_files_from_latest_turn(latest_turn);
    let verification_result = latest_turn.and_then(verification_result_from_turn_value);
    let failure = latest_turn
        .and_then(|turn| turn.get("failure"))
        .filter(|failure| failure.is_object());
    let failure_category = failure
        .and_then(|failure| failure.get("kind"))
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| {
            if verification_result
                .as_ref()
                .and_then(|value| value.get("passed"))
                .and_then(|value| value.as_bool())
                == Some(false)
            {
                "verification_failed"
            } else {
                "none"
            }
        });
    let failure_reason = failure
        .and_then(|failure| failure.get("message"))
        .and_then(|value| value.as_str());
    let provider_usage = provider_usage_from_events(raw_events);
    let duration_ms = latest_turn
        .and_then(|turn| {
            let updated_at_ms = turn.get("updated_at_ms")?.as_u64()?;
            let created_at_ms = turn.get("created_at_ms")?.as_u64()?;
            Some(updated_at_ms.saturating_sub(created_at_ms))
        })
        .or_else(|| {
            let updated_at_ms = snapshot["updated_at_ms"].as_u64()?;
            let created_at_ms = snapshot["created_at_ms"].as_u64()?;
            Some(updated_at_ms.saturating_sub(created_at_ms))
        })
        .unwrap_or(0);
    let input_tokens = provider_usage
        .get("latest")
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(|value| value.as_u64());
    let output_tokens = provider_usage
        .get("latest")
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(|value| value.as_u64());
    let confirm_requests = raw_events
        .iter()
        .filter(|event| event["event_type"].as_str() == Some("confirm_ask"))
        .count();
    let final_answer = last_message_content(snapshot, "assistant")
        .or_else(|| snapshot["summary"].as_str().map(str::to_string))
        .unwrap_or_default();
    let error_code = if failure_category == "none" {
        None
    } else {
        Some(failure_category)
    };
    let validation_attempts = if verification_result.is_some() { 1 } else { 0 };
    let forge_run_evidence = serde_json::json!({
        "schema_version": 2,
        "source": "forge_session_export",
        "session_id": session_id,
        "turn_id": latest_turn
            .and_then(|turn| turn.get("turn_id"))
            .and_then(|value| value.as_str()),
        "workspace_path": workspace,
        "prompt": user_prompt.clone(),
        "normalized_goal": latest_turn
            .and_then(|turn| turn.get("user_goal"))
            .and_then(|value| value.as_str())
            .unwrap_or(session_id),
        "prepared_context": prepared_context_from_snapshot_and_events(latest_turn, raw_events),
        "memory_audit": memory_audit_from_events(raw_events),
        "permission_decisions": permission_decisions_from_events(raw_events),
        "tool_calls": tool_calls.clone(),
        "shell_outputs": shell_outputs.clone(),
        "changed_files": changed_files.clone(),
        "file_diffs": [],
        "verification": verification_result.clone(),
        "provider_usage": provider_usage.clone(),
        "failure_category": failure_category,
        "failure_reason": failure_reason,
        "recovery": null,
        "completion_eligibility": {
            "status": "unknown"
        },
        "a2a_child_capsules": a2a_child_capsules_from_events(raw_events),
        "continuity_lessons": continuity_lessons_from_transcript_error(transcript_error),
    });

    serde_json::json!({
        "task_id": session_id,
        "session_id": session_id,
        "user_prompt": user_prompt,
        "provider": provider,
        "model": model,
        "context_files": [],
        "raw_events": raw_events,
        "tool_calls": tool_calls,
        "shell_outputs": shell_outputs,
        "file_diffs": [],
        "changed_files": changed_files,
        "verification_result": verification_result,
        "final_answer": final_answer,
        "model_rounds": latest_turn
            .and_then(|turn| turn.get("model_rounds"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        "confirm_requests": confirm_requests,
        "repair_attempts_used": 0,
        "validation_attempts": validation_attempts,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "error": error_code,
        "failure_reason": failure_reason,
        "failure_category": failure_category,
        "duration_ms": duration_ms,
        "forge_run_evidence": forge_run_evidence,
    })
}

fn last_message_content(snapshot: &serde_json::Value, role: &str) -> Option<String> {
    snapshot
        .get("messages")
        .and_then(|messages| messages.as_array())?
        .iter()
        .rev()
        .find(|message| message["role"].as_str() == Some(role))
        .and_then(|message| message["content"].as_str())
        .map(str::to_string)
}

fn tool_call_from_turn_tool_value(tool: &serde_json::Value) -> serde_json::Value {
    let command = tool["command"]
        .as_str()
        .filter(|command| !command.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            tool["affected_files"]
                .as_array()
                .and_then(|files| files.first())
                .and_then(|value| value.as_str())
                .map(|path| format!("{} {path}", tool["name"].as_str().unwrap_or("tool")))
        })
        .unwrap_or_else(|| tool["name"].as_str().unwrap_or("tool").to_string());
    let duration_ms = tool["ended_at_ms"]
        .as_u64()
        .and_then(|ended| {
            let started = tool["started_at_ms"].as_u64()?;
            Some(ended.saturating_sub(started))
        })
        .unwrap_or(0);
    serde_json::json!({
        "command": command,
        "stdout": tool["result_summary"].as_str().unwrap_or(""),
        "stderr": if tool["is_error"].as_bool().unwrap_or(false) {
            tool["result_summary"].as_str().unwrap_or("")
        } else {
            ""
        },
        "exit_code": if tool["is_error"].as_bool().unwrap_or(false) { 1 } else { 0 },
        "duration_ms": duration_ms,
    })
}

fn shell_outputs_from_events(events: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut pending: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut outputs = Vec::new();
    for event in events {
        match event["event_type"].as_str() {
            Some("shell_start") => {
                if let Some(block_id) = event["block_id"].as_str() {
                    pending.insert(
                        block_id.to_string(),
                        (
                            event["command"].as_str().unwrap_or("").to_string(),
                            String::new(),
                        ),
                    );
                }
            }
            Some("shell_output") => {
                if let Some(block_id) = event["block_id"].as_str() {
                    pending
                        .entry(block_id.to_string())
                        .or_default()
                        .1
                        .push_str(event["content"].as_str().unwrap_or(""));
                }
            }
            Some("shell_end") => {
                if let Some(block_id) = event["block_id"].as_str() {
                    let (command, stdout) = pending.remove(block_id).unwrap_or_default();
                    outputs.push(serde_json::json!({
                        "command": command,
                        "stdout": stdout,
                        "stderr": "",
                        "exit_code": event["exit_code"].as_i64().unwrap_or(0),
                        "duration_ms": 0,
                    }));
                }
            }
            _ => {}
        }
    }
    outputs
}

fn changed_files_from_latest_turn(latest_turn: Option<&serde_json::Value>) -> Vec<String> {
    let mut changed = BTreeSet::new();
    if let Some(tools) = latest_turn
        .and_then(|turn| turn.get("tools"))
        .and_then(|tools| tools.as_array())
    {
        for tool in tools {
            if let Some(files) = tool["affected_files"].as_array() {
                for file in files {
                    if let Some(path) = file.as_str().filter(|path| !path.trim().is_empty()) {
                        changed.insert(path.to_string());
                    }
                }
            }
        }
    }
    changed.into_iter().collect()
}

fn verification_result_from_turn_value(turn: &serde_json::Value) -> Option<serde_json::Value> {
    let verification = turn.get("verification")?;
    let command = verification["command"].as_str()?;
    let status = verification["status"].as_str().unwrap_or("not_needed");
    let passed = matches!(status, "passed" | "skipped");
    Some(serde_json::json!({
        "command": command,
        "passed": passed,
        "stdout": verification["stdout_preview"].as_str().unwrap_or(""),
        "stderr": verification["stderr_preview"].as_str().unwrap_or(""),
        "exit_code": verification["exit_code"].as_i64().unwrap_or(if passed { 0 } else { 1 }),
        "duration_ms": verification["duration_ms"].as_u64().unwrap_or(0),
    }))
}

fn provider_usage_from_events(events: &[serde_json::Value]) -> serde_json::Value {
    let usage_events = events
        .iter()
        .filter(|event| {
            matches!(
                event["event_type"].as_str(),
                Some("provider_usage" | "usage")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    serde_json::json!({
        "events": usage_events,
        "latest": usage_events.last(),
    })
}

fn prepared_context_from_snapshot_and_events(
    latest_turn: Option<&serde_json::Value>,
    events: &[serde_json::Value],
) -> serde_json::Value {
    serde_json::json!({
        "turn_prepared": events.iter().rev().find(|event| {
            event["event_type"].as_str() == Some("turn_prepared")
        }).and_then(|event| event.get("prepared")).cloned(),
        "turn_context": latest_turn.and_then(|turn| turn.get("context")).cloned(),
    })
}

fn memory_audit_from_events(events: &[serde_json::Value]) -> serde_json::Value {
    let prepared = events
        .iter()
        .rev()
        .find(|event| event["event_type"].as_str() == Some("turn_prepared"))
        .and_then(|event| event.get("prepared"));
    serde_json::json!({
        "selected_memory_ids": prepared
            .and_then(|prepared| prepared.get("selected_memory_ids"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
        "selected_memory_audit": prepared
            .and_then(|prepared| prepared.get("selected_memory_audit"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
        "selected_project_record_ids": prepared
            .and_then(|prepared| prepared.get("selected_project_record_ids"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    })
}

fn permission_decisions_from_events(events: &[serde_json::Value]) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter(|event| {
            matches!(
                event["event_type"].as_str(),
                Some("permission_decision" | "confirm_ask" | "confirm_response")
            )
        })
        .filter_map(|event| {
            let evidence = event
                .get("evidence")
                .or_else(|| event.get("permission_evidence"))?;
            Some(serde_json::json!({
                "event_type": event["event_type"].as_str(),
                "block_id": event["block_id"].as_str(),
                "approved": event.get("approved"),
                "evidence": evidence,
            }))
        })
        .collect()
}

fn a2a_child_capsules_from_events(events: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let Some(state) = events
        .iter()
        .rev()
        .find(|event| event["event_type"].as_str() == Some("agent_a2a_updated"))
        .and_then(|event| event.get("state"))
    else {
        return Vec::new();
    };
    let Some(tasks) = state.get("tasks").and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    let child_tasks = tasks
        .iter()
        .filter_map(|task| Some((task.get("task_id")?.as_str()?.to_string(), task)))
        .collect::<BTreeMap<_, _>>();
    let mut capsules = Vec::new();
    for task in tasks {
        let Some(task_capsules) = task
            .get("child_capsules")
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for capsule in task_capsules {
            let mut enriched = capsule.clone();
            let Some(object) = enriched.as_object_mut() else {
                capsules.push(enriched);
                continue;
            };
            let child_task_id = object
                .get("child_task_id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let child_task = child_task_id
                .as_ref()
                .and_then(|task_id| child_tasks.get(task_id));
            if let Some(child_task) = child_task {
                merge_child_task_field(object, child_task, "execution_mode");
                merge_child_task_field(object, child_task, "worktree_path");
                merge_child_task_field(object, child_task, "tests_passed");
                merge_child_task_field(object, child_task, "diff_truncated");
                merge_child_task_field(object, child_task, "cleaned_up");
                merge_child_task_field(object, child_task, "changed_file_count");
                merge_child_task_field(object, child_task, "test_report_excerpt");
                merge_child_task_field(object, child_task, "lease_owner");
                merge_child_task_field(object, child_task, "lease_acquired_at_ms");
                merge_child_task_field(object, child_task, "lease_expires_at_ms");
                merge_child_task_field(object, child_task, "attempt_count");
                merge_child_task_field(object, child_task, "max_attempts");
                merge_child_task_field(object, child_task, "runtime_events");
                merge_child_task_field(object, child_task, "review_gate");
                merge_child_task_field(object, child_task, "recovery_actions");
                merge_child_task_field(object, child_task, "review_decision");
                merge_child_task_field(object, child_task, "resume_note");
                if !object.contains_key("failure_reason") {
                    if let Some(failure_message) = child_task.get("failure_message") {
                        object.insert("failure_reason".to_string(), failure_message.clone());
                    }
                }
            }
            capsules.push(enriched);
        }
    }
    capsules
}

fn merge_child_task_field(
    target: &mut serde_json::Map<String, serde_json::Value>,
    child_task: &serde_json::Value,
    key: &str,
) {
    if target.contains_key(key) {
        return;
    }
    if let Some(value) = child_task.get(key).filter(|value| !value.is_null()) {
        target.insert(key.to_string(), value.clone());
    }
}

fn continuity_lessons_from_transcript_error(error: Option<&str>) -> Vec<serde_json::Value> {
    error
        .map(|error| {
            vec![serde_json::json!({
                "formed_count": null,
                "error": error,
            })]
        })
        .unwrap_or_default()
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

fn render_session_event_tail_lines(result: &serde_json::Value) -> Vec<String> {
    let session_id = result["session_id"].as_str().unwrap_or("?");
    let next_cursor = result["next_cursor"].as_u64().unwrap_or(0);
    let total_events = result["total_events"].as_u64().unwrap_or(0);
    let cursor_reset = result["cursor_reset"].as_bool().unwrap_or(false);
    let events = result
        .get("events")
        .and_then(|events| events.as_array())
        .cloned()
        .unwrap_or_default();
    let mut lines = vec![
        "Gateway session events:".to_string(),
        format!(
            "  {session_id}  events={} next_cursor={next_cursor} total={total_events} reset={cursor_reset}",
            events.len()
        ),
    ];
    for event in events {
        let event_type = event["event_type"].as_str().unwrap_or("event");
        let block_id = event["block_id"].as_str().unwrap_or("-");
        let preview = event
            .get("content")
            .and_then(|value| value.as_str())
            .or_else(|| event.get("message").and_then(|value| value.as_str()))
            .unwrap_or("")
            .trim();
        if preview.is_empty() {
            lines.push(format!("  {event_type}  {block_id}"));
        } else {
            lines.push(format!(
                "  {event_type}  {block_id}  {}",
                truncate_event_preview(preview)
            ));
        }
    }
    lines
}

fn truncate_event_preview(value: &str) -> String {
    const LIMIT: usize = 120;
    let mut chars = value.chars();
    let preview = chars.by_ref().take(LIMIT).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
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

fn render_rename_result_lines(snapshot: &SessionSnapshotSummary) -> Vec<String> {
    let mut lines = vec![
        "Session renamed:".to_string(),
        format!(
            "  {}  {}/{}  {}",
            snapshot.session_id, snapshot.provider, snapshot.model, snapshot.working_dir
        ),
    ];
    if let Some(summary) = snapshot.summary.as_deref() {
        if !summary.trim().is_empty() {
            lines.push(format!("  summary  {}", summary.trim()));
        }
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

fn parse_export_eval_args(args: &[String]) -> Result<ExportEvalArgs, String> {
    let Some(session_id) = args
        .first()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Err("session_id is required".to_string());
    };
    let mut output_path = None;
    let mut format = ExportEvalFormat::AgentTrace;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                index += 1;
                let Some(path) = args.get(index).filter(|path| !path.trim().is_empty()) else {
                    return Err("--output requires a path".to_string());
                };
                output_path = Some(PathBuf::from(path));
            }
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("agent-trace") => ExportEvalFormat::AgentTrace,
                    Some("forge-payload") => ExportEvalFormat::ForgePayload,
                    Some(other) => {
                        return Err(format!(
                            "unsupported --format {other}; expected agent-trace or forge-payload"
                        ));
                    }
                    None => return Err("--format requires a value".to_string()),
                };
            }
            other => return Err(format!("unknown export-eval option: {other}")),
        }
        index += 1;
    }

    Ok(ExportEvalArgs {
        session_id: session_id.to_string(),
        output_path,
        format,
    })
}

fn parse_events_args(args: &[String]) -> Result<(Option<usize>, Option<usize>), String> {
    let mut after_cursor = None;
    let mut limit = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--after" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--after requires a cursor".to_string())?;
                after_cursor = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "--after must be a non-negative integer".to_string())?,
                );
                index += 2;
            }
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--limit requires a count".to_string())?;
                limit = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a non-negative integer".to_string())?,
                );
                index += 2;
            }
            unknown => return Err(format!("Unknown events option: {unknown}")),
        }
    }
    Ok((after_cursor, limit))
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
    use forge::session_store::{
        SessionSnapshotPruneReport, SessionSnapshotStoreStats, SessionSnapshotSummary,
    };
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
    fn render_rename_result_lines_shows_new_summary() {
        let summary = SessionSnapshotSummary {
            session_id: "session-rename".to_string(),
            provider: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            working_dir: "/repo/rename".to_string(),
            summary: Some("Launch plan".to_string()),
            created_at_ms: 10,
            updated_at_ms: 20,
            message_count: 3,
        };

        let lines = super::render_rename_result_lines(&summary);

        assert_eq!(lines[0], "Session renamed:");
        assert_eq!(
            lines[1],
            "  session-rename  deepseek/deepseek-v4-flash  /repo/rename"
        );
        assert_eq!(lines[2], "  summary  Launch plan");
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
    fn parse_events_args_accepts_after_and_limit() {
        let args = vec![
            "--after".to_string(),
            "2".to_string(),
            "--limit".to_string(),
            "10".to_string(),
        ];

        assert_eq!(
            super::parse_events_args(&args).unwrap(),
            (Some(2), Some(10))
        );
        assert!(super::parse_events_args(&["--after".to_string()]).is_err());
    }

    #[test]
    fn parse_export_eval_args_accepts_output_and_format() {
        let args = vec![
            "session-1".to_string(),
            "--output".to_string(),
            "tmp/trace.json".to_string(),
            "--format".to_string(),
            "forge-payload".to_string(),
        ];

        let parsed = super::parse_export_eval_args(&args).expect("parse");

        assert_eq!(parsed.session_id, "session-1");
        assert_eq!(
            parsed.output_path.unwrap(),
            std::path::PathBuf::from("tmp/trace.json")
        );
        assert_eq!(parsed.format, super::ExportEvalFormat::ForgePayload);
        assert!(super::parse_export_eval_args(&[]).is_err());
    }

    #[test]
    fn build_session_eval_trace_payload_uses_snapshot_and_transcript_facts() {
        let snapshot = serde_json::json!({
            "session_id": "session-1",
            "provider": "deepseek",
            "model": "deepseek-v4-flash",
            "working_dir": "/workspace/app",
            "summary": "Fixed feedback",
            "created_at_ms": 100,
            "updated_at_ms": 500,
            "messages": [
                {"role": "user", "content": "Fix button feedback."},
                {"role": "assistant", "content": "Done."}
            ],
            "latest_turn": {
                "turn_id": "turn-1",
                "session_id": "session-1",
                "workspace_path": "/workspace/app",
                "provider": "deepseek",
                "model": "deepseek-v4-flash",
                "route": "direct",
                "phase": "fix",
                "user_goal": "Fix button feedback.",
                "context": {
                    "sources": [
                        {
                            "kind": "user_input",
                            "label": "prompt",
                            "reason": "visible input",
                            "estimated_tokens": 4,
                            "injected": true
                        }
                    ],
                    "estimated_tokens": 4,
                    "budget_tokens": 1000,
                    "omitted_sources": []
                },
                "tools": [
                    {
                        "tool_call_id": "tool-1",
                        "name": "edit_file",
                        "category": "write",
                        "status": "completed",
                        "started_at_ms": 120,
                        "ended_at_ms": 180,
                        "result_summary": "Edited src/App.tsx",
                        "is_error": false,
                        "affected_files": ["src/App.tsx"],
                        "command": null
                    }
                ],
                "verification": {
                    "status": "passed",
                    "command": "npm test",
                    "exit_code": 0,
                    "stdout_preview": "passed",
                    "stderr_preview": null,
                    "duration_ms": 75,
                    "completed_at_ms": 300
                },
                "status": "completed",
                "model_rounds": 2,
                "tool_call_count": 1,
                "failed_tool_count": 0,
                "compact_saved_tokens": 0,
                "created_at_ms": 100,
                "updated_at_ms": 400
            }
        });
        let events = vec![
            serde_json::json!({
                "event_type": "turn_prepared",
                "session_id": "session-1",
                "prepared": {
                    "selected_memory_ids": ["memory-1"],
                    "selected_memory_audit": [
                        {
                            "memory_id": "memory-1",
                            "source": "profile",
                            "source_id": "profile-1",
                            "kind": "preference",
                            "score": 0.9,
                            "reason": "project match",
                            "project_match": true,
                            "profile_match": true,
                            "injected": true
                        }
                    ],
                    "selected_project_record_ids": ["record-1"],
                    "context_estimate": {
                        "sources": [{"kind": "user_input", "label": "prompt"}]
                    }
                }
            }),
            serde_json::json!({
                "event_type": "permission_decision",
                "session_id": "session-1",
                "block_id": "permission-1",
                "evidence": {"decision": "allow", "approved": true}
            }),
            serde_json::json!({
                "event_type": "provider_usage",
                "session_id": "session-1",
                "input_tokens": 111,
                "output_tokens": 22,
                "estimated_cost_micros": 9
            }),
            serde_json::from_str::<serde_json::Value>(
                r#"{
                    "event_type": "agent_a2a_updated",
                    "session_id": "session-1",
                    "state": {
                        "running_count": 0,
                        "completed_count": 1,
                        "failed_count": 1,
                        "interrupted_count": 0,
                        "tasks": [
                            {
                                "task_id": "parent-1",
                                "agent_id": "agent-parent",
                                "role": "reviewer",
                                "execution_mode": "read_only",
                                "status": "completed",
                                "title": "Parent",
                                "messages": [],
                                "latest_message": null,
                                "failure_message": null,
                                "updated_at_ms": 380,
                                "artifact_count": 0,
                                "latest_artifact_kind": null,
                                "latest_artifact_title": null,
                                "needs_human_review": null,
                                "reason_codes": [],
                                "tests_passed": null,
                                "diff_truncated": null,
                                "worktree_path": null,
                                "cleaned_up": null,
                                "suggested_action": null,
                                "child_capsules": [
                                    {
                                        "capsule_id": "child-capsule:parent-1:child-1",
                                        "parent_task_id": "parent-1",
                                        "child_task_id": "child-1",
                                        "child_goal": "Patch button feedback",
                                        "status": "completed",
                                        "artifact_titles": ["Patch proposal", "Worktree diff"],
                                        "changed_files": ["src/App.tsx"],
                                        "review_decision": "approved",
                                        "failure_reason": null,
                                        "next_action": "Review child evidence before parent completion.",
                                        "estimated_tokens": 24
                                    }
                                ]
                            },
                            {
                                "task_id": "child-1",
                                "agent_id": "agent-child",
                                "role": "implementer",
                                "execution_mode": "worktree_worker",
                                "status": "completed",
                                "title": "Child",
                                "messages": [],
                                "latest_message": null,
                                "failure_message": null,
                                "updated_at_ms": 370,
                                "artifact_count": 2,
                                "latest_artifact_kind": "diff_summary",
                                "latest_artifact_title": "Worktree diff",
                                "needs_human_review": false,
                                "reason_codes": [],
                                "tests_passed": true,
                                "diff_truncated": false,
                                "worktree_path": "/tmp/forge-child",
                                "cleaned_up": false,
                                "suggested_action": "Review approved by controller.",
                                "review_decision": "approved",
                                "runtime_events": [
                                    {
                                        "kind": "assigned",
                                        "label": "Assigned",
                                        "detail": "Child worker assigned",
                                        "created_at_ms": 300
                                    },
                                    {
                                        "kind": "lease_claimed",
                                        "label": "Lease claimed",
                                        "detail": "worker-1",
                                        "created_at_ms": 305
                                    },
                                    {
                                        "kind": "started",
                                        "label": "Started",
                                        "detail": "Worktree worker started",
                                        "created_at_ms": 310
                                    },
                                    {
                                        "kind": "file_fact",
                                        "label": "File fact",
                                        "detail": "src/App.tsx",
                                        "created_at_ms": 340
                                    },
                                    {
                                        "kind": "completed",
                                        "label": "Completed",
                                        "detail": "Worktree worker completed",
                                        "created_at_ms": 370
                                    }
                                ],
                                "review_gate": {
                                    "kind": "approved",
                                    "label": "Review approved",
                                    "reason": "ship it",
                                    "completion_impact": "child_review_approved_only",
                                    "parent_task_id": "parent-1",
                                    "child_task_id": "child-1",
                                    "reviewed_at_ms": 360
                                },
                                "recovery_actions": []
                            }
                        ]
                    }
                }"#,
            )
            .expect("a2a projection fixture"),
            serde_json::json!({
                "event_type": "shell_start",
                "session_id": "session-1",
                "block_id": "shell-1",
                "command": "npm test"
            }),
            serde_json::json!({
                "event_type": "shell_output",
                "session_id": "session-1",
                "block_id": "shell-1",
                "content": "passed\n"
            }),
            serde_json::json!({
                "event_type": "shell_end",
                "session_id": "session-1",
                "block_id": "shell-1",
                "exit_code": 0
            }),
        ];

        let payload =
            super::build_session_eval_trace_payload("session-1", &snapshot, &events, None);

        assert_eq!(payload["task_id"], "session-1");
        assert_eq!(payload["user_prompt"], "Fix button feedback.");
        assert_eq!(payload["tool_calls"][0]["command"], "edit_file src/App.tsx");
        assert_eq!(payload["shell_outputs"][0]["stdout"], "passed\n");
        assert_eq!(payload["changed_files"], serde_json::json!(["src/App.tsx"]));
        assert_eq!(payload["verification_result"]["passed"], true);
        assert_eq!(payload["input_tokens"], 111);
        assert_eq!(payload["output_tokens"], 22);
        assert_eq!(payload["forge_run_evidence"]["schema_version"], 2);
        assert_eq!(
            payload["forge_run_evidence"]["completion_eligibility"]["status"],
            "unknown"
        );
        assert_eq!(
            payload["forge_run_evidence"]["memory_audit"]["selected_memory_ids"],
            serde_json::json!(["memory-1"])
        );
        assert_eq!(
            payload["forge_run_evidence"]["permission_decisions"][0]["evidence"]["decision"],
            "allow"
        );
        assert_eq!(
            payload["forge_run_evidence"]["provider_usage"]["latest"]["input_tokens"],
            111
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["child_task_id"],
            "child-1"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["review_gate"]["kind"],
            "approved"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["execution_mode"],
            "worktree_worker"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["worktree_path"],
            "/tmp/forge-child"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["tests_passed"],
            true
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["diff_truncated"],
            false
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["cleaned_up"],
            false
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["runtime_events"][0]["kind"],
            "assigned"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["runtime_events"][3]["kind"],
            "file_fact"
        );
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
    fn render_session_event_tail_lines_shows_cursor_and_event_preview() {
        let result = serde_json::json!({
            "ok": true,
            "session_id": "session-1",
            "events": [
                {
                    "event_type": "user_message",
                    "block_id": "user-1",
                    "content": "continue"
                }
            ],
            "next_cursor": 4,
            "total_events": 6,
            "cursor_reset": false
        });

        let lines = super::render_session_event_tail_lines(&result);

        assert_eq!(lines[0], "Gateway session events:");
        assert_eq!(
            lines[1],
            "  session-1  events=1 next_cursor=4 total=6 reset=false"
        );
        assert_eq!(lines[2], "  user_message  user-1  continue");
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
