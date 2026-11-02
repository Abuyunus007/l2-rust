//! The supervisor: starts, stops and restarts the server processes, collects their output as log
//! lines and serves the admin API (`docs/admin-api.md`) to the GUI.
//!
//! The binary (`main.rs`) is a thin wrapper around [`Running::start`]; the library exists so the
//! integration tests can run a whole supervisor on an ephemeral port.

pub mod api;
pub mod config;
pub mod launcher;
pub mod logs;
pub mod pidfile;
pub mod process;
pub mod secret;
pub mod takeover;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use l2_admin_api::{Token, serve};
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::api::{SupervisorHandler, spawn_admin_link};
use crate::config::SupervisorConfig;
use crate::logs::LogStore;
use crate::process::Supervisor;

/// A supervisor that is listening for admin clients. Nothing is started yet:
/// call [`Running::autostart`] for the servers marked `Autostart`.
pub struct Running {
    pub config: SupervisorConfig,
    pub supervisor: Arc<Supervisor>,
    local_addr: SocketAddr,
    stop_requested: Arc<Notify>,
    tasks: Vec<JoinHandle<()>>,
}

impl Running {
    /// Binds the admin listener and creates the (idle) server actors.
    pub async fn start(config: SupervisorConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind(config.listen).await?;
        let local_addr = listener.local_addr()?;

        let logs = LogStore::new(config.log_buffer_lines);
        let supervisor = Supervisor::new(&config, logs);

        let mut tasks = Vec::new();
        let mut links = HashMap::new();
        for server in &config.servers {
            if let Some((link, task)) = spawn_admin_link(server, supervisor.event_sender()) {
                if let Some(handle) = supervisor.get(&server.name) {
                    let graceful = link.clone();
                    handle.set_graceful_stop(Arc::new(move || {
                        let link = graceful.clone();
                        Box::pin(async move { link.request_shutdown().await })
                    }));
                }
                links.insert(server.name.clone(), link);
                tasks.push(task);
            }
        }
        let stop_requested = Arc::new(Notify::new());
        let handler = SupervisorHandler::new(Arc::clone(&supervisor), links, Arc::clone(&stop_requested));
        tasks.push(tokio::spawn(serve(
            listener,
            "supervisor".into(),
            // D-56: on this machine only, an empty Token means no token at all.
            if config.token.is_empty() && config.listen.ip().is_loopback() {
                Token::local_only()
            } else {
                Token::new(config.token.clone())
            },
            handler,
        )));
        Ok(Self {
            config,
            supervisor,
            local_addr,
            stop_requested,
            tasks,
        })
    }

    /// The address the admin API listens on (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Completes once a client sent `StopSupervisor` and all servers are stopped.
    pub async fn stop_requested(&self) {
        self.stop_requested.notified().await;
    }

    pub async fn autostart(&self) {
        self.supervisor.autostart(&self.config).await;
    }

    /// Stops all servers gracefully, then the admin listener.
    pub async fn shutdown(self) {
        self.supervisor.shutdown().await;
        for task in self.tasks {
            task.abort();
        }
    }
}
