//! Forge Trigger CLI - enqueue and inspect gateway triggers.
//!
//! Usage:
//! - `forge_trigger enqueue --message "run digest" [--profile id] [--provider name] [--model name] [--workspace path]`
//! - `forge_trigger list`
//! - `forge_trigger runs`
//! - `forge_trigger replay --run-id <run-id>`
//! - `forge_trigger status`
//! - `forge_trigger ownership-eligibility --session-id <id> --task-id <id>`
//! - `forge_trigger read-only-owner-diagnostics --task-id <id> [--session-id <id>] [--approved-by name|--dev-only-allow]`
//! - `forge_trigger clear-stale-session-input --input-id <id> [--reason text]`

use forge::gateway::client::{build_dashboard_snapshot_request, GatewayClient};
use forge::gateway::protocol::{
    ClearStaleSessionInputParams, ClearStaleSessionInputResult, EnqueueTriggerParams,
    EnqueueTriggerResult, GatewayOwnershipEligibilityDecision, GatewayOwnershipEligibilityParams,
    GatewayOwnershipEligibilityResult, GatewayOwnershipMode, GatewayReadOnlyOwnerDiagnosticsParams,
    GatewayReadOnlyOwnerDiagnosticsResult, GatewayReply, GatewayRequest, GetTriggerRunParams,
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
    OwnershipEligibility,
    ReadOnlyOwnerDiagnostics,
    ClearStaleSessionInput,
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
    session_id: Option<String>,
    input_id: Option<String>,
    reason: Option<String>,
    task_id: Option<String>,
    requested_ownership_mode: GatewayOwnershipMode,
    approved_by: Option<String>,
    dev_only_allow: bool,
    requested_at_ms: Option<u64>,
    expires_at_ms: Option<u64>,
    idempotency_key: Option<String>,
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
        TriggerCommand::OwnershipEligibility => {
            show_gateway_ownership_eligibility(&mut client, args).await
        }
        TriggerCommand::ReadOnlyOwnerDiagnostics => {
            show_gateway_read_only_owner_diagnostics(&mut client, args).await
        }
        TriggerCommand::ClearStaleSessionInput => {
            clear_stale_session_input(&mut client, args).await
        }
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
        "ownership-eligibility" | "eligibility" => TriggerCommand::OwnershipEligibility,
        "read-only-owner-diagnostics" | "read-only-owner" | "readonly-owner" => {
            TriggerCommand::ReadOnlyOwnerDiagnostics
        }
        "clear-stale-session-input" | "clear-stale-input" | "clear-stale-gateway-input" => {
            TriggerCommand::ClearStaleSessionInput
        }
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
        session_id: None,
        input_id: None,
        reason: None,
        task_id: None,
        requested_ownership_mode: GatewayOwnershipMode::GatewayReadOnlyOwner,
        approved_by: None,
        dev_only_allow: false,
        requested_at_ms: None,
        expires_at_ms: None,
        idempotency_key: None,
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
            "--session-id" => parsed.session_id = Some(next_value(&mut iter, "--session-id")?),
            "--input-id" => parsed.input_id = Some(next_value(&mut iter, "--input-id")?),
            "--reason" => parsed.reason = Some(next_value(&mut iter, "--reason")?),
            "--task-id" => parsed.task_id = Some(next_value(&mut iter, "--task-id")?),
            "--mode" | "--requested-mode" => {
                parsed.requested_ownership_mode =
                    parse_ownership_mode(&next_value(&mut iter, "--mode")?)?
            }
            "--approved-by" => parsed.approved_by = Some(next_value(&mut iter, "--approved-by")?),
            "--dev-only-allow" => parsed.dev_only_allow = true,
            "--requested-at-ms" => {
                parsed.requested_at_ms = Some(next_u64_value(&mut iter, "--requested-at-ms")?)
            }
            "--expires-at-ms" => {
                parsed.expires_at_ms = Some(next_u64_value(&mut iter, "--expires-at-ms")?)
            }
            "--idempotency-key" => {
                parsed.idempotency_key = Some(next_value(&mut iter, "--idempotency-key")?)
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
    } else if parsed.command == TriggerCommand::ReadOnlyOwnerDiagnostics {
        if !positional_message.is_empty() {
            return Err("unexpected positional argument".to_string());
        }
        parsed.task_id = clean_optional(parsed.task_id);
        if parsed.task_id.is_none() {
            return Err("task_id is required for read-only owner diagnostics".to_string());
        }
    } else if parsed.command == TriggerCommand::ClearStaleSessionInput {
        if parsed.input_id.is_none() && positional_message.len() == 1 {
            parsed.input_id = positional_message.pop();
        } else if !positional_message.is_empty() {
            return Err("unexpected positional argument".to_string());
        }
        parsed.input_id = clean_optional(parsed.input_id);
        if parsed.input_id.is_none() {
            return Err("input_id is required for clear stale session input".to_string());
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
    parsed.session_id = clean_optional(parsed.session_id);
    parsed.input_id = clean_optional(parsed.input_id);
    parsed.reason = clean_optional(parsed.reason);
    parsed.task_id = clean_optional(parsed.task_id);
    parsed.approved_by = clean_optional(parsed.approved_by);
    parsed.idempotency_key = clean_optional(parsed.idempotency_key);

    Ok(parsed)
}

fn parse_ownership_mode(value: &str) -> Result<GatewayOwnershipMode, String> {
    match value.trim() {
        "local_default" => Ok(GatewayOwnershipMode::LocalDefault),
        "gateway_opt_in" => Ok(GatewayOwnershipMode::GatewayOptIn),
        "gateway_opt_in_dry_run" => Ok(GatewayOwnershipMode::GatewayOptInDryRun),
        "gateway_read_only_owner" => Ok(GatewayOwnershipMode::GatewayReadOnlyOwner),
        "gateway_patch_proposal_owner" => Ok(GatewayOwnershipMode::GatewayPatchProposalOwner),
        "gateway_tool_owner_blocked_by_default" => {
            Ok(GatewayOwnershipMode::GatewayToolOwnerBlockedByDefault)
        }
        other => Err(format!("unknown gateway ownership mode: {other}")),
    }
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

fn next_u64_value<I>(iter: &mut I, flag: &str) -> Result<u64, String>
where
    I: Iterator<Item = String>,
{
    let value = next_value(iter, flag)?;
    value
        .trim()
        .parse::<u64>()
        .map_err(|error| format!("{flag} requires an unsigned integer: {error}"))
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

    for line in render_trigger_run_detail_lines(&result.run) {
        println!("{line}");
    }
    Ok(())
}

fn render_trigger_run_detail_lines(run: &TriggerRunRecord) -> Vec<String> {
    let mut lines = vec![
        format!("Trigger run {}", run.id),
        format!("  trigger: {}", run.trigger_id),
        format!("  session: {}", run.session_id.as_deref().unwrap_or("-")),
        format!("  status: {}", run.status),
        format!("  attempt: {}", run.attempt),
        format!("  started_at_ms: {}", run.started_at_ms),
        format!("  ended_at_ms: {}", run.ended_at_ms),
        format!(
            "  executor: {}",
            run.executor_kind.as_deref().unwrap_or("-")
        ),
        format!(
            "  failure_category: {}",
            run.failure_category.as_deref().unwrap_or("-")
        ),
        format!(
            "  lease_expires_at_ms: {}",
            run.lease_expires_at_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
        format!("  profile: {}", run.profile_id.as_deref().unwrap_or("-")),
        format!("  provider: {}", run.provider.as_deref().unwrap_or("-")),
        format!("  model: {}", run.model.as_deref().unwrap_or("-")),
        format!(
            "  workspace: {}",
            run.workspace_path.as_deref().unwrap_or("-")
        ),
        format!("  message: {}", run.message),
    ];
    if let Some(trigger_message) = run.trigger_message.as_deref() {
        lines.push(format!("  trigger_message: {trigger_message}"));
    }
    lines
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

async fn show_gateway_ownership_eligibility(
    client: &mut GatewayClient,
    args: ParsedTriggerArgs,
) -> Result<(), String> {
    let params = GatewayOwnershipEligibilityParams {
        session_id: args.session_id,
        task_id: args.task_id,
        requested_mode: args.requested_ownership_mode,
    };
    let params = serde_json::to_value(params)
        .map_err(|error| format!("Failed to encode ownership eligibility params: {error}"))?;
    let reply = send(
        client,
        "evaluate_gateway_ownership_eligibility",
        Some(params),
    )
    .await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let result = serde_json::from_value::<GatewayOwnershipEligibilityResult>(response.result)
        .map_err(|error| format!("Gateway returned invalid ownership eligibility: {error}"))?;

    for line in render_gateway_ownership_eligibility_lines(&result) {
        println!("{line}");
    }
    Ok(())
}

async fn show_gateway_read_only_owner_diagnostics(
    client: &mut GatewayClient,
    args: ParsedTriggerArgs,
) -> Result<(), String> {
    let params = GatewayReadOnlyOwnerDiagnosticsParams {
        task_id: args
            .task_id
            .ok_or_else(|| "task_id is required for read-only owner diagnostics".to_string())?,
        session_id: args.session_id,
        approved_by: args.approved_by,
        dev_only_allow: args.dev_only_allow,
        requested_at_ms: args.requested_at_ms,
        expires_at_ms: args.expires_at_ms,
        idempotency_key: args.idempotency_key,
    };
    let params = serde_json::to_value(params)
        .map_err(|error| format!("Failed to encode read-only owner params: {error}"))?;
    let reply = send(
        client,
        "run_gateway_read_only_owner_diagnostics",
        Some(params),
    )
    .await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let result = serde_json::from_value::<GatewayReadOnlyOwnerDiagnosticsResult>(response.result)
        .map_err(|error| {
        format!("Gateway returned invalid read-only owner diagnostics: {error}")
    })?;

    for line in render_gateway_read_only_owner_diagnostics_lines(&result) {
        println!("{line}");
    }
    Ok(())
}

async fn clear_stale_session_input(
    client: &mut GatewayClient,
    args: ParsedTriggerArgs,
) -> Result<(), String> {
    let params = ClearStaleSessionInputParams {
        input_id: args
            .input_id
            .ok_or_else(|| "input_id is required for clear stale session input".to_string())?,
        reason: args.reason,
    };
    let params = serde_json::to_value(params)
        .map_err(|error| format!("Failed to encode clear stale input params: {error}"))?;
    let reply = send(client, "clear_stale_session_input", Some(params)).await?;
    let GatewayReply::Ok(response) = reply else {
        return Err(render_gateway_error(reply));
    };
    let result = serde_json::from_value::<ClearStaleSessionInputResult>(response.result)
        .map_err(|error| format!("Gateway returned invalid clear stale input result: {error}"))?;

    for line in render_clear_stale_session_input_lines(&result) {
        println!("{line}");
    }
    Ok(())
}

fn render_clear_stale_session_input_lines(result: &ClearStaleSessionInputResult) -> Vec<String> {
    let mut lines = vec![
        if result.cleared {
            format!("Cleared stale session input {}.", result.input_id)
        } else {
            format!("No queued session input matched {}.", result.input_id)
        },
        format!("Pending session inputs: {}.", result.pending_inputs),
    ];

    if let Some(evidence) = result.evidence.as_ref() {
        let mut line = format!(
            "Evidence: session={} action={} completed={}",
            evidence.session_id,
            session_input_action_label(evidence.action),
            evidence.completed_at_ms
        );
        if let Some(reason) = evidence
            .reason
            .as_deref()
            .filter(|reason| !reason.is_empty())
        {
            line.push_str(&format!(" reason={}", truncate(reason, 120)));
        }
        lines.push(line);
    }

    lines
}

fn render_runtime_status_lines(status: &GatewayRuntimeStatus) -> Vec<String> {
    let mut lines = vec![
        status.message.clone(),
        format!(
            "Ownership: {} default={} can_own_sessions={} parity={} recovery={}",
            render_ownership_mode(status.ownership.ownership_mode),
            status.ownership.gateway_default_enabled,
            status.ownership.gateway_can_own_sessions,
            status.ownership.parity_gate,
            status.ownership.recovery_gate
        ),
        format!(
            "Runtime health: ok={} replay={} tasks={} recovery={}",
            status.runtime_health.ok,
            status.runtime_health.last_replay.message,
            status.runtime_health.last_replay.task_count,
            status
                .runtime_health
                .last_recovery_action
                .as_ref()
                .map(|action| action.task_id.as_str())
                .unwrap_or("none")
        ),
        format!("Uptime: {}s", status.uptime_seconds),
        format!("Active sessions: {}", status.active_sessions),
        format!("Pending triggers: {}", status.pending_triggers),
        format!("Pending session inputs: {}", status.pending_session_inputs),
        format!("Loop runner: {}", status.loop_runner),
        format!("Pending loop tasks: {}", status.pending_loop_tasks),
        format!("Running loop tasks: {}", status.running_loop_tasks),
        format!("Stale loop task leases: {}", status.stale_loop_task_leases),
        format!("Orphaned loop tasks: {}", status.orphaned_loop_tasks),
        format!("Interrupted loop tasks: {}", status.interrupted_loop_tasks),
        format!("Recoverable loop tasks: {}", status.recoverable_loop_tasks),
        format!("Dry-run owner runs: {}", status.dry_run_headless_owner_runs),
        format!("Waiting owner runs: {}", status.waiting_headless_owner_runs),
        format!("Denied owner runs: {}", status.denied_headless_owner_runs),
        format!("Expired owner runs: {}", status.expired_headless_owner_runs),
        format!("Claimed triggers: {}", status.claimed_triggers),
        format!("Dead-letter runs: {}", status.dead_letter_runs),
    ];
    if status.degraded_mode.active {
        lines.push(format!(
            "Degraded: {} (fallback: {}, recovery: {})",
            status.degraded_mode.reason,
            status.degraded_mode.fallback,
            status.degraded_mode.recovery_command
        ));
    }
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
            let mut line = format!(
                "  {}  session={}  action={}  completed={}  {}",
                input.input_id,
                input.session_id,
                session_input_action_label(input.action),
                input.completed_at_ms,
                truncate(&input.message_preview, 80)
            );
            if let Some(reason) = input.reason.as_deref().filter(|reason| !reason.is_empty()) {
                line.push_str(&format!("  reason={}", truncate(reason, 80)));
            }
            lines.push(line);
        }
    }
    lines
}

fn session_input_action_label(
    action: forge::gateway::session_input::SessionInputCompletionAction,
) -> &'static str {
    match action {
        forge::gateway::session_input::SessionInputCompletionAction::Accepted => "accepted",
        forge::gateway::session_input::SessionInputCompletionAction::ClearedStale => {
            "cleared_stale"
        }
    }
}

fn render_ownership_mode(mode: GatewayOwnershipMode) -> &'static str {
    match mode {
        GatewayOwnershipMode::LocalDefault => "local_default",
        GatewayOwnershipMode::GatewayOptIn => "gateway_opt_in",
        GatewayOwnershipMode::GatewayOptInDryRun => "gateway_opt_in_dry_run",
        GatewayOwnershipMode::GatewayReadOnlyOwner => "gateway_read_only_owner",
        GatewayOwnershipMode::GatewayPatchProposalOwner => "gateway_patch_proposal_owner",
        GatewayOwnershipMode::GatewayToolOwnerBlockedByDefault => {
            "gateway_tool_owner_blocked_by_default"
        }
    }
}

fn render_ownership_eligibility_decision(
    decision: GatewayOwnershipEligibilityDecision,
) -> &'static str {
    match decision {
        GatewayOwnershipEligibilityDecision::Allow => "allow",
        GatewayOwnershipEligibilityDecision::Deny => "deny",
        GatewayOwnershipEligibilityDecision::RequiresHumanApproval => "requires_human_approval",
    }
}

fn render_gateway_ownership_eligibility_lines(
    result: &GatewayOwnershipEligibilityResult,
) -> Vec<String> {
    vec![
        format!(
            "Gateway ownership eligibility: {}",
            render_ownership_eligibility_decision(result.decision)
        ),
        format!(
            "Requested mode: {}",
            render_ownership_mode(result.requested_mode)
        ),
        format!(
            "Session: {}",
            result.session_id.as_deref().unwrap_or("unknown")
        ),
        format!("Task: {}", result.task_id.as_deref().unwrap_or("unknown")),
        format!("Reasons: {}", render_joined_or_none(&result.reasons)),
        format!(
            "Missing evidence: {}",
            render_joined_or_none(&result.missing_evidence)
        ),
        format!(
            "Side effects: provider={} tools={} write_files={} task_state={}",
            result.would_execute_provider,
            result.would_execute_tools,
            result.would_write_files,
            result.changes_task_state
        ),
        format!(
            "Patch proposal: proposal_only={} generate={} apply={}",
            result.proposal_only, result.would_generate_patch_proposal, result.would_apply_patch
        ),
        format!("Required action: {}", result.required_action),
    ]
}

fn render_gateway_read_only_owner_diagnostics_lines(
    result: &GatewayReadOnlyOwnerDiagnosticsResult,
) -> Vec<String> {
    let status = if result.completed {
        "completed"
    } else if result.started {
        "started"
    } else if !result.ok {
        "denied"
    } else {
        "not_started"
    };
    let owner_run_id = result
        .owner_run
        .as_ref()
        .map(|run| run.owner_run_id.as_str())
        .unwrap_or("none");
    let session_id = result
        .owner_run
        .as_ref()
        .and_then(|run| run.session_id.as_deref())
        .or(result.task.session_id.as_deref())
        .unwrap_or("unknown");

    vec![
        format!("Gateway read-only owner diagnostics: {status}"),
        format!("Task: {}", result.task.id),
        format!("Session: {session_id}"),
        format!("Owner run: {owner_run_id}"),
        format!("Gateway can resume: {}", result.gateway_can_resume),
        format!(
            "Side effects: provider={} tools={} shell={} write_files={} confirmations={} commits={}",
            result.side_effects.provider,
            result.side_effects.tools,
            result.side_effects.shell,
            result.side_effects.write_files,
            result.side_effects.confirmations,
            result.side_effects.commits
        ),
        format!("Summary: {}", result.summary),
        format!("Message: {}", result.message),
    ]
}

fn render_joined_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
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
    "Usage: forge_trigger <enqueue|list|runs|replay|show|status|dashboard|ownership-eligibility|read-only-owner-diagnostics|clear-stale-session-input> [options]\n\
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
       --run-id, --id <id>     Trigger run id to inspect\n\
     \n\
     ownership-eligibility options:\n\
       --session-id <id>       Optional session id to evaluate\n\
       --task-id <id>          Optional loop task id to evaluate\n\
       --mode <mode>           Requested owner mode, default gateway_read_only_owner\n\
                               Modes: local_default, gateway_opt_in, gateway_opt_in_dry_run,\n\
                               gateway_read_only_owner, gateway_patch_proposal_owner,\n\
                               gateway_tool_owner_blocked_by_default\n\
     \n\
     read-only-owner-diagnostics options:\n\
       --task-id <id>          Loop task id to inspect\n\
       --session-id <id>       Optional desktop session id\n\
       --approved-by <name>    Human approval label\n\
       --dev-only-allow        Dev-only explicit allow for local tests\n\
       --requested-at-ms <n>   Optional deterministic request timestamp\n\
       --expires-at-ms <n>     Optional deterministic lease expiry timestamp\n\
       --idempotency-key <id>  Optional idempotency key\n\
     \n\
     clear-stale-session-input options:\n\
       --input-id <id>         Queued gateway session input id to clear\n\
       --reason <text>         Optional operator recovery reason"
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
    fn parse_ownership_eligibility_collects_context_and_mode() {
        let parsed = super::parse_trigger_args([
            "ownership-eligibility",
            "--session-id",
            " session-1 ",
            "--task-id",
            " task-1 ",
            "--mode",
            "gateway_read_only_owner",
        ])
        .expect("parse");

        assert_eq!(parsed.command, super::TriggerCommand::OwnershipEligibility);
        assert_eq!(parsed.session_id.as_deref(), Some("session-1"));
        assert_eq!(parsed.task_id.as_deref(), Some("task-1"));
        assert_eq!(
            parsed.requested_ownership_mode,
            forge::gateway::protocol::GatewayOwnershipMode::GatewayReadOnlyOwner
        );
    }

    #[test]
    fn parse_ownership_eligibility_accepts_patch_proposal_owner_mode() {
        let parsed = super::parse_trigger_args([
            "ownership-eligibility",
            "--session-id",
            "session-1",
            "--task-id",
            "loop-1",
            "--mode",
            "gateway_patch_proposal_owner",
        ])
        .expect("parse");

        assert_eq!(
            super::render_ownership_mode(parsed.requested_ownership_mode),
            "gateway_patch_proposal_owner"
        );
    }

    #[test]
    fn parse_read_only_owner_diagnostics_collects_explicit_allow_and_timing() {
        let parsed = super::parse_trigger_args([
            "read-only-owner-diagnostics",
            "--task-id",
            " loop-readonly ",
            "--session-id",
            " desktop-session-1 ",
            "--dev-only-allow",
            "--requested-at-ms",
            "10",
            "--expires-at-ms",
            "60010",
            "--idempotency-key",
            " readonly:loop-readonly ",
        ])
        .expect("parse");

        assert_eq!(
            parsed.command,
            super::TriggerCommand::ReadOnlyOwnerDiagnostics
        );
        assert_eq!(parsed.task_id.as_deref(), Some("loop-readonly"));
        assert_eq!(parsed.session_id.as_deref(), Some("desktop-session-1"));
        assert!(parsed.dev_only_allow);
        assert_eq!(parsed.requested_at_ms, Some(10));
        assert_eq!(parsed.expires_at_ms, Some(60010));
        assert_eq!(
            parsed.idempotency_key.as_deref(),
            Some("readonly:loop-readonly")
        );
    }

    #[test]
    fn parse_read_only_owner_diagnostics_requires_task_id() {
        let err = super::parse_trigger_args(["read-only-owner-diagnostics", "--dev-only-allow"])
            .expect_err("missing task id");

        assert!(err.contains("task_id is required"));
    }

    #[test]
    fn parse_clear_stale_session_input_collects_input_id_and_reason() {
        let parsed = super::parse_trigger_args([
            "clear-stale-session-input",
            "--input-id",
            " input-stale ",
            "--reason",
            " operator cleared duplicate gateway input ",
        ])
        .expect("parse");

        assert_eq!(
            parsed.command,
            super::TriggerCommand::ClearStaleSessionInput
        );
        assert_eq!(parsed.input_id.as_deref(), Some("input-stale"));
        assert_eq!(
            parsed.reason.as_deref(),
            Some("operator cleared duplicate gateway input")
        );
    }

    #[test]
    fn render_clear_stale_session_input_lines_include_recovery_evidence() {
        let result = forge::gateway::protocol::ClearStaleSessionInputResult {
            ok: true,
            input_id: "input-stale".into(),
            cleared: true,
            pending_inputs: 0,
            evidence: Some(
                forge::gateway::session_input::SessionInputCompletionRecord {
                    input_id: "input-stale".into(),
                    session_id: "session-1".into(),
                    message_preview: "continue".into(),
                    received_at_ms: 10,
                    completed_at_ms: 20,
                    action:
                        forge::gateway::session_input::SessionInputCompletionAction::ClearedStale,
                    reason: Some("operator cleared duplicate gateway input".into()),
                },
            ),
        };

        let lines = super::render_clear_stale_session_input_lines(&result);

        assert_eq!(
            lines,
            vec![
                "Cleared stale session input input-stale.".to_string(),
                "Pending session inputs: 0.".to_string(),
                "Evidence: session=session-1 action=cleared_stale completed=20 reason=operator cleared duplicate gateway input".to_string(),
            ]
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
            executor_kind: Some("eval_headless".into()),
            failure_category: None,
            lease_expires_at_ms: Some(300_010),
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
    fn render_trigger_run_detail_lines_include_runtime_evidence() {
        let run = super::TriggerRunRecord {
            id: "run-detail".into(),
            trigger_id: "trigger-detail".into(),
            session_id: Some("gateway-session-1".into()),
            attempt: 3,
            status: "dead_letter".into(),
            message: "provider offline".into(),
            started_at_ms: 10,
            ended_at_ms: 20,
            executor_kind: Some("eval_headless".into()),
            failure_category: Some("runner_error".into()),
            lease_expires_at_ms: Some(300_010),
            trigger_message: Some("run digest".into()),
            profile_id: Some("ops".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5".into()),
            workspace_path: Some("/repo".into()),
        };

        let lines = super::render_trigger_run_detail_lines(&run);

        assert!(lines.contains(&"  executor: eval_headless".to_string()));
        assert!(lines.contains(&"  failure_category: runner_error".to_string()));
        assert!(lines.contains(&"  lease_expires_at_ms: 300010".to_string()));
    }

    #[test]
    fn render_runtime_status_lines_include_recent_session_inputs() {
        let status = super::GatewayRuntimeStatus {
            ok: true,
            message: "Gateway runtime is reachable.".into(),
            ownership: forge::gateway::protocol::default_gateway_ownership_capability(),
            degraded_mode: forge::gateway::protocol::default_gateway_degraded_mode_status(),
            runtime_health: forge::loop_runtime::default_runtime_health_snapshot(),
            uptime_seconds: 42,
            active_sessions: 1,
            pending_triggers: 0,
            pending_session_inputs: 0,
            loop_runner: "started".into(),
            pending_loop_tasks: 2,
            running_loop_tasks: 1,
            stale_loop_task_leases: 0,
            orphaned_loop_tasks: 1,
            interrupted_loop_tasks: 0,
            recoverable_loop_tasks: 1,
            dry_run_headless_owner_runs: 3,
            waiting_headless_owner_runs: 2,
            denied_headless_owner_runs: 1,
            expired_headless_owner_runs: 0,
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
                    action: forge::gateway::session_input::SessionInputCompletionAction::Accepted,
                    reason: None,
                },
                forge::gateway::session_input::SessionInputCompletionRecord {
                    input_id: "input-stale".into(),
                    session_id: "session-1".into(),
                    message_preview: "stale continuation".into(),
                    received_at_ms: 11,
                    completed_at_ms: 21,
                    action:
                        forge::gateway::session_input::SessionInputCompletionAction::ClearedStale,
                    reason: Some("operator cleared stale queued input".into()),
                },
            ],
            runtime_tasks: Vec::new(),
        };

        let lines = super::render_runtime_status_lines(&status);

        assert!(lines.contains(
            &"Ownership: local_default default=false can_own_sessions=false parity=pending recovery=pending"
                .to_string()
        ));
        assert!(lines.contains(&"Loop runner: started".to_string()));
        assert!(lines.contains(&"Pending loop tasks: 2".to_string()));
        assert!(lines.contains(&"Running loop tasks: 1".to_string()));
        assert!(lines.contains(&"Orphaned loop tasks: 1".to_string()));
        assert!(lines.contains(&"Recoverable loop tasks: 1".to_string()));
        assert!(lines.contains(&"Dry-run owner runs: 3".to_string()));
        assert!(lines.contains(&"Waiting owner runs: 2".to_string()));
        assert!(lines.contains(&"Denied owner runs: 1".to_string()));
        assert!(lines.contains(&"Expired owner runs: 0".to_string()));
        assert!(lines.contains(&"Recent session inputs:".to_string()));
        assert!(lines.contains(
            &"  input-1  session=session-1  action=accepted  completed=20  continue".to_string()
        ));
        assert!(lines.contains(
            &"  input-stale  session=session-1  action=cleared_stale  completed=21  stale continuation  reason=operator cleared stale queued input"
                .to_string()
        ));
    }

    #[test]
    fn render_runtime_status_lines_include_degraded_recovery_command() {
        let status = super::GatewayRuntimeStatus {
            ok: true,
            message: "Gateway runtime is reachable, but degraded mode is active.".into(),
            ownership: forge::gateway::protocol::default_gateway_ownership_capability(),
            degraded_mode: forge::gateway::protocol::GatewayDegradedModeStatus {
                active: true,
                reason: "runtime task 'webhook_listener' failed: address already in use".into(),
                fallback: "desktop_runtime".into(),
                input_policy:
                    "Queued session input stays pending until the owning desktop runtime accepts it."
                        .into(),
                confirmation_policy:
                    "Pending confirmations stay with the owning desktop runtime.".into(),
                recovery_command: "forge service restart".into(),
            },
            runtime_health: forge::loop_runtime::default_runtime_health_snapshot(),
            uptime_seconds: 42,
            active_sessions: 1,
            pending_triggers: 0,
            pending_session_inputs: 0,
            loop_runner: "started".into(),
            pending_loop_tasks: 0,
            running_loop_tasks: 0,
            stale_loop_task_leases: 0,
            orphaned_loop_tasks: 0,
            interrupted_loop_tasks: 0,
            recoverable_loop_tasks: 0,
            dry_run_headless_owner_runs: 0,
            waiting_headless_owner_runs: 0,
            denied_headless_owner_runs: 0,
            expired_headless_owner_runs: 0,
            claimed_triggers: 0,
            dead_letter_runs: 0,
            recent_runs: Vec::new(),
            recent_session_inputs: Vec::new(),
            runtime_tasks: Vec::new(),
        };
        let lines = super::render_runtime_status_lines(&status);

        assert!(lines.contains(
            &"Degraded: runtime task 'webhook_listener' failed: address already in use (fallback: desktop_runtime, recovery: forge service restart)"
                .to_string()
        ));
    }

    #[test]
    fn render_dashboard_snapshot_lines_include_core_operational_sections() {
        let snapshot = super::GatewayDashboardSnapshot {
            ok: true,
            generated_at_ms: 100,
            status: super::GatewayRuntimeStatus {
                ok: true,
                message: "Gateway runtime is reachable.".into(),
                ownership: forge::gateway::protocol::default_gateway_ownership_capability(),
                degraded_mode: forge::gateway::protocol::default_gateway_degraded_mode_status(),
                runtime_health: forge::loop_runtime::default_runtime_health_snapshot(),
                uptime_seconds: 42,
                active_sessions: 1,
                pending_triggers: 1,
                pending_session_inputs: 0,
                loop_runner: "stopped".into(),
                pending_loop_tasks: 0,
                running_loop_tasks: 0,
                stale_loop_task_leases: 0,
                orphaned_loop_tasks: 0,
                interrupted_loop_tasks: 0,
                recoverable_loop_tasks: 0,
                dry_run_headless_owner_runs: 0,
                waiting_headless_owner_runs: 0,
                denied_headless_owner_runs: 0,
                expired_headless_owner_runs: 0,
                claimed_triggers: 1,
                dead_letter_runs: 0,
                recent_runs: Vec::new(),
                recent_session_inputs: Vec::new(),
                runtime_tasks: Vec::new(),
            },
            loop_tasks: Vec::new(),
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

    #[test]
    fn render_gateway_ownership_eligibility_lines_explain_dry_run_denial() {
        let result = forge::gateway::protocol::GatewayOwnershipEligibilityResult {
            ok: true,
            decision: forge::gateway::protocol::GatewayOwnershipEligibilityDecision::Deny,
            requested_mode: forge::gateway::protocol::GatewayOwnershipMode::GatewayReadOnlyOwner,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            reasons: vec![
                "gateway_ownership_disabled".into(),
                "read_only_owner_requires_explicit_approval".into(),
            ],
            missing_evidence: vec!["memory_recall_audit".into(), "context_capsule".into()],
            required_action:
                "Keep the desktop runtime as owner; resolve the listed gateway eligibility gaps first."
                    .into(),
            proposal_only: false,
            would_generate_patch_proposal: false,
            would_apply_patch: false,
            would_execute_provider: false,
            would_execute_tools: false,
            would_write_files: false,
            changes_task_state: false,
        };

        let lines = super::render_gateway_ownership_eligibility_lines(&result);

        assert!(lines.contains(&"Gateway ownership eligibility: deny".to_string()));
        assert!(lines.contains(&"Requested mode: gateway_read_only_owner".to_string()));
        assert!(lines.contains(&"Session: session-1".to_string()));
        assert!(lines.contains(&"Task: task-1".to_string()));
        assert!(lines.contains(
            &"Reasons: gateway_ownership_disabled, read_only_owner_requires_explicit_approval"
                .to_string()
        ));
        assert!(
            lines.contains(&"Missing evidence: memory_recall_audit, context_capsule".to_string())
        );
        assert!(lines.contains(
            &"Side effects: provider=false tools=false write_files=false task_state=false"
                .to_string()
        ));
        assert!(lines.contains(
            &"Patch proposal: proposal_only=false generate=false apply=false".to_string()
        ));
    }

    #[test]
    fn render_gateway_ownership_eligibility_lines_show_patch_proposal_policy() {
        let result = forge::gateway::protocol::GatewayOwnershipEligibilityResult {
            ok: true,
            decision: forge::gateway::protocol::GatewayOwnershipEligibilityDecision::Deny,
            requested_mode:
                forge::gateway::protocol::GatewayOwnershipMode::GatewayPatchProposalOwner,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            reasons: vec!["patch_proposal_owner_requires_gate".into()],
            missing_evidence: vec![
                "patch_proposal_review_gate".into(),
                "diff_evidence_contract".into(),
            ],
            required_action: "Patch proposals require review; direct apply stays blocked.".into(),
            proposal_only: true,
            would_generate_patch_proposal: true,
            would_apply_patch: false,
            would_execute_provider: false,
            would_execute_tools: false,
            would_write_files: false,
            changes_task_state: false,
        };

        let lines = super::render_gateway_ownership_eligibility_lines(&result);

        assert!(lines.contains(&"Requested mode: gateway_patch_proposal_owner".to_string()));
        assert!(lines
            .contains(&"Patch proposal: proposal_only=true generate=true apply=false".to_string()));
        assert!(lines.contains(
            &"Side effects: provider=false tools=false write_files=false task_state=false"
                .to_string()
        ));
    }

    #[test]
    fn render_gateway_read_only_owner_diagnostics_lines_show_side_effects() {
        let mut task = forge::loop_runtime::LoopTaskRecord::new(
            "summarize projection".into(),
            Some("desktop-session-1".into()),
            None,
            None,
            None,
            None,
            None,
        );
        task.id = "loop-readonly".into();
        let result = forge::gateway::protocol::GatewayReadOnlyOwnerDiagnosticsResult {
            ok: true,
            started: true,
            completed: true,
            gateway_can_resume: true,
            task,
            owner_run: Some(forge::loop_runtime::HeadlessOwnerRun {
                owner_run_id: "gateway-readonly-owner:loop-readonly:1".into(),
                task_id: "loop-readonly".into(),
                session_id: Some("desktop-session-1".into()),
                lease_id: "lease-loop-readonly".into(),
                attempt: 1,
                state: forge::loop_runtime::HeadlessOwnerRunState::Completed,
                snapshot_source:
                    forge::loop_runtime::HeadlessOwnerSnapshotSource::CurrentDesktopSession,
                snapshot_ref: Some("loop-projection".into()),
                human_gate_id: "human-gate-loop-readonly".into(),
                policy_decision_id: "policy-loop-readonly".into(),
                budget_snapshot_id: "budget-loop-readonly".into(),
                idempotency_key: "readonly:loop-readonly".into(),
                correlation_id: "corr-loop-readonly".into(),
                causation_id: None,
                requested_by: "forge_trigger".into(),
                requested_at_ms: 10,
                heartbeat_at_ms: Some(11),
                expires_at_ms: 60010,
                cancellation_reason: None,
                waiting_reason: None,
                executor_kind: forge::loop_runtime::HeadlessOwnerExecutorKind::DryRun,
                evidence_refs: vec!["gateway_read_only_diagnostics".into()],
            }),
            summary: "Read-only diagnostics completed without side effects.".into(),
            message: "Gateway read-only diagnostics owner completed.".into(),
            side_effects: forge::gateway::protocol::GatewayReadOnlyOwnerSideEffects {
                provider: false,
                tools: false,
                shell: false,
                write_files: false,
                confirmations: false,
                commits: false,
            },
        };

        let lines = super::render_gateway_read_only_owner_diagnostics_lines(&result);

        assert!(lines.contains(&"Gateway read-only owner diagnostics: completed".to_string()));
        assert!(lines.contains(&"Task: loop-readonly".to_string()));
        assert!(lines.contains(&"Session: desktop-session-1".to_string()));
        assert!(lines.contains(&"Owner run: gateway-readonly-owner:loop-readonly:1".to_string()));
        assert!(lines.contains(&"Gateway can resume: true".to_string()));
        assert!(lines.contains(
            &"Side effects: provider=false tools=false shell=false write_files=false confirmations=false commits=false"
                .to_string()
        ));
    }
}
