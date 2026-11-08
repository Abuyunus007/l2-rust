//! The audit log: every command the assistant ran, and what came back.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use tracing::warn;

/// Appends one line per command to a file (`None`: no log).
#[derive(Debug, Clone, Default)]
pub struct Audit {
    path: Option<PathBuf>,
}

impl Audit {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    /// `2026-11-08 14:03:12 run_admin_command "kick Bob" -> ok: Bob was kicked.` The first line
    /// of the answer is enough to see what happened. A log that cannot be written gives a
    /// warning; the command has run either way.
    pub fn record(&self, tool: &str, command: &str, ok: bool, answer: &str) {
        let Some(path) = &self.path else {
            return;
        };
        let first = answer.lines().next().unwrap_or_default();
        let line = format!(
            "{} {tool} {command:?} -> {}: {first}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            if ok { "ok" } else { "error" },
        );
        let written = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| OpenOptions::new().create(true).append(true).open(path))
            .and_then(|mut file| file.write_all(line.as_bytes()));
        if let Err(error) = written {
            warn!("The audit log {} could not be written: {error}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_command_is_one_line_with_its_first_answer_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log").join("mcp-audit.log");
        let audit = Audit::new(Some(path.clone()));
        audit.record(
            "run_admin_command",
            "kick Bob",
            true,
            "Bob was kicked.\nsecond line",
        );
        audit.record(
            "run_destructive_admin_command",
            "ban_char Amy",
            false,
            "no such player",
        );
        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].ends_with(r#"run_admin_command "kick Bob" -> ok: Bob was kicked."#),
            "{}",
            lines[0]
        );
        assert!(
            lines[1].ends_with(r#""ban_char Amy" -> error: no such player"#),
            "{}",
            lines[1]
        );
        Audit::new(None).record("x", "y", true, "z");
    }
}
