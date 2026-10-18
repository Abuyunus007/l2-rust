//! `l2-supervisor [path/to/supervisor.ini]`
//!
//! Runs until Ctrl+C (or SIGTERM on Unix), then stops the servers it started.

use std::path::PathBuf;

use anyhow::Context;
use l2_supervisor::Running;
use l2_supervisor::config::SupervisorConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let path = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("supervisor.ini"), PathBuf::from);
    let config = SupervisorConfig::load(&path).with_context(|| format!("loading {}", path.display()))?;
    if config.token.is_empty() {
        tracing::warn!(
            "Token is empty in {}: every admin connection will be refused",
            path.display()
        );
    }

    let running = Running::start(config.clone())
        .await
        .with_context(|| format!("cannot listen on {}", config.listen))?;
    tracing::info!(
        "admin API on ws://{}, {} server(s) configured",
        running.local_addr(),
        config.servers.len()
    );
    running.autostart().await;

    wait_for_shutdown_signal().await;
    tracing::info!("shutting down, stopping servers");
    running.shutdown().await;
    Ok(())
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut term) = signal(SignalKind::terminate()) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
