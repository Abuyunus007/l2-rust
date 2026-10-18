#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic
#![allow(dead_code)] // each test file uses a different part

use std::time::Duration;

use l2_admin_api::{LogLine, ServerState, ServerStatus};
use l2_supervisor::config::{ServerConfig, SupervisorConfig};
use l2_supervisor::logs::LogStore;
use l2_supervisor::process::ServerHandle;

pub const CHILD: &str = env!("CARGO_BIN_EXE_supervisor-test-child");
pub const WAIT: Duration = Duration::from_secs(15);

/// A server running the test child in `mode` (`args` are the arguments after the mode).
pub fn child_server(name: &str, args: &[&str]) -> ServerConfig {
    ServerConfig {
        name: name.to_string(),
        command: CHILD.to_string(),
        args: args.iter().map(ToString::to_string).collect(),
        working_dir: None,
        admin_endpoint: None,
        admin_token: String::new(),
        autostart: false,
        restart_on_exit_code: Some(2),
    }
}

pub fn config(servers: Vec<ServerConfig>) -> SupervisorConfig {
    SupervisorConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        token: "secret".into(),
        log_buffer_lines: 1000,
        stop_timeout: Duration::from_secs(2),
        restart_delay: Duration::ZERO,
        servers,
    }
}

/// Polls until `done` is true; panics with `what` after [`WAIT`].
pub async fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !done() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub async fn wait_state(handle: &ServerHandle, state: ServerState) -> ServerStatus {
    eventually(&format!("state {state:?}"), || handle.status().state == state).await;
    handle.status()
}

pub fn lines(logs: &LogStore, server: &str) -> Vec<LogLine> {
    logs.subscribe(Some(server), None).0
}

pub async fn wait_log(logs: &LogStore, server: &str, needle: &str) -> LogLine {
    let mut found = None;
    eventually(&format!("log line containing '{needle}'"), || {
        found = lines(logs, server)
            .into_iter()
            .find(|l| l.message.contains(needle));
        found.is_some()
    })
    .await;
    found.unwrap()
}
