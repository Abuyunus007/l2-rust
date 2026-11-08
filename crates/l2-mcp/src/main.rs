//! `l2-mcp`: the MCP bridge (D-63). An MCP client starts it and talks to it on stdin/stdout; it
//! runs the admin commands on the game server through the admin endpoint.
//!
//! Settings, each from an argument or else the environment:
//! - `--url` / `L2_MCP_URL`: the admin endpoint, default `ws://127.0.0.1:7902` (`Admin.ini`
//!   `Bind` of the game server, or a supervisor's endpoint).
//! - `--server` / `L2_MCP_SERVER`: the game server's name when the endpoint is a supervisor.
//! - `L2_ADMIN_TOKEN`: the endpoint's token (`Admin.ini` `Token`). Only from the environment: an
//!   argument would show in the process list.
//! - `--audit-log` / `L2_MCP_AUDIT_LOG`: default `log/mcp-audit.log`; `--audit-log none` for none.
//! - `--no-destructive`: leave out the tool that runs commands that punish, remove or stop.
//!
//! Nothing but MCP messages may go to stdout: the log goes to stderr.

use std::path::PathBuf;

use anyhow::{Result, bail};
use l2_admin_api::ADMIN_TOKEN_ENV;
use l2_mcp::audit::Audit;
use l2_mcp::backend::AdminBackend;
use l2_mcp::server::McpServer;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::info;

const USAGE: &str =
    "Usage: l2-mcp [--url ws://host:port] [--server NAME] [--audit-log FILE|none] [--no-destructive]
The token comes from the environment variable L2_ADMIN_TOKEN.";

struct Settings {
    url: String,
    server: Option<String>,
    token: String,
    audit_log: Option<PathBuf>,
    destructive: bool,
}

fn settings() -> Result<Settings> {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let mut url = env("L2_MCP_URL").unwrap_or_else(|| "ws://127.0.0.1:7902".to_owned());
    let mut server = env("L2_MCP_SERVER");
    let mut audit = env("L2_MCP_AUDIT_LOG").unwrap_or_else(|| "log/mcp-audit.log".to_owned());
    let mut destructive = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| match args.next() {
            Some(v) => Ok(v),
            None => bail!("{name} needs a value\n{USAGE}"),
        };
        match arg.as_str() {
            "--url" => url = value("--url")?,
            "--server" => server = Some(value("--server")?),
            "--audit-log" => audit = value("--audit-log")?,
            "--no-destructive" => destructive = false,
            "--help" | "-h" => {
                eprintln!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    Ok(Settings {
        url,
        server,
        token: env(ADMIN_TOKEN_ENV).unwrap_or_default(),
        audit_log: (!audit.eq_ignore_ascii_case("none")).then(|| PathBuf::from(audit)),
        destructive,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let settings = settings()?;
    info!(
        "l2-mcp for {}{}, destructive commands {}",
        settings.url,
        settings
            .server
            .as_deref()
            .map(|s| format!(" (server {s})"))
            .unwrap_or_default(),
        if settings.destructive {
            "with approval"
        } else {
            "off"
        }
    );
    let backend = AdminBackend::new(settings.url, settings.token, settings.server);
    let server = McpServer::new(backend, Audit::new(settings.audit_log), settings.destructive);

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if let Some(reply) = server.handle_line(&line).await {
            stdout.write_all(reply.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}
