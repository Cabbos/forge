//! Forge Trigger CLI - enqueue and inspect gateway triggers.
//!
//! Usage:
//! - `forge_trigger enqueue --message "run digest" [--profile id] [--provider name] [--model name] [--workspace path]`
//! - `forge_trigger list`
//! - `forge_trigger runs`
//! - `forge_trigger replay --run-id <run-id>`
//! - `forge_trigger status`

use forge::gateway::client::GatewayClient;
use forge::gateway::protocol::{
    EnqueueTriggerParams, EnqueueTriggerResult, GatewayReply, GatewayRequest,
    ReplayTriggerRunParams, ReplayTriggerRunResult,
};
use forge::gateway::runner::TriggerRunRecord;
use forge::gateway::server::{default_socket_path, GatewayRuntimeStatus};
use forge::gateway::webhook::PendingTrigger;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerCommand {
    Enqueue,
    List,
    Runs,
    Replay,
    Status,
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
        TriggerCommand::Status => show_runtime_status(&mut client).await,
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
        "status" => TriggerCommand::Status,
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
            "--id" if parsed.command == TriggerCommand::Replay => {
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
    } else if parsed.command == TriggerCommand::Replay {
        if parsed.run_id.is_none() && positional_message.len() == 1 {
            parsed.run_id = positional_message.pop();
        } else if !positional_message.is_empty() {
            return Err("unexpected positional argument".to_string());
        }
        parsed.run_id = clean_optional(parsed.run_id);
        if parsed.run_id.is_none() {
            return Err("run_id is required for trigger replay".to_string());
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
        println!(
            "  {}  trigger={}  attempt={}  {}  {}",
            run.id,
            run.trigger_id,
            run.attempt,
            run.status,
            truncate(&run.message, 80)
        );
    }
    Ok(())
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

async fn show_runtime_status(client: &mut GatewayClient) -> Result<(), String> {
    let reply = send(client, "runtime_status", None).await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let status = serde_json::from_value::<GatewayRuntimeStatus>(response.result)
        .map_err(|error| format!("Gateway returned invalid runtime status: {error}"))?;

    println!("{}", status.message);
    println!("Uptime: {}s", status.uptime_seconds);
    println!("Active sessions: {}", status.active_sessions);
    println!("Pending triggers: {}", status.pending_triggers);
    println!("Claimed triggers: {}", status.claimed_triggers);
    println!("Dead-letter runs: {}", status.dead_letter_runs);
    Ok(())
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
    "Usage: forge_trigger <enqueue|list|runs|replay|status> [options]\n\
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
       --run-id, --id <id>     Trigger run id to replay"
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
}
