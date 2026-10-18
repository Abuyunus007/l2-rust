//! Test child for the supervisor's process tests. Not part of the product.
//!
//! Usage: `supervisor-test-child <mode> [arg]`
//! * `exit <code>`            exit at once with that code
//! * `print`                  write to stdout and stderr (tracing-style and plain lines), exit 0
//! * `restart-once <file>`    exit 2 on the first run (creates `file`), "run again" and sleep on the second
//! * `sleep`                  print "ready", sleep until told to stop (exit 0 on SIGTERM on Unix)
//! * `stubborn`               print "ready", ignore the stop request, sleep forever

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::io::Write;
use std::process::exit;
use std::time::Duration;

fn ready() {
    println!("ready");
    std::io::stdout().flush().unwrap();
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("exit") => exit(args.next().unwrap().parse().unwrap()),
        Some("print") => {
            println!("plain stdout line");
            println!("2026-01-02T03:04:05.123456Z  WARN child::mod: tracing style line");
            eprintln!("stderr line");
            exit(0);
        }
        Some("restart-once") => {
            let marker = args.next().unwrap();
            if std::path::Path::new(&marker).exists() {
                println!("run again");
                std::io::stdout().flush().unwrap();
                tokio::time::sleep(Duration::from_secs(3600)).await;
            } else {
                std::fs::write(&marker, b"x").unwrap();
                exit(2);
            }
        }
        Some("sleep") => {
            ready_and_wait_for_stop().await;
            exit(0);
        }
        Some("stubborn") => {
            ignore_stop(); // before "ready": the test sends the stop as soon as it sees it
            ready();
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
        _ => exit(64),
    }
}

#[cfg(unix)]
async fn ready_and_wait_for_stop() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut stop = signal(SignalKind::terminate()).unwrap();
    ready();
    stop.recv().await;
}

// Windows has no graceful stop the supervisor can send; the test just waits to be killed.
#[cfg(not(unix))]
async fn ready_and_wait_for_stop() {
    ready();
    tokio::time::sleep(Duration::from_secs(3600)).await;
}

#[cfg(unix)]
fn ignore_stop() {
    use tokio::signal::unix::{SignalKind, signal};
    // Installing a handler and never reading it swallows SIGTERM.
    let handler = signal(SignalKind::terminate()).unwrap();
    std::mem::forget(handler);
}

#[cfg(not(unix))]
fn ignore_stop() {}
