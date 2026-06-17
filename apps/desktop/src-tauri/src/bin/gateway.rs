//! Forge Gateway — background daemon binary.
//!
//! Usage: `forge-gateway` (no arguments)
//!
//! Listens on:
//! - Unix socket at `~/.forge/gateway.sock` for IPC (JSON-line protocol)
//! - TCP `127.0.0.1:2021` for webhook/trigger ingestion (JSON-line)
//! - HTTP `127.0.0.1:2022` for the read-only gateway dashboard

use std::sync::Arc;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let state = Arc::new(forge::gateway::server::GatewayState::new());
    let socket_path = forge::gateway::server::default_socket_path();

    log::info!(
        "Forge Gateway v{} starting on {} (webhook :{})",
        forge::gateway::protocol::GATEWAY_VERSION,
        socket_path.display(),
        forge::gateway::webhook::WEBHOOK_PORT,
    );

    // Spawn local read-only HTTP dashboard in background.
    let dashboard_runtime_state = state.clone();
    state.mark_runtime_task_started(forge::gateway::server::DASHBOARD_HTTP_TASK);
    tokio::spawn(async move {
        if let Err(e) = forge::gateway::dashboard::serve(dashboard_runtime_state.clone()).await {
            log::error!("Dashboard listener died: {e}");
            dashboard_runtime_state
                .mark_runtime_task_failed(forge::gateway::server::DASHBOARD_HTTP_TASK, e);
        }
    });

    // Spawn webhook TCP listener in background.
    let webhook_state = state.trigger_store.clone();
    let webhook_runtime_state = state.clone();
    state.mark_runtime_task_started(forge::gateway::server::WEBHOOK_LISTENER_TASK);
    tokio::spawn(async move {
        if let Err(e) = forge::gateway::webhook::serve(webhook_state).await {
            log::error!("Webhook listener died: {e}");
            webhook_runtime_state
                .mark_runtime_task_failed(forge::gateway::server::WEBHOOK_LISTENER_TASK, e);
        }
    });

    // Spawn trigger consumer in background. The runner converts queued
    // webhook/scheduler triggers into headless Forge requests.
    let trigger_runner_state = state.trigger_store.clone();
    let trigger_run_state = state.trigger_run_store.clone();
    let fallback_workspace =
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    forge::gateway::runner::spawn_trigger_runner(
        trigger_runner_state,
        trigger_run_state,
        fallback_workspace.clone(),
    );
    state.mark_runtime_task_started(forge::gateway::server::TRIGGER_RUNNER_TASK);

    // Spawn loop runtime runner. The first MVP only claims durable loop-task
    // leases and records waiting/interrupted states; it does not create a
    // headless AgentSession or resume side effects.
    let loop_runner_journal = state.loop_event_journal.clone();
    let loop_runner_projection = state.loop_task_projection_store.clone();
    let loop_runner_runtime_state = state.clone();
    state.mark_runtime_task_started(forge::gateway::server::LOOP_RUNNER_TASK);
    tokio::spawn(async move {
        if let Err(e) = forge::loop_runtime::runner::serve_loop_runner(
            loop_runner_journal,
            loop_runner_projection,
        )
        .await
        {
            log::error!("Loop runner died: {e}");
            loop_runner_runtime_state
                .mark_runtime_task_failed(forge::gateway::server::LOOP_RUNNER_TASK, e);
        }
    });

    // Spawn scheduler tick in background. Due tasks are queued into the same
    // trigger store, then picked up by the trigger runner above.
    let scheduler_store = Arc::new(forge::scheduler::SchedulerStore::new(
        forge::scheduler::SchedulerStore::default_path(),
    ));
    forge::scheduler::spawn_scheduler_tick(
        scheduler_store,
        state.trigger_store.clone(),
        fallback_workspace,
    );
    state.mark_runtime_task_started(forge::gateway::server::SCHEDULER_TICK_TASK);

    // Block on the Unix socket listener.
    match forge::gateway::server::serve(state, socket_path).await {
        Ok(()) => log::info!("Gateway shut down cleanly"),
        Err(e) => {
            log::error!("Gateway fatal: {e}");
            std::process::exit(1);
        }
    }
}
