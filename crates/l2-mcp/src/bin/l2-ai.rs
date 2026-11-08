//! `l2-ai`: chat in the terminal with a local model that runs admin commands on the game server
//! (D-63). The model runs in `llama-server` (llama.cpp) or Ollama on this machine; `l2-ai` gives
//! it the bridge's tools and asks the administrator before every command that punishes, removes
//! or stops something.
//!
//! Settings, each from an argument or else the environment:
//! - `--llm-url` / `L2_AI_LLM_URL`: the model's OpenAI-compatible endpoint, default
//!   `http://127.0.0.1:8080` (`llama-server`; Ollama: `http://127.0.0.1:11434`).
//! - `--model` / `L2_AI_MODEL`: the model's name at that endpoint (Ollama needs it), default `local`.
//! - `--url`, `--server`, `--audit-log`, `--no-destructive` and `L2_ADMIN_TOKEN`: as for `l2-mcp`.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use l2_admin_api::ADMIN_TOKEN_ENV;
use l2_mcp::audit::Audit;
use l2_mcp::backend::AdminBackend;
use l2_mcp::chat::{Chat, Confirm};
use l2_mcp::llm::HttpLlm;
use l2_mcp::server::McpServer;
use tokio::io::{AsyncBufReadExt, BufReader, Lines, Stdin};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

const USAGE: &str =
    "Usage: l2-ai [--llm-url http://host:port] [--model NAME] [--url ws://host:port] [--server NAME]
             [--audit-log FILE|none] [--no-destructive]
The token of the admin endpoint comes from the environment variable L2_ADMIN_TOKEN.";

type Input = Arc<Mutex<Lines<BufReader<Stdin>>>>;

struct Settings {
    llm_url: String,
    model: String,
    url: String,
    server: Option<String>,
    token: String,
    audit_log: Option<PathBuf>,
    destructive: bool,
}

fn settings() -> Result<Settings> {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let mut llm_url = env("L2_AI_LLM_URL").unwrap_or_else(|| "http://127.0.0.1:8080".to_owned());
    let mut model = env("L2_AI_MODEL").unwrap_or_else(|| "local".to_owned());
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
            "--llm-url" => llm_url = value("--llm-url")?,
            "--model" => model = value("--model")?,
            "--url" => url = value("--url")?,
            "--server" => server = Some(value("--server")?),
            "--audit-log" => audit = value("--audit-log")?,
            "--no-destructive" => destructive = false,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    Ok(Settings {
        llm_url,
        model,
        url,
        server,
        token: env(ADMIN_TOKEN_ENV).unwrap_or_default(),
        audit_log: (!audit.eq_ignore_ascii_case("none")).then(|| PathBuf::from(audit)),
        destructive,
    })
}

/// Asks in the terminal.
struct TerminalConfirm {
    input: Input,
}

impl Confirm for TerminalConfirm {
    async fn approve(&self, command: &str, reason: &str) -> bool {
        print!(
            "\n!! Destructive command: {command}\n   Reason: {reason}\n   Run it? (y/j = yes, anything else = no): "
        );
        let _ = std::io::stdout().flush();
        let answer = self
            .input
            .lock()
            .await
            .next_line()
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes" | "j" | "ja"
        )
    }
}

async fn read_line(input: &Input) -> Result<Option<String>> {
    Ok(input.lock().await.next_line().await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();
    let settings = settings()?;
    let llm = HttpLlm::new(&settings.llm_url).map_err(anyhow::Error::msg)?;
    let backend = AdminBackend::new(settings.url.clone(), settings.token, settings.server);
    let server = McpServer::new(backend, Audit::new(settings.audit_log), settings.destructive);
    let input: Input = Arc::new(Mutex::new(BufReader::new(tokio::io::stdin()).lines()));
    let confirm = TerminalConfirm {
        input: Arc::clone(&input),
    };
    let mut chat = Chat::new(server, llm, confirm, settings.model);

    println!(
        "l2-ai: model at {}, game server at {}. Type what you want done; /reset forgets the chat, /exit ends it.",
        settings.llm_url, settings.url
    );
    loop {
        print!("\n> ");
        let _ = std::io::stdout().flush();
        let Some(line) = read_line(&input).await? else {
            break;
        };
        match line.trim() {
            "" => {}
            "/exit" | "/quit" => break,
            "/reset" => {
                chat.reset();
                println!("(The chat is forgotten.)");
            }
            text => {
                println!("...");
                match chat.say(text).await {
                    Ok(answer) => println!("{answer}"),
                    Err(error) => println!("[error] {error}"),
                }
            }
        }
    }
    Ok(())
}
