//! `supervisor.ini`: the Rust-only settings file of the supervisor (D-36).
//!
//! Java: `dist/login/LoginServer.vbs`, `LoginServerTask.sh`, `dist/game/GameServer.*` (what a start
//! script does: working directory, command line, restart on exit code 2).
//!
//! The file uses the Java `.ini` syntax (read with `l2_config::IniFile`). It has no sections, so
//! the server list is flat: `Servers = login, game` plus `Server.<name>.<Key>` entries. See
//! `supervisor.example.ini`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use l2_config::IniFile;
use thiserror::Error;

pub const DEFAULT_LISTEN: &str = "127.0.0.1:7900";
/// Java start scripts: exit code 2 = "restart me", 0 = normal shutdown.
pub const DEFAULT_RESTART_EXIT_CODE: i32 = 2;
pub const DEFAULT_LOG_BUFFER_LINES: usize = 10_000;
pub const DEFAULT_STOP_TIMEOUT_SECS: u64 = 30;
/// `LoginServerTask.sh` / `GameServer.sh` sleep 10 s before starting again.
pub const DEFAULT_RESTART_DELAY_SECS: u64 = 10;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Ini(#[from] l2_config::ConfigError),
    #[error("invalid value for {key}: {message}")]
    Invalid { key: String, message: String },
    #[error("missing {0}")]
    Missing(String),
}

fn invalid(key: &str, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        key: key.to_string(),
        message: message.into(),
    }
}

/// One supervised process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub name: String,
    /// A bare name is looked up on the `PATH`; a path with separators is already resolved
    /// against the folder of `supervisor.ini`.
    pub command: String,
    pub args: Vec<String>,
    /// Resolved against the folder of `supervisor.ini`. `None` = the supervisor's own directory.
    pub working_dir: Option<PathBuf>,
    /// `host:port` of the server's own admin endpoint (`config/Admin.ini`).
    pub admin_endpoint: Option<String>,
    pub admin_token: String,
    pub autostart: bool,
    /// Exit code that means "restart me". `None` = never restart.
    pub restart_on_exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorConfig {
    pub listen: SocketAddr,
    pub token: String,
    pub log_buffer_lines: usize,
    pub stop_timeout: Duration,
    pub restart_delay: Duration,
    pub servers: Vec<ServerConfig>,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 7900)),
            token: String::new(),
            log_buffer_lines: DEFAULT_LOG_BUFFER_LINES,
            stop_timeout: Duration::from_secs(DEFAULT_STOP_TIMEOUT_SECS),
            restart_delay: Duration::from_secs(DEFAULT_RESTART_DELAY_SECS),
            servers: Vec::new(),
        }
    }
}

impl SupervisorConfig {
    /// Reads `path`. Unlike the Java configs a missing file is an error: without it there is
    /// nothing to supervise and no token.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let ini = IniFile::parse("supervisor.ini", &text)?;
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Self::from_ini(&ini, base)
    }

    /// `base_dir` = folder of `supervisor.ini`, for relative paths.
    pub fn from_ini(ini: &IniFile, base_dir: &Path) -> Result<Self, ConfigError> {
        let defaults = Self::default();
        let listen = match get(ini, "Listen") {
            Some(text) => text
                .parse::<SocketAddr>()
                .map_err(|e| invalid("Listen", format!("'{text}': {e}")))?,
            None => defaults.listen,
        };
        let log_buffer_lines = get_num(ini, "LogBufferLines", defaults.log_buffer_lines)?;
        if log_buffer_lines == 0 {
            return Err(invalid("LogBufferLines", "must be at least 1"));
        }
        let stop_secs = get_num(ini, "StopTimeoutSecs", DEFAULT_STOP_TIMEOUT_SECS)?;
        let delay_secs = get_num(ini, "RestartDelaySecs", DEFAULT_RESTART_DELAY_SECS)?;

        let mut servers: Vec<ServerConfig> = Vec::new();
        for name in get(ini, "Servers")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            if servers.iter().any(|s| s.name == name) {
                return Err(invalid("Servers", format!("'{name}' is listed twice")));
            }
            servers.push(parse_server(ini, name, base_dir)?);
        }

        Ok(Self {
            listen,
            token: get(ini, "Token").unwrap_or_default().to_string(),
            log_buffer_lines,
            stop_timeout: Duration::from_secs(stop_secs),
            restart_delay: Duration::from_secs(delay_secs),
            servers,
        })
    }
}

fn parse_server(ini: &IniFile, name: &str, base_dir: &Path) -> Result<ServerConfig, ConfigError> {
    // The name ends up in log lines, URLs of the GUI and key prefixes: keep it boring.
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(invalid(
            "Servers",
            format!("'{name}': only letters, digits, '_' and '-' are allowed"),
        ));
    }
    let key = |suffix: &str| format!("Server.{name}.{suffix}");

    let command = get(ini, &key("Command"))
        .ok_or_else(|| ConfigError::Missing(key("Command")))?
        .to_string();
    // A relative path to an executable is relative to supervisor.ini, not to whatever the
    // current directory happens to be. Bare names (`java`) stay PATH lookups.
    let command = if command.contains(['/', '\\']) && Path::new(&command).is_relative() {
        base_dir.join(&command).to_string_lossy().into_owned()
    } else {
        command
    };
    let args =
        split_args(get(ini, &key("Args")).unwrap_or_default()).map_err(|m| invalid(&key("Args"), m))?;
    let working_dir = get(ini, &key("WorkingDir")).map(|d| base_dir.join(d));
    let admin_endpoint = get(ini, &key("AdminEndpoint"))
        .map(|e| e.trim_start_matches("ws://").trim_end_matches('/').to_string());
    let autostart = match get(ini, &key("Autostart")) {
        None => false,
        Some(v) => {
            parse_bool(v).ok_or_else(|| invalid(&key("Autostart"), format!("'{v}' is not True/False")))?
        }
    };
    // Key absent = the Java default (2). Key present but empty = never restart.
    let restart_on_exit_code = match get(ini, &key("RestartOnExitCode")) {
        None if !ini.contains_key(&key("RestartOnExitCode")) => Some(DEFAULT_RESTART_EXIT_CODE),
        None => None,
        Some(v) => Some(
            v.parse::<i32>()
                .map_err(|_| invalid(&key("RestartOnExitCode"), format!("'{v}' is not a number")))?,
        ),
    };

    Ok(ServerConfig {
        name: name.to_string(),
        command,
        args,
        working_dir,
        admin_endpoint,
        admin_token: get(ini, &key("AdminToken")).unwrap_or_default().to_string(),
        autostart,
        restart_on_exit_code,
    })
}

/// The value of `key`, trimmed (Java's `Properties` keeps trailing blanks, which would trip over
/// every hand-edited file). `None` when missing or blank.
fn get<'a>(ini: &'a IniFile, key: &str) -> Option<&'a str> {
    ini.get_value(key).map(str::trim).filter(|v| !v.is_empty())
}

fn get_num<T: std::str::FromStr>(ini: &IniFile, key: &str, default: T) -> Result<T, ConfigError> {
    match get(ini, key) {
        None => Ok(default),
        Some(v) => v
            .parse()
            .map_err(|_| invalid(key, format!("'{v}' is not a valid number"))),
    }
}

fn parse_bool(text: &str) -> Option<bool> {
    match text.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Splits a command line on whitespace; `"double quotes"` group words (no escapes).
fn split_args(text: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    for c in text.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if in_quotes {
        return Err("unterminated double quote".into());
    }
    if has_token {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<SupervisorConfig, ConfigError> {
        let ini = IniFile::parse("supervisor.ini", text).unwrap();
        SupervisorConfig::from_ini(&ini, Path::new("base"))
    }

    #[test]
    fn defaults_for_an_empty_file() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg, SupervisorConfig::default());
        assert_eq!(cfg.listen.to_string(), "127.0.0.1:7900");
        assert!(cfg.servers.is_empty());
    }

    #[test]
    fn split_args_handles_quotes() {
        assert_eq!(
            split_args(r#"@java.cfg -jar "../my libs/GameServer.jar" """#).unwrap(),
            vec!["@java.cfg", "-jar", "../my libs/GameServer.jar", ""]
        );
        assert!(split_args("a \"b").is_err());
        assert!(split_args("   ").unwrap().is_empty());
    }

    #[test]
    fn parses_servers() {
        let cfg = parse(
            "Listen = 0.0.0.0:7000\nToken = abc \nLogBufferLines = 50\nStopTimeoutSecs=5\nRestartDelaySecs=0\n\
             Servers = login, game\n\
             Server.login.Command = ../bin/login\n\
             Server.login.WorkingDir = login\n\
             Server.login.AdminEndpoint = ws://127.0.0.1:7901/\n\
             Server.login.AdminToken = t\n\
             Server.login.Autostart = True\n\
             Server.game.Command = java\n\
             Server.game.Args = @java.cfg -jar ../libs/GameServer.jar\n\
             Server.game.RestartOnExitCode =\n",
        )
        .unwrap();
        assert_eq!(cfg.listen.to_string(), "0.0.0.0:7000");
        assert_eq!(cfg.token, "abc");
        assert_eq!(cfg.log_buffer_lines, 50);
        assert_eq!(cfg.stop_timeout, Duration::from_secs(5));
        assert_eq!(cfg.restart_delay, Duration::ZERO);
        let login = &cfg.servers[0];
        assert_eq!(login.name, "login");
        assert_eq!(
            login.command,
            Path::new("base").join("../bin/login").to_string_lossy()
        );
        assert_eq!(login.working_dir, Some(Path::new("base").join("login")));
        assert_eq!(login.admin_endpoint.as_deref(), Some("127.0.0.1:7901"));
        assert_eq!(login.admin_token, "t");
        assert!(login.autostart);
        assert_eq!(login.restart_on_exit_code, Some(2));
        let game = &cfg.servers[1];
        assert_eq!(game.command, "java");
        assert_eq!(game.args, vec!["@java.cfg", "-jar", "../libs/GameServer.jar"]);
        assert!(!game.autostart);
        assert_eq!(game.restart_on_exit_code, None);
        assert_eq!(game.working_dir, None);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            parse("Listen = nonsense"),
            Err(ConfigError::Invalid { .. })
        ));
        assert!(matches!(
            parse("LogBufferLines = 0"),
            Err(ConfigError::Invalid { .. })
        ));
        assert!(matches!(parse("Servers = a"), Err(ConfigError::Missing(_))));
        assert!(matches!(
            parse("Servers = a, a\nServer.a.Command = x"),
            Err(ConfigError::Invalid { .. })
        ));
        assert!(matches!(
            parse("Servers = a/b\nServer.a/b.Command = x"),
            Err(ConfigError::Invalid { .. })
        ));
        assert!(matches!(
            parse("Servers = a\nServer.a.Command = x\nServer.a.RestartOnExitCode = two"),
            Err(ConfigError::Invalid { .. })
        ));
    }

    #[test]
    fn example_config_parses() {
        let text = include_str!("../supervisor.example.ini");
        let cfg = parse(text).unwrap();
        assert_eq!(cfg.servers.len(), 2);
        assert_eq!(cfg.servers[1].name, "game");
        assert_eq!(cfg.servers[1].command, "java");
        assert_eq!(cfg.servers[1].args[0], "@java.cfg");
        assert_eq!(cfg.servers[0].admin_endpoint.as_deref(), Some("127.0.0.1:7901"));
        assert!(cfg.token.is_empty());
    }
}
