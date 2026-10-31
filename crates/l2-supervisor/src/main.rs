//! `l2-supervisor [path/to/supervisor.ini]`
//!
//! Runs until Ctrl+C (or SIGTERM on Unix) or an admin `StopSupervisor`, then stops the servers
//! it started. `supervisor.pid` next to the ini file names the running process.

use std::path::{Path, PathBuf};

use anyhow::Context;
use l2_supervisor::config::SupervisorConfig;
use l2_supervisor::{Running, pidfile, secret};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let path = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("supervisor.ini"), PathBuf::from);
    let mut config = SupervisorConfig::load(&path).with_context(|| format!("loading {}", path.display()))?;
    // D-56: no token to type. An empty `Token` means the one in `supervisor.token` (the GUI reads
    // the same file); an empty `AdminToken` means a fresh one per start, handed to that server.
    if config.token.is_empty() {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        config.token = secret::load_or_create(dir)
            .with_context(|| format!("creating {}", secret::token_path(dir).display()))?;
    }
    for server in &mut config.servers {
        if server.admin_endpoint.is_some() && server.admin_token.is_empty() {
            server.admin_token = secret::random_token();
        }
    }

    // Refuse to run twice on one supervisor.ini; the GUI relies on this file (G-01).
    let pid_path = pidfile::path_for(&path);
    if let Some(other) = pidfile::read(&pid_path)
        && other.pid != std::process::id()
        && other.is_alive()
    {
        anyhow::bail!(
            "a supervisor is already running (pid {}, {})",
            other.pid,
            other.exe
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
    // Written after the bind succeeded, so a failed start never touches another supervisor's file.
    let _pid_guard = match pidfile::PidFileGuard::create(&pid_path) {
        Ok(guard) => Some(guard),
        Err(e) => {
            tracing::warn!("cannot write {}: {e}", pid_path.display());
            None
        }
    };
    running.autostart().await;

    tokio::select! {
        () = wait_for_shutdown_signal() => {}
        () = running.stop_requested() => tracing::info!("stop requested through the admin API"),
    }
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
