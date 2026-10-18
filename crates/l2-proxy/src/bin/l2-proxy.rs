//! `l2-proxy`: the standalone login and game proxy. The GUI runs the same library in-process (D-35).
//!
//! Point the game client at `--listen`, point `--upstream` at the real login server:
//!
//! ```text
//! l2-proxy --listen 127.0.0.1:2106 --upstream 127.0.0.1:2107 --capture login.jsonl
//! ```

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use l2_proxy::{CaptureWriter, Dir, Proxy, ProxyConfig};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

/// Decrypting login and game proxy with JSONL captures.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Where the game client connects.
    #[arg(long, default_value = "127.0.0.1:2106")]
    listen: SocketAddr,

    /// The real login server (ip:port).
    #[arg(long)]
    upstream: SocketAddr,

    /// Write every packet to this JSON-lines file (credentials are always redacted).
    #[arg(long, conflicts_with = "capture_dir")]
    capture: Option<PathBuf>,

    /// Write each run to a new file `capture-<unix-ms>.jsonl` in this folder (created if
    /// missing). Use this when the proxy runs permanently, e.g. under the supervisor.
    #[arg(long)]
    capture_dir: Option<PathBuf>,

    /// Also record the account name of each login attempt (never the password).
    #[arg(long)]
    log_accounts: bool,

    /// IPv4 address the client uses to reach the proxy's game ports. Default: the IP of --listen.
    #[arg(long)]
    advertise_ip: Option<Ipv4Addr>,

    /// Address the game ports are opened on. Default: the IP of --listen.
    #[arg(long)]
    game_bind: Option<IpAddr>,

    /// Open the game port of server id N on BASE+N instead of a free port.
    #[arg(long, value_name = "BASE")]
    game_port_base: Option<u16>,

    /// Leave the server list alone: the client then connects to the game server directly.
    #[arg(long)]
    no_rewrite: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let args = Args::parse();

    let capture_path = match (&args.capture, &args.capture_dir) {
        (Some(path), _) => Some(path.clone()),
        (None, Some(dir)) => {
            std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            Some(dir.join(format!("capture-{ms}.jsonl")))
        }
        (None, None) => None,
    };
    let mut writer = match &capture_path {
        Some(path) => Some(
            CaptureWriter::create(path)
                .with_context(|| format!("cannot create capture {}", path.display()))?,
        ),
        None => None,
    };

    let mut config = ProxyConfig::new(args.listen, args.upstream);
    config.log_accounts = args.log_accounts;
    config.advertise_ip = args.advertise_ip;
    config.game_bind_ip = args.game_bind;
    config.game_port_base = args.game_port_base;
    config.rewrite_server_list = !args.no_rewrite;

    let (tx, mut rx) = mpsc::unbounded_channel();
    println!("Generating RSA keys...");
    let proxy = Proxy::start(config, tx).await.context("cannot start the proxy")?;
    println!(
        "Proxy ready: client -> {}  =>  login server {}",
        proxy.local_addr(),
        args.upstream
    );
    if let Some(path) = &capture_path {
        println!("Capturing to {} (Ctrl+C to stop)", path.display());
    }

    let drain = tokio::spawn(async move {
        while let Some(record) = rx.recv().await {
            let arrow = match record.dir {
                Dir::C2s => "client -> server",
                Dir::S2c => "server -> client",
            };
            match record.ex_opcode {
                Some(ex) => tracing::info!(
                    "[conn {}] {arrow}: {} (opcode {:#04x} {ex:#06x})",
                    record.conn,
                    record.name,
                    record.opcode
                ),
                None => tracing::info!(
                    "[conn {}] {arrow}: {} (opcode {:#04x})",
                    record.conn,
                    record.name,
                    record.opcode
                ),
            }
            if let Some(writer) = writer.as_mut() {
                if let Err(e) = writer.write_record(&record) {
                    tracing::error!("cannot write the capture: {e}");
                }
            }
        }
    });

    tokio::signal::ctrl_c().await.context("cannot wait for Ctrl+C")?;
    println!("Stopping...");
    proxy.stop().await;
    // Records are flushed one by one; give the writer a moment to drain what is queued.
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    Ok(())
}
