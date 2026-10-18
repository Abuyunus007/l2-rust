//! Per-server log ring buffers, line parsing and live streaming.
//!
//! Every output line of a child becomes a [`LogLine`] with a per-server `seq` (starting at 1).
//! The newest `capacity` lines of each server are kept for `SubscribeLogs.since` backlogs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_admin_api::{LogLevel, LogLine};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::broadcast;

/// Live subscribers that fall further behind than this lose lines (they get a warning, not a hang).
const LIVE_CHANNEL_CAPACITY: usize = 4096;
/// Longer lines are cut: a runaway child must not fill memory through 10,000 huge lines.
const MAX_MESSAGE_CHARS: usize = 16 * 1024;

#[derive(Default)]
struct ServerLog {
    next_seq: u64,
    lines: VecDeque<LogLine>,
}

pub struct LogStore {
    capacity: usize,
    servers: Mutex<HashMap<String, ServerLog>>,
    live: broadcast::Sender<LogLine>,
}

impl LogStore {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (live, _) = broadcast::channel(LIVE_CHANNEL_CAPACITY);
        Arc::new(Self {
            capacity: capacity.max(1),
            servers: Mutex::default(),
            live,
        })
    }

    fn servers(&self) -> MutexGuard<'_, HashMap<String, ServerLog>> {
        // A panic while holding the lock can't leave the map half-updated in a harmful way.
        self.servers.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Stores one raw output line of `server` (parsing its level) and streams it to subscribers.
    pub fn push(&self, server: &str, raw: &str) -> LogLine {
        let parsed = parse_line(raw);
        self.push_parsed(server, parsed.level, parsed.target, parsed.message)
    }

    /// A line written by the supervisor itself about `server` (restarts, exit codes).
    pub fn push_supervisor(&self, server: &str, level: LogLevel, message: String) -> LogLine {
        self.push_parsed(server, Some(level), Some("supervisor".into()), message)
    }

    fn push_parsed(
        &self,
        server: &str,
        level: Option<LogLevel>,
        target: Option<String>,
        mut message: String,
    ) -> LogLine {
        if let Some((cut, _)) = message.char_indices().nth(MAX_MESSAGE_CHARS) {
            message.truncate(cut);
            message.push_str(" [truncated]");
        }
        let mut servers = self.servers();
        let log = servers.entry(server.to_string()).or_default();
        log.next_seq += 1;
        let line = LogLine {
            server: server.to_string(),
            seq: log.next_seq,
            ts_ms: now_ms(),
            level,
            target,
            message,
        };
        if log.lines.len() == self.capacity {
            log.lines.pop_front();
        }
        log.lines.push_back(line.clone());
        // Sent while the lock is held, so `subscribe` sees every line exactly once, either in its
        // backlog or on its receiver.
        let _ = self.live.send(line.clone());
        line
    }

    /// Backlog plus a receiver for everything after it.
    ///
    /// `server: None` = all servers. `since` keeps only lines with `seq > since`; seq numbers are
    /// per server, so with several servers one `since` applies to each of them.
    /// The backlog is ordered by time. The receiver carries *all* servers; filter by
    /// `LogLine::server`.
    pub fn subscribe(
        &self,
        server: Option<&str>,
        since: Option<u64>,
    ) -> (Vec<LogLine>, broadcast::Receiver<LogLine>) {
        let servers = self.servers();
        let receiver = self.live.subscribe();
        let since = since.unwrap_or(0);
        let mut backlog: Vec<LogLine> = servers
            .iter()
            .filter(|(name, _)| server.is_none_or(|s| s == name.as_str()))
            .flat_map(|(_, log)| log.lines.iter())
            .filter(|l| l.seq > since)
            .cloned()
            .collect();
        backlog.sort_by(|a, b| (a.ts_ms, &a.server, a.seq).cmp(&(b.ts_ms, &b.server, b.seq)));
        (backlog, receiver)
    }

    /// Reads `reader` line by line into the store until EOF. Used for a child's stdout and stderr.
    pub async fn capture<R: AsyncRead + Unpin>(self: Arc<Self>, server: String, reader: R) {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    // Not every JVM writes UTF-8 to a pipe; never fail on a bad byte.
                    let text = String::from_utf8_lossy(&buf);
                    self.push(&server, text.trim_end_matches(['\n', '\r']));
                }
            }
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedLine {
    pub level: Option<LogLevel>,
    pub target: Option<String>,
    pub message: String,
}

/// Extracts level and target from a line when its format is known, else keeps the whole line.
///
/// Known formats:
/// * `tracing-subscriber` fmt (the Rust servers): `2026-01-02T03:04:05.123456Z  INFO target: message`
/// * Java `ConsoleLogFormatter` (`[dd/MM HH:mm:ss] message`) has no level: `level = None`.
/// * `java.util.logging` fallback output (`SEVERE: ...`, `WARNING: ...`, `INFO: ...`).
///
/// ANSI colour codes are removed.
pub fn parse_line(raw: &str) -> ParsedLine {
    let clean = strip_ansi(raw);
    let plain = |level: Option<LogLevel>, message: &str| ParsedLine {
        level,
        target: None,
        message: message.to_string(),
    };

    if let Some(parsed) = parse_tracing(&clean) {
        return parsed;
    }

    // java.util.logging default format
    for (prefix, level) in [
        ("SEVERE: ", LogLevel::Error),
        ("WARNING: ", LogLevel::Warn),
        ("INFO: ", LogLevel::Info),
    ] {
        if let Some(message) = clean.strip_prefix(prefix) {
            return plain(Some(level), message);
        }
    }
    plain(None, &clean)
}

/// `<timestamp> <LEVEL> [target: ]message`, fields separated by one or more spaces.
fn parse_tracing(clean: &str) -> Option<ParsedLine> {
    let (timestamp, rest) = clean.trim_start().split_once(char::is_whitespace)?;
    if !looks_like_timestamp(timestamp) {
        return None;
    }
    let rest = rest.trim_start();
    let (word, rest) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let level = level_from_word(word)?;
    let rest = rest.trim_start();
    // The target is the first word if it ends with a colon (module paths contain "::").
    if let Some((first, message)) = rest.split_once(' ')
        && let Some(target) = first.strip_suffix(':')
        && !target.is_empty()
    {
        return Some(ParsedLine {
            level: Some(level),
            target: Some(target.to_string()),
            message: message.trim_start().to_string(),
        });
    }
    Some(ParsedLine {
        level: Some(level),
        target: None,
        message: rest.to_string(),
    })
}

fn looks_like_timestamp(word: &str) -> bool {
    // 2026-01-02T03:04:05Z and variants; RFC 3339 starts with a digit and has a 'T' or ':'.
    word.starts_with(|c: char| c.is_ascii_digit()) && word.contains(['T', ':'])
}

fn level_from_word(word: &str) -> Option<LogLevel> {
    match word {
        "ERROR" => Some(LogLevel::Error),
        "WARN" => Some(LogLevel::Warn),
        "INFO" => Some(LogLevel::Info),
        "DEBUG" => Some(LogLevel::Debug),
        "TRACE" => Some(LogLevel::Trace),
        _ => None,
    }
}

/// Removes `ESC [ ... <letter>` sequences (colours from `tracing-subscriber`).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tracing_format() {
        let p = parse_line("2026-01-02T03:04:05.123456Z  INFO login_server::net: Listening on 0.0.0.0:2106");
        assert_eq!(p.level, Some(LogLevel::Info));
        assert_eq!(p.target.as_deref(), Some("login_server::net"));
        assert_eq!(p.message, "Listening on 0.0.0.0:2106");

        let p = parse_line("2026-01-02T03:04:05Z ERROR boom");
        assert_eq!(p.level, Some(LogLevel::Error));
        assert_eq!(p.target, None);
        assert_eq!(p.message, "boom");
    }

    #[test]
    fn strips_ansi_colours() {
        let p = parse_line(
            "\u{1b}[2m2026-01-02T03:04:05Z\u{1b}[0m \u{1b}[33m WARN\u{1b}[0m \u{1b}[2mx:\u{1b}[0m careful",
        );
        assert_eq!(p.level, Some(LogLevel::Warn));
        assert_eq!(p.target.as_deref(), Some("x"));
        assert_eq!(p.message, "careful");
    }

    #[test]
    fn java_lines() {
        let p = parse_line("[18/10 12:00:00] Loaded 0 IP Bans.");
        assert_eq!(p.level, None);
        assert_eq!(p.message, "[18/10 12:00:00] Loaded 0 IP Bans.");
        let p = parse_line("SEVERE: something broke");
        assert_eq!(p.level, Some(LogLevel::Error));
        assert_eq!(p.message, "something broke");
    }

    #[test]
    fn unknown_lines_are_kept() {
        for raw in [
            "",
            "INFO is not at the start",
            "12 hello",
            "2026-01-02T03:04:05Z NOTICE x",
        ] {
            let p = parse_line(raw);
            assert_eq!(p.level, None, "{raw}");
            assert_eq!(p.message, raw);
        }
    }

    #[test]
    fn ring_buffer_drops_oldest_and_seq_continues() {
        let store = LogStore::new(3);
        for i in 1..=5 {
            store.push("a", &format!("line {i}"));
        }
        store.push("b", "other");
        let (backlog, _rx) = store.subscribe(Some("a"), None);
        let seqs: Vec<u64> = backlog.iter().map(|l| l.seq).collect();
        assert_eq!(seqs, vec![3, 4, 5]);
        let (backlog, _rx) = store.subscribe(Some("a"), Some(4));
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].message, "line 5");
        let (backlog, _rx) = store.subscribe(None, None);
        assert_eq!(backlog.len(), 4);
    }

    #[test]
    fn subscribe_gets_each_line_once() {
        let store = LogStore::new(10);
        store.push("a", "old");
        let (backlog, mut rx) = store.subscribe(None, None);
        store.push("a", "new");
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].message, "old");
        assert_eq!(rx.try_recv().unwrap().message, "new");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn long_lines_are_truncated() {
        let store = LogStore::new(10);
        let line = store.push("a", &"x".repeat(MAX_MESSAGE_CHARS + 100));
        assert!(line.message.ends_with(" [truncated]"));
        assert_eq!(
            line.message.chars().count(),
            MAX_MESSAGE_CHARS + " [truncated]".len()
        );
    }

    #[tokio::test]
    async fn capture_reads_lines_and_survives_bad_bytes() {
        let store = LogStore::new(10);
        let data: &[u8] = b"first\r\nsecond \xff\xfe\nlast without newline";
        Arc::clone(&store).capture("a".into(), data).await;
        let (backlog, _rx) = store.subscribe(Some("a"), None);
        let messages: Vec<&str> = backlog.iter().map(|l| l.message.as_str()).collect();
        assert_eq!(
            messages,
            vec!["first", "second \u{fffd}\u{fffd}", "last without newline"]
        );
    }
}
