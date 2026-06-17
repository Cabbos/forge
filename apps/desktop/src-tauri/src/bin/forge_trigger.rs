//! Forge Trigger CLI - enqueue and inspect gateway triggers.
//!
//! Usage:
//! - `forge_trigger enqueue --message "run digest" [--profile id] [--provider name] [--model name] [--workspace path]`
//! - `forge_trigger list`
//! - `forge_trigger runs`
//! - `forge_trigger replay --run-id <run-id>`
//! - `forge_trigger status`

use forge::gateway::client::{build_dashboard_snapshot_request, GatewayClient};
use forge::gateway::protocol::{
    EnqueueTriggerParams, EnqueueTriggerResult, GatewayReply, GatewayRequest, GetTriggerRunParams,
    GetTriggerRunResult, ReplayTriggerRunParams, ReplayTriggerRunResult,
};
use forge::gateway::runner::TriggerRunRecord;
use forge::gateway::server::{
    default_socket_path, GatewayDashboardEventLogEntry, GatewayDashboardSnapshot,
    GatewayRuntimeStatus,
};
use forge::gateway::webhook::PendingTrigger;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerCommand {
    Enqueue,
    List,
    Runs,
    Replay,
    Show,
    Status,
    Dashboard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTriggerArgs {
    command: TriggerCommand,
    message: Option<String>,
    run_id: Option<String>,
    trigger_id: Option<String>,
    profile_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    workspace_path: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = match parse_trigger_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", usage());
            std::process::exit(1);
        }
    };

    let socket_path = default_socket_path();
    if !socket_path.exists() {
        eprintln!(
            "Gateway is not running (no socket at {}).",
            socket_path.display()
        );
        eprintln!("Start it with: forge service start");
        std::process::exit(1);
    }

    let mut client = match GatewayClient::connect(&socket_path).await {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Failed to connect to gateway: {error}");
            std::process::exit(1);
        }
    };

    let result = match args.command {
        TriggerCommand::Enqueue => enqueue_trigger(&mut client, args).await,
        TriggerCommand::List => list_pending_triggers(&mut client).await,
        TriggerCommand::Runs => list_trigger_runs(&mut client).await,
        TriggerCommand::Replay => replay_trigger_run(&mut client, args).await,
        TriggerCommand::Show => show_trigger_run(&mut client, args).await,
        TriggerCommand::Status => show_runtime_status(&mut client).await,
        TriggerCommand::Dashboard => show_dashboard_snapshot(&mut client).await,
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn parse_trigger_args<I, S>(args: I) -> Result<ParsedTriggerArgs, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter().map(|arg| arg.as_ref().to_string());
    let Some(command) = iter.next() else {
        return Err("trigger command is required".to_string());
    };

    let command = match command.as_str() {
        "enqueue" => TriggerCommand::Enqueue,
        "list" => TriggerCommand::List,
        "runs" => TriggerCommand::Runs,
        "replay" => TriggerCommand::Replay,
        "show" => TriggerCommand::Show,
        "status" => TriggerCommand::Status,
        "dashboard" => TriggerCommand::Dashboard,
        other => return Err(format!("unknown trigger command: {other}")),
    };

    let mut parsed = ParsedTriggerArgs {
        command,
        message: None,
        run_id: None,
        trigger_id: None,
        profile_id: None,
        provider: None,
        model: None,
        workspace_path: None,
    };
    let mut positional_message: Vec<String> = Vec::new();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--message" | "-m" => parsed.message = Some(next_value(&mut iter, "--message")?),
            "--id"
                if matches!(
                    parsed.command,
                    TriggerCommand::Replay | TriggerCommand::Show
                ) =>
            {
                parsed.run_id = Some(next_value(&mut iter, "--id")?)
            }
            "--id" | "--trigger-id" => parsed.trigger_id = Some(next_value(&mut iter, "--id")?),
            "--run-id" => parsed.run_id = Some(next_value(&mut iter, "--run-id")?),
            "--profile" | "-p" => parsed.profile_id = Some(next_value(&mut iter, "--profile")?),
            "--provider" => parsed.provider = Some(next_value(&mut iter, "--provider")?),
            "--model" => parsed.model = Some(next_value(&mut iter, "--model")?),
            "--workspace" | "-w" => {
                parsed.workspace_path = Some(next_value(&mut iter, "--workspace")?)
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option: {flag}")),
            value => positional_message.push(value.to_string()),
        }
    }

    if parsed.command == TriggerCommand::Enqueue {
        if parsed.message.is_none() && !positional_message.is_empty() {
            parsed.message = Some(positional_message.join(" "));
        }
        parsed.message = clean_optional(parsed.message);
        if parsed.message.is_none() {
            return Err("message is required for trigger enqueue".to_string());
        }
    } else if matches!(
        parsed.command,
        TriggerCommand::Replay | TriggerCommand::Show
    ) {
        if parsed.run_id.is_none() && positional_message.len() == 1 {
            parsed.run_id = positional_message.pop();
        } else if !positional_message.is_empty() {
            return Err("unexpected positional argument".to_string());
        }
        parsed.run_id = clean_optional(parsed.run_id);
        if parsed.run_id.is_none() {
            return Err(format!(
                "run_id is required for trigger {}",
                match parsed.command {
                    TriggerCommand::Replay => "replay",
                    TriggerCommand::Show => "show",
                    _ => unreachable!(),
                }
            ));
        }
    } else if !positional_message.is_empty() {
        return Err("unexpected positional argument".to_string());
    }

    parsed.trigger_id = clean_optional(parsed.trigger_id);
    parsed.run_id = clean_optional(parsed.run_id);
    parsed.profile_id = clean_optional(parsed.profile_id);
    parsed.provider = clean_optional(parsed.provider);
    parsed.model = clean_optional(parsed.model);
    parsed.workspace_path = clean_optional(parsed.workspace_path);

    Ok(parsed)
}

fn next_value<I>(iter: &mut I, flag: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    let Some(value) = iter.next() else {
        return Err(format!("{flag} requires a value"));
    };
    if value.starts_with('-') {
        return Err(format!("{flag} requires a value"));
    }
    Ok(value)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn enqueue_trigger(
    client: &mut GatewayClient,
    args: ParsedTriggerArgs,
) -> Result<(), String> {
    let params = EnqueueTriggerParams {
        message: args
            .message
            .ok_or_else(|| "message is required for trigger enqueue".to_string())?,
        trigger_id: args.trigger_id,
        profile_id: args.profile_id,
        provider: args.provider,
        model: args.model,
        workspace_path: args.workspace_path,
    };

    let reply = send(
        client,
        "enqueue_trigger",
        Some(serde_json::to_value(params).unwrap()),
    )
    .await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let result = serde_json::from_value::<EnqueueTriggerResult>(response.result)
        .map_err(|error| format!("Gateway returned invalid enqueue result: {error}"))?;

    println!(
        "Queued trigger {}. Pending triggers: {}.",
        result.trigger_id, result.pending_triggers
    );
    Ok(())
}

async fn list_pending_triggers(client: &mut GatewayClient) -> Result<(), String> {
    let reply = send(client, "list_pending_triggers", None).await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let triggers = serde_json::from_value::<Vec<PendingTrigger>>(response.result)
        .map_err(|error| format!("Gateway returned invalid trigger list: {error}"))?;

    if triggers.is_empty() {
        println!("No pending triggers.");
        return Ok(());
    }

    println!("Pending triggers:");
    for trigger in triggers {
        let state = if trigger.claimed_at_ms.is_some() {
            "claimed"
        } else {
            "pending"
        };
        let profile = trigger.profile_id.as_deref().unwrap_or("-");
        println!(
            "  {}  {}  profile={}  attempts={}  {}",
            trigger.id,
            state,
            profile,
            trigger.attempt_count,
            truncate(&trigger.message, 80)
        );
    }
    Ok(())
}

async fn list_trigger_runs(client: &mut GatewayClient) -> Result<(), String> {
    let reply = send(client, "list_trigger_runs", None).await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let runs = serde_json::from_value::<Vec<TriggerRunRecord>>(response.result)
        .map_err(|error| format!("Gateway returned invalid trigger runs: {error}"))?;

    if runs.is_empty() {
        println!("No trigger runs.");
        return Ok(());
    }

    println!("Recent trigger runs:");
    for run in runs.into_iter().take(20) {
        println!("{}", render_trigger_run_summary_line(&run));
    }
    Ok(())
}

fn render_trigger_run_summary_line(run: &TriggerRunRecord) -> String {
    let session = run.session_id.as_deref().unwrap_or("-");
    format!(
        "  {}  trigger={}  session={}  attempt={}  {}  {}",
        run.id,
        run.trigger_id,
        session,
        run.attempt,
        run.status,
        truncate(&run.message, 80)
    )
}

async fn replay_trigger_run(
    client: &mut GatewayClient,
    args: ParsedTriggerArgs,
) -> Result<(), String> {
    let params = ReplayTriggerRunParams {
        run_id: args
            .run_id
            .ok_or_else(|| "run_id is required for trigger replay".to_string())?,
    };
    let reply = send(
        client,
        "replay_trigger_run",
        Some(serde_json::to_value(params).unwrap()),
    )
    .await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let result = serde_json::from_value::<ReplayTriggerRunResult>(response.result)
        .map_err(|error| format!("Gateway returned invalid replay result: {error}"))?;

    println!(
        "Replayed run {} as trigger {}. Pending triggers: {}.",
        result.run_id, result.trigger_id, result.pending_triggers
    );
    Ok(())
}

async fn show_trigger_run(
    client: &mut GatewayClient,
    args: ParsedTriggerArgs,
) -> Result<(), String> {
    let params = GetTriggerRunParams {
        run_id: args
            .run_id
            .ok_or_else(|| "run_id is required for trigger show".to_string())?,
    };
    let reply = send(
        client,
        "get_trigger_run",
        Some(serde_json::to_value(params).unwrap()),
    )
    .await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let result = serde_json::from_value::<GetTriggerRunResult>(response.result)
        .map_err(|error| format!("Gateway returned invalid trigger run detail: {error}"))?;

    let run = result.run;
    println!("Trigger run {}", run.id);
    println!("  trigger: {}", run.trigger_id);
    println!("  session: {}", run.session_id.as_deref().unwrap_or("-"));
    println!("  status: {}", run.status);
    println!("  attempt: {}", run.attempt);
    println!("  started_at_ms: {}", run.started_at_ms);
    println!("  ended_at_ms: {}", run.ended_at_ms);
    println!("  profile: {}", run.profile_id.as_deref().unwrap_or("-"));
    println!("  provider: {}", run.provider.as_deref().unwrap_or("-"));
    println!("  model: {}", run.model.as_deref().unwrap_or("-"));
    println!(
        "  workspace: {}",
        run.workspace_path.as_deref().unwrap_or("-")
    );
    println!("  message: {}", run.message);
    if let Some(trigger_message) = run.trigger_message.as_deref() {
        println!("  trigger_message: {}", trigger_message);
    }
    Ok(())
}

async fn show_runtime_status(client: &mut GatewayClient) -> Result<(), String> {
    let reply = send(client, "runtime_status", None).await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let status = serde_json::from_value::<GatewayRuntimeStatus>(response.result)
        .map_err(|error| format!("Gateway returned invalid runtime status: {error}"))?;

    for line in render_runtime_status_lines(&status) {
        println!("{line}");
    }
    Ok(())
}

async fn show_dashboard_snapshot(client: &mut GatewayClient) -> Result<(), String> {
    let reply = client
        .send(build_dashboard_snapshot_request())
        .await
        .map_err(|error| format!("Request failed: {error}"))?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let snapshot = serde_json::from_value::<GatewayDashboardSnapshot>(response.result)
        .map_err(|error| format!("Gateway returned invalid dashboard snapshot: {error}"))?;

    for line in render_dashboard_snapshot_lines(&snapshot) {
        println!("{line}");
    }
    Ok(())
}

fn render_runtime_status_lines(status: &GatewayRuntimeStatus) -> Vec<String> {
    let mut lines = vec![
        status.message.clone(),
        format!("Uptime: {}s", status.uptime_seconds),
        format!("Active sessions: {}", status.active_sessions),
        format!("Pending triggers: {}", status.pending_triggers),
        format!("Pending session inputs: {}", status.pending_session_inputs),
        format!("Loop runner: {}", status.loop_runner),
        format!("Pending loop tasks: {}", status.pending_loop_tasks),
        format!("Running loop tasks: {}", status.running_loop_tasks),
        format!("Stale loop task leases: {}", status.stale_loop_task_leases),
        format!("Claimed triggers: {}", status.claimed_triggers),
        format!("Dead-letter runs: {}", status.dead_letter_runs),
    ];
    if !status.runtime_tasks.is_empty() {
        lines.push("Runtime tasks:".to_string());
        for task in &status.runtime_tasks {
            let state = if task.running { "running" } else { "stopped" };
            if let Some(error) = task.last_error.as_deref() {
                lines.push(format!("  {}: {} ({})", task.name, state, error));
            } else {
                lines.push(format!("  {}: {}", task.name, state));
            }
        }
    }
    if !status.recent_session_inputs.is_empty() {
        lines.push("Recent session inputs:".to_string());
        for input in status.recent_session_inputs.iter().take(5) {
            lines.push(format!(
                "  {}  session={}  completed={}  {}",
                input.input_id,
                input.session_id,
                input.completed_at_ms,
                truncate(&input.message_preview, 80)
            ));
        }
    }
    lines
}

fn render_dashboard_snapshot_lines(snapshot: &GatewayDashboardSnapshot) -> Vec<String> {
    let mut lines = vec![
        format!("Gateway dashboard snapshot @ {}", snapshot.generated_at_ms),
        format!("Sessions: {}", snapshot.sessions.len()),
        format!("Queued triggers: {}", snapshot.queued_triggers.len()),
        format!("Recent runs: {}", snapshot.recent_runs.len()),
        format!(
            "Recent completed session inputs: {}",
            snapshot.recent_session_inputs.len()
        ),
    ];

    lines.extend(render_runtime_status_lines(&snapshot.status));

    if !snapshot.sessions.is_empty() {
        lines.push("Sessions:".to_string());
        for session in snapshot.sessions.iter().take(10) {
            lines.push(format!(
                "  {}  {}/{}  {}",
                session.session_id, session.provider, session.model, session.workspace_path
            ));
        }
    }

    if !snapshot.queued_triggers.is_empty() {
        lines.push("Queued triggers:".to_string());
        for trigger in snapshot.queued_triggers.iter().take(10) {
            let state = if trigger.claimed_at_ms.is_some() {
                "claimed"
            } else {
                "pending"
            };
            lines.push(format!(
                "  {}  {}  profile={}  attempts={}  {}",
                trigger.id,
                state,
                trigger.profile_id.as_deref().unwrap_or("-"),
                trigger.attempt_count,
                truncate(&trigger.message, 80)
            ));
        }
    }

    if !snapshot.event_log.is_empty() {
        lines.push("Event log:".to_string());
        for entry in snapshot.event_log.iter().take(10) {
            lines.push(render_dashboard_event_line(entry));
        }
    }

    lines
}

fn render_dashboard_event_line(entry: &GatewayDashboardEventLogEntry) -> String {
    format!(
        "  {}  {}  session={}  {}",
        entry.kind,
        entry.id,
        entry.session_id.as_deref().unwrap_or("-"),
        truncate(&entry.message, 80)
    )
}

async fn send(
    client: &mut GatewayClient,
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<GatewayReply, String> {
    client
        .send(GatewayRequest {
            id: uuid::Uuid::now_v7().simple().to_string(),
            method: method.to_string(),
            params,
        })
        .await
        .map_err(|error| format!("Request failed: {error}"))
}

fn render_gateway_error(reply: GatewayReply) -> String {
    match reply {
        GatewayReply::Err(error) => {
            format!(
                "Gateway error: {} (code: {})",
                error.error.message, error.error.code
            )
        }
        GatewayReply::Ok(_) => "Gateway returned an unexpected response.".to_string(),
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn usage() -> &'static str {
    "Usage: forge_trigger <enqueue|list|runs|replay|show|status|dashboard> [options]\n\
     \n\
     enqueue options:\n\
       --message, -m <text>     Prompt/message to queue\n\
       --id, --trigger-id <id>  Optional trigger id\n\
       --profile, -p <id>      Profile id\n\
       --provider <name>       Provider override\n\
       --model <name>          Model override\n\
       --workspace, -w <path>  Workspace override\n\
     \n\
     replay options:\n\
       --run-id, --id <id>     Trigger run id to replay\n\
     \n\
     show options:\n\
       --run-id, --id <id>     Trigger run id to inspect"
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_enqueue_collects_metadata_flags() {
        let parsed = super::parse_trigger_args([
            "enqueue",
            "--message",
            "run digest",
            "--profile",
            "ops",
            "--provider",
            "openai",
            "--model",
            "gpt-5",
            "--workspace",
            "/repo/workspace",
        ])
        .expect("parse");

        assert_eq!(parsed.command, super::TriggerCommand::Enqueue);
        assert_eq!(parsed.message.as_deref(), Some("run digest"));
        assert_eq!(parsed.profile_id.as_deref(), Some("ops"));
        assert_eq!(parsed.provider.as_deref(), Some("openai"));
        assert_eq!(parsed.model.as_deref(), Some("gpt-5"));
        assert_eq!(parsed.workspace_path.as_deref(), Some("/repo/workspace"));
    }

    #[test]
    fn parse_enqueue_uses_positional_message() {
        let parsed = super::parse_trigger_args(["enqueue", "run digest"]).expect("parse");

        assert_eq!(parsed.command, super::TriggerCommand::Enqueue);
        assert_eq!(parsed.message.as_deref(), Some("run digest"));
    }

    #[test]
    fn parse_enqueue_rejects_blank_message() {
        let err = super::parse_trigger_args(["enqueue", "--message", "   "]).expect_err("blank");

        assert!(err.contains("message is required"));
    }

    #[test]
    fn parse_known_read_commands() {
        assert_eq!(
            super::parse_trigger_args(["status"])
                .expect("status")
                .command,
            super::TriggerCommand::Status
        );
        assert_eq!(
            super::parse_trigger_args(["dashboard"])
                .expect("dashboard")
                .command,
            super::TriggerCommand::Dashboard
        );
        assert_eq!(
            super::parse_trigger_args(["list"]).expect("list").command,
            super::TriggerCommand::List
        );
        assert_eq!(
            super::parse_trigger_args(["runs"]).expect("runs").command,
            super::TriggerCommand::Runs
        );
    }

    #[test]
    fn parse_replay_collects_run_id() {
        let parsed = super::parse_trigger_args(["replay", "--run-id", " run-1 "]).expect("parse");

        assert_eq!(parsed.command, super::TriggerCommand::Replay);
        assert_eq!(parsed.run_id.as_deref(), Some("run-1"));
    }

    #[test]
    fn parse_replay_rejects_missing_run_id() {
        let err = super::parse_trigger_args(["replay"]).expect_err("missing run id");

        assert!(err.contains("run_id is required"));
    }

    #[test]
    fn parse_show_collects_run_id() {
        let parsed = super::parse_trigger_args(["show", " run-1 "]).expect("parse");

        assert_eq!(parsed.command, super::TriggerCommand::Show);
        assert_eq!(parsed.run_id.as_deref(), Some("run-1"));
    }

    #[test]
    fn parse_show_accepts_id_alias() {
        let parsed = super::parse_trigger_args(["show", "--id", " run-2 "]).expect("parse");

        assert_eq!(parsed.command, super::TriggerCommand::Show);
        assert_eq!(parsed.run_id.as_deref(), Some("run-2"));
    }

    #[test]
    fn parse_show_rejects_missing_run_id() {
        let err = super::parse_trigger_args(["show"]).expect_err("missing run id");

        assert!(err.contains("run_id is required"));
    }

    #[test]
    fn render_trigger_run_summary_line_includes_session_id_when_available() {
        let run = super::TriggerRunRecord {
            id: "run-1".into(),
            trigger_id: "trigger-1".into(),
            session_id: Some("gateway-session-1".into()),
            attempt: 2,
            status: "completed".into(),
            message: "done".into(),
            started_at_ms: 10,
            ended_at_ms: 20,
            trigger_message: Some("run digest".into()),
            profile_id: Some("ops".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5".into()),
            workspace_path: Some("/repo".into()),
        };

        assert_eq!(
            super::render_trigger_run_summary_line(&run),
            "  run-1  trigger=trigger-1  session=gateway-session-1  attempt=2  completed  done"
        );
    }

    #[test]
    fn render_runtime_status_lines_include_recent_session_inputs() {
        let status = super::GatewayRuntimeStatus {
            ok: true,
            message: "Gateway runtime is reachable.".into(),
            uptime_seconds: 42,
            active_sessions: 1,
            pending_triggers: 0,
            pending_session_inputs: 0,
            loop_runner: "started".into(),
            pending_loop_tasks: 2,
            running_loop_tasks: 1,
            stale_loop_task_leases: 0,
            claimed_triggers: 0,
            dead_letter_runs: 0,
            recent_runs: Vec::new(),
            recent_session_inputs: vec![
                forge::gateway::session_input::SessionInputCompletionRecord {
                    input_id: "input-1".into(),
                    session_id: "session-1".into(),
                    message_preview: "continue".into(),
                    received_at_ms: 10,
                    completed_at_ms: 20,
                },
            ],
            runtime_tasks: Vec::new(),
        };

        let lines = super::render_runtime_status_lines(&status);

        assert!(lines.contains(&"Loop runner: started".to_string()));
        assert!(lines.contains(&"Pending loop tasks: 2".to_string()));
        assert!(lines.contains(&"Running loop tasks: 1".to_string()));
        assert!(lines.contains(&"Recent session inputs:".to_string()));
        assert!(lines.contains(&"  input-1  session=session-1  completed=20  continue".to_string()));
    }

    #[test]
    fn render_dashboard_snapshot_lines_include_core_operational_sections() {
        let snapshot = super::GatewayDashboardSnapshot {
            ok: true,
            generated_at_ms: 100,
            status: super::GatewayRuntimeStatus {
                ok: true,
                message: "Gateway runtime is reachable.".into(),
                uptime_seconds: 42,
                active_sessions: 1,
                pending_triggers: 1,
                pending_session_inputs: 0,
                loop_runner: "stopped".into(),
                pending_loop_tasks: 0,
                running_loop_tasks: 0,
                stale_loop_task_leases: 0,
                claimed_triggers: 1,
                dead_letter_runs: 0,
                recent_runs: Vec::new(),
                recent_session_inputs: Vec::new(),
                runtime_tasks: Vec::new(),
            },
            sessions: vec![forge::gateway::protocol::GatewaySessionInfo {
                session_id: "session-1".into(),
                provider: "claude".into(),
                model: "sonnet".into(),
                workspace_path: "/repo".into(),
                created_at_ms: 1,
                owner_pid: Some(42),
                last_seen_at_ms: Some(90),
                restored_from_registry: false,
            }],
            queued_triggers: vec![forge::gateway::webhook::PendingTrigger {
                id: "trigger-1".into(),
                message: "run digest".into(),
                profile_id: Some("ops".into()),
                provider: Some("claude".into()),
                model: Some("sonnet".into()),
                workspace_path: Some("/repo".into()),
                attempt_count: 1,
                claimed_at_ms: Some(99),
                received_at_ms: 80,
            }],
            recent_runs: Vec::new(),
            recent_session_inputs: Vec::new(),
            event_log: vec![super::GatewayDashboardEventLogEntry {
                kind: "trigger_run".into(),
                id: "run-1".into(),
                message: "completed: ok".into(),
                at_ms: 95,
                session_id: Some("session-1".into()),
            }],
        };

        let lines = super::render_dashboard_snapshot_lines(&snapshot);

        assert!(lines.contains(&"Gateway dashboard snapshot @ 100".to_string()));
        assert!(lines.contains(&"Sessions: 1".to_string()));
        assert!(lines.contains(&"Queued triggers: 1".to_string()));
        assert!(lines.contains(&"  session-1  claude/sonnet  /repo".to_string()));
        assert!(lines
            .contains(&"  trigger-1  claimed  profile=ops  attempts=1  run digest".to_string()));
        assert!(
            lines.contains(&"  trigger_run  run-1  session=session-1  completed: ok".to_string())
        );
    }
}
