//! Forge Gateway — background daemon binary.
//!
//! Usage: `forge-gateway` (no arguments)
//!
//! Listens on:
//! - Unix socket at `~/.forge/gateway.sock` for IPC (JSON-line protocol)
//! - TCP `127.0.0.1:2021` for webhook/trigger ingestion (JSON-line)

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

    // Spawn webhook TCP listener in background.
    let webhook_state = state.trigger_store.clone();
    tokio::spawn(async move {
        if let Err(e) = forge::gateway::webhook::serve(webhook_state).await {
            log::error!("Webhook listener died: {e}");
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
        fallback_workspace,
    );

    // Block on the Unix socket listener.
    match forge::gateway::server::serve(state, socket_path).await {
        Ok(()) => log::info!("Gateway shut down cleanly"),
        Err(e) => {
            log::error!("Gateway fatal: {e}");
            std::process::exit(1);
        }
    }
}
