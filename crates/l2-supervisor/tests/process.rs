#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! Process control against the tiny `supervisor-test-child` binary.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use l2_admin_api::{LogLevel, ServerState};
use l2_supervisor::logs::LogStore;
use l2_supervisor::process::Supervisor;

fn setup(servers: Vec<l2_supervisor::config::ServerConfig>) -> (Arc<Supervisor>, Arc<LogStore>) {
    let cfg = config(servers);
    let logs = LogStore::new(cfg.log_buffer_lines);
    (Supervisor::new(&cfg, Arc::clone(&logs)), logs)
}

#[tokio::test]
async fn exit_zero_means_stopped() {
    let (sup, _logs) = setup(vec![child_server("s", &["exit", "0"])]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    let status = wait_state(s, ServerState::Stopped).await;
    assert_eq!(status.last_exit_code, Some(0));
    assert_eq!(status.pid, None);
}

#[tokio::test]
async fn exit_one_means_crashed_and_is_not_restarted() {
    let (sup, logs) = setup(vec![child_server("s", &["exit", "1"])]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    let status = wait_state(s, ServerState::Crashed).await;
    assert_eq!(status.last_exit_code, Some(1));
    wait_log(&logs, "s", "crashed, exit code 1").await;
    // Stays crashed: nothing restarts it.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(s.status().state, ServerState::Crashed);
    // A crashed server can be started by hand.
    s.start().await.unwrap();
    wait_state(s, ServerState::Crashed).await;
}

#[tokio::test]
async fn exit_two_restarts_the_process() {
    let marker = tempfile::tempdir().unwrap();
    let marker_file = marker.path().join("ran").to_string_lossy().into_owned();
    let (sup, logs) = setup(vec![child_server("s", &["restart-once", &marker_file])]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    // First run exits with 2, the second run prints and keeps running.
    wait_log(&logs, "s", "run again").await;
    let status = wait_state(s, ServerState::Running).await;
    assert_eq!(status.last_exit_code, Some(2));
    assert!(status.pid.is_some());
    wait_log(&logs, "s", "exited with code 2, restarting").await;
    s.stop().await.unwrap();
}

#[tokio::test]
async fn no_restart_when_the_code_is_not_configured() {
    let mut cfg = child_server("s", &["exit", "2"]);
    cfg.restart_on_exit_code = None;
    let (sup, _logs) = setup(vec![cfg]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    let status = wait_state(s, ServerState::Crashed).await;
    assert_eq!(status.last_exit_code, Some(2));
}

#[tokio::test]
async fn output_is_captured_with_levels() {
    let (sup, logs) = setup(vec![child_server("s", &["print"])]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    wait_log(&logs, "s", "stderr line").await;
    let traced = wait_log(&logs, "s", "tracing style line").await;
    assert_eq!(traced.level, Some(LogLevel::Warn));
    assert_eq!(traced.target.as_deref(), Some("child::mod"));
    let plain = wait_log(&logs, "s", "plain stdout line").await;
    assert_eq!(plain.level, None);
    assert_eq!(plain.message, "plain stdout line");

    let all = lines(&logs, "s");
    assert!(all.iter().all(|l| l.server == "s"));
    // seq increases by one per line, starting at 1.
    for (i, line) in all.iter().enumerate() {
        assert_eq!(line.seq, i as u64 + 1);
    }
}

#[tokio::test]
async fn graceful_stop() {
    let (sup, logs) = setup(vec![child_server("s", &["sleep"])]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    wait_log(&logs, "s", "ready").await;
    assert_eq!(s.status().state, ServerState::Running);
    assert!(s.status().pid.is_some());

    let began = Instant::now();
    s.stop().await.unwrap();
    let status = s.status();
    assert_eq!(status.state, ServerState::Stopped);
    assert_eq!(status.pid, None);
    // On Unix the child handles SIGTERM and exits 0 (on Windows there is no graceful stop).
    #[cfg(unix)]
    assert_eq!(status.last_exit_code, Some(0));
    assert!(began.elapsed() < Duration::from_secs(2), "stop took too long");
}

#[tokio::test]
async fn child_that_ignores_the_stop_is_killed() {
    let (sup, logs) = setup(vec![child_server("s", &["stubborn"])]);
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    wait_log(&logs, "s", "ready").await;

    let began = Instant::now();
    s.stop().await.unwrap();
    assert_eq!(s.status().state, ServerState::Stopped);
    assert_eq!(s.status().pid, None);
    // Unix: SIGTERM is ignored, so the 2 s timeout runs out and the kill follows.
    #[cfg(unix)]
    {
        assert!(began.elapsed() >= Duration::from_secs(2));
        wait_log(&logs, "s", "did not stop within 2 s, killing it").await;
        assert_eq!(
            s.status().last_exit_code,
            None,
            "killed by a signal has no exit code"
        );
    }
    assert!(began.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn start_stop_restart_edge_cases() {
    let (sup, logs) = setup(vec![child_server("s", &["sleep"])]);
    let s = sup.get("s").unwrap();

    // Stopping a stopped server is fine.
    s.stop().await.unwrap();

    s.start().await.unwrap();
    wait_log(&logs, "s", "ready").await;
    assert_eq!(s.start().await.unwrap_err(), "already running");

    let first_pid = s.status().pid.unwrap();
    s.restart().await.unwrap();
    let status = s.status();
    assert_eq!(status.state, ServerState::Running);
    assert_ne!(status.pid.unwrap(), first_pid);

    // Restart of a stopped server just starts it.
    s.stop().await.unwrap();
    s.restart().await.unwrap();
    assert_eq!(s.status().state, ServerState::Running);
    s.stop().await.unwrap();
}

#[tokio::test]
async fn missing_binary_is_reported_and_does_not_kill_the_supervisor() {
    let mut cfg = child_server("s", &[]);
    cfg.command = "definitely-not-a-real-program-l2".into();
    let (sup, logs) = setup(vec![cfg, child_server("other", &["sleep"])]);
    let s = sup.get("s").unwrap();
    let err = s.start().await.unwrap_err();
    assert!(err.contains("cannot start"), "{err}");
    assert_eq!(s.status().state, ServerState::Crashed);
    wait_log(&logs, "s", "cannot start").await;

    // The other server is unaffected.
    let other = sup.get("other").unwrap();
    other.start().await.unwrap();
    assert_eq!(other.status().state, ServerState::Running);
    sup.shutdown().await;
    assert_eq!(other.status().state, ServerState::Stopped);
}

#[tokio::test]
async fn stop_during_restart_delay_cancels_the_restart() {
    let marker = tempfile::tempdir().unwrap();
    let marker_file = marker.path().join("ran").to_string_lossy().into_owned();
    let mut cfg = config(vec![child_server("s", &["restart-once", &marker_file])]);
    cfg.restart_delay = Duration::from_secs(30);
    let logs = LogStore::new(100);
    let sup = Supervisor::new(&cfg, Arc::clone(&logs));
    let s = sup.get("s").unwrap();
    s.start().await.unwrap();
    wait_log(&logs, "s", "restarting in 30 s").await;
    assert_eq!(s.status().state, ServerState::Starting);
    s.stop().await.unwrap();
    assert_eq!(s.status().state, ServerState::Stopped);
}

#[tokio::test]
async fn state_changes_are_published() {
    let (sup, _logs) = setup(vec![child_server("s", &["exit", "0"])]);
    let mut events = sup.subscribe_events();
    sup.get("s").unwrap().start().await.unwrap();
    let mut seen = Vec::new();
    while seen.last() != Some(&ServerState::Stopped) {
        let event = tokio::time::timeout(WAIT, events.recv()).await.unwrap().unwrap();
        if let l2_admin_api::Event::ServerStateChanged { status } = event {
            assert_eq!(status.name, "s");
            seen.push(status.state);
        }
    }
    assert_eq!(
        seen,
        vec![ServerState::Starting, ServerState::Running, ServerState::Stopped]
    );
}
