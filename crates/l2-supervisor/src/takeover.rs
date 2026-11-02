//! Replacing a supervisor that holds our port but belongs to another folder (D-56): an older
//! package, or a copy started from somewhere else. The GUI asks the user first.

use std::net::SocketAddr;
use std::process::Command;
use std::time::{Duration, Instant};

use l2_admin_api::{AdminClient, Request};

use crate::launcher::{connect_addr, port_open, supervisor_exe_name};

/// How long the other supervisor gets to stop its servers and exit.
const STOP_WAIT: Duration = Duration::from_secs(45);

/// How the other supervisor went away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakeOver {
    /// It stopped its servers and exited on `StopSupervisor`.
    Stopped,
    /// An older version that wants a token: its process (and its servers) were ended.
    Ended { pid: u32 },
}

/// Stops whatever supervisor listens on `listen`: first through the admin API (a current version
/// needs no token from this machine), else by ending its process when the port's owner is an
/// `l2-supervisor`. Blocking; run it off the UI thread.
pub fn take_over(listen: SocketAddr) -> Result<TakeOver, String> {
    let url = format!("ws://{}", connect_addr(listen));
    let graceful = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(async {
            let (client, _events) = AdminClient::connect(&url, "").await.map_err(|e| e.to_string())?;
            client
                .request(Request::StopSupervisor)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
    if graceful.is_ok() && wait_until_closed(listen, STOP_WAIT) {
        return Ok(TakeOver::Stopped);
    }
    let pid = port_owner(listen.port()).ok_or_else(|| {
        format!(
            "port {} is used, but its owner could not be found; stop the other supervisor by hand",
            listen.port()
        )
    })?;
    let image = process_image(pid).unwrap_or_default();
    if !image.eq_ignore_ascii_case(supervisor_exe_name()) {
        return Err(format!(
            "port {} belongs to {image} (pid {pid}), not to a supervisor; it was left alone",
            listen.port()
        ));
    }
    end_process_tree(pid)?;
    if wait_until_closed(listen, Duration::from_secs(10)) {
        Ok(TakeOver::Ended { pid })
    } else {
        Err(format!("the supervisor (pid {pid}) did not go away"))
    }
}

fn wait_until_closed(listen: SocketAddr, limit: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if !port_open(listen) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

/// The pid that listens on TCP `port` on this machine.
#[cfg(windows)]
fn port_owner(port: u16) -> Option<u32> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok()?;
    parse_netstat_owner(&String::from_utf8_lossy(&output.stdout), port)
}

#[cfg(not(windows))]
fn port_owner(port: u16) -> Option<u32> {
    // `ss -ltnpH 'sport = :7900'` prints `... users:(("l2-supervisor",pid=1234,fd=9))`.
    let output = Command::new("ss")
        .args(["-ltnpH", &format!("sport = :{port}")])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let at = text.find("pid=")?;
    text[at + 4..]
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// `netstat -ano` lines look like `  TCP    127.0.0.1:7900   0.0.0.0:0   LISTENING   1234`.
fn parse_netstat_owner(text: &str, port: u16) -> Option<u32> {
    let suffix = format!(":{port}");
    text.lines().find_map(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        match cols.as_slice() {
            [proto, local, _, state, pid]
                if proto.eq_ignore_ascii_case("TCP")
                    && local.ends_with(&suffix)
                    && state.eq_ignore_ascii_case("LISTENING") =>
            {
                pid.parse().ok()
            }
            _ => None,
        }
    })
}

#[cfg(windows)]
fn process_image(pid: u32) -> Option<String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text
        .lines()
        .next()?
        .split(',')
        .next()?
        .trim_matches('"')
        .to_string();
    (!first.is_empty() && !first.starts_with("INFO")).then_some(first)
}

#[cfg(not(windows))]
fn process_image(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim().to_string())
}

/// Ends the supervisor and the servers it started (an old supervisor cannot be asked).
#[cfg(windows)]
fn end_process_tree(pid: u32) -> Result<(), String> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|e| e.to_string())?;
    if status.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&status.stderr).trim().to_string())
    }
}

#[cfg(not(windows))]
fn end_process_tree(pid: u32) -> Result<(), String> {
    // SIGTERM: the supervisor stops its servers itself on it.
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("kill -TERM {pid} failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_listening_owner_in_netstat_output() {
        let text = "\nActive Connections\n\n  Proto  Local Address          Foreign Address        State           PID\n\
                    \x20 TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1100\n\
                    \x20 TCP    127.0.0.1:7900         0.0.0.0:0              LISTENING       9032\n\
                    \x20 TCP    127.0.0.1:7900         127.0.0.1:56093        ESTABLISHED     9032\n\
                    \x20 TCP    127.0.0.1:17900        0.0.0.0:0              LISTENING       4444\n";
        assert_eq!(parse_netstat_owner(text, 7900), Some(9032));
        assert_eq!(parse_netstat_owner(text, 135), Some(1100));
        assert_eq!(parse_netstat_owner(text, 900), None);
    }
}
