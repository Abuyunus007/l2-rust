#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! The GUI's launcher against the real `l2-supervisor` binary: start detached from a temp
//! `supervisor.ini`, connect, never start a second one, stop it through the API.
//!
//! The port is chosen by the OS (bind port 0, remember, release); no real port is touched.

use std::path::Path;
use std::time::{Duration, Instant};

use l2_admin_api::{AdminClient, Request, Response};
use l2_supervisor::launcher::{LOG_PATH, LaunchStatus, ensure_running};
use l2_supervisor::pidfile;

const EXE: &str = env!("CARGO_BIN_EXE_l2-supervisor");
const WAIT: Duration = Duration::from_secs(30);

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Kills the supervisor by pid if the test fails before the API stop (never by name).
struct KillOnDrop(Option<u32>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let Some(pid) = self.0 else { return };
        if cfg!(windows) {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
        } else {
            let _ = std::process::Command::new("kill").arg(pid.to_string()).output();
        }
    }
}

async fn connect_with_retry(
    url: &str,
    token: &str,
) -> (
    AdminClient,
    tokio::sync::mpsc::UnboundedReceiver<l2_admin_api::Event>,
) {
    let deadline = Instant::now() + WAIT;
    loop {
        match AdminClient::connect(url, token).await {
            Ok(pair) => return pair,
            Err(e) => {
                assert!(Instant::now() < deadline, "supervisor never came up: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn launcher_starts_connects_never_doubles_and_stops() {
    let run = tempfile::tempdir().unwrap();
    let port = free_port();
    std::fs::write(
        run.path().join("supervisor.ini"),
        format!("Listen = 127.0.0.1:{port}\nToken = integration-test\nStopTimeoutSecs = 2\n"),
    )
    .unwrap();
    let pid_path = pidfile::path_for(&run.path().join("supervisor.ini"));

    // 1. Nobody listens: the launcher starts one.
    let launch = ensure_running(run.path(), Path::new(EXE));
    assert_eq!(launch.status, LaunchStatus::Starting);
    let connection = launch.connection.unwrap();
    assert_eq!(connection.url(), format!("ws://127.0.0.1:{port}"));

    let (client, _events) = connect_with_retry(&connection.url(), &connection.token).await;
    let first = pidfile::read(&pid_path).expect("pid file written at start");
    let _guard = KillOnDrop(Some(first.pid));
    assert!(first.is_alive());
    assert_ne!(first.pid, std::process::id());
    assert!(
        run.path().join(LOG_PATH).is_file(),
        "output goes to run/log/supervisor.log"
    );

    // 2. Now it listens: the next call connects and starts nothing.
    let again = ensure_running(run.path(), Path::new(EXE));
    assert_eq!(again.status, LaunchStatus::Running);
    assert_eq!(pidfile::read(&pid_path).unwrap().pid, first.pid);

    // 3. It answers requests, then stops cleanly on StopSupervisor and removes its pid file.
    assert_eq!(
        client.request(Request::ListServers).await.unwrap(),
        Response::Servers { servers: vec![] }
    );
    assert_eq!(
        client.request(Request::StopSupervisor).await.unwrap(),
        Response::Ok
    );
    eventually("the pid file to go away", || !pid_path.exists()).await;
    eventually("the process to exit", || !first.is_alive()).await;
}
