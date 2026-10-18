//! The supervisor: starts, stops and restarts the server processes, collects their output as log
//! lines and serves the admin API (`docs/admin-api.md`) to the GUI.
//!
//! The binary (`main.rs`) is a thin wrapper around [`Running::start`]; the library exists so the
//! integration tests can run a whole supervisor on an ephemeral port.

pub mod api;
pub mod config;
pub mod logs;
pub mod process;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use l2_admin_api::{Token, serve};
use tokio::net::TcpListener;
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
                links.insert(server.name.clone(), link);
                tasks.push(task);
            }
        }
        let handler = SupervisorHandler::new(Arc::clone(&supervisor), links);
        tasks.push(tokio::spawn(serve(
            listener,
            "supervisor".into(),
            Token::new(config.token.clone()),
            handler,
        )));
        Ok(Self {
            config,
            supervisor,
            local_addr,
            tasks,
        })
    }

    /// The address the admin API listens on (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
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
