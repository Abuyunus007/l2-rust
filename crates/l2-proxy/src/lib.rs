//! A decrypting man-in-the-middle proxy for the L2 login and game protocols (D-34, D-35).
//!
//! The H5 client connects to the proxy instead of the login server. The proxy talks to the real
//! login server (Java or Rust) as a client, decrypts both directions, writes every packet as a
//! [`Record`] to a channel (a JSONL capture file, or a live packet list in the GUI) and rewrites
//! the `ServerList` so the game connection goes through the proxy too. Credentials are never
//! recorded. See the `login` module source for the protocol details.
//!
//! The game connection is a pass-through: the bytes are forwarded unchanged, and a copy is
//! split into frames, decrypted (when the `KeyPacket` announces encryption) and recorded with
//! the opcode name from the generated tables. See the `game` module source.
//!
//! ```no_run
//! # async fn demo() -> Result<(), l2_proxy::ProxyError> {
//! use l2_proxy::{Proxy, ProxyConfig};
//!
//! let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
//! let config = ProxyConfig::new("127.0.0.1:2106".parse().unwrap(), "127.0.0.1:2107".parse().unwrap());
//! let proxy = Proxy::start(config, tx).await?;
//! while let Some(record) = rx.recv().await {
//!     println!("{:?} {}", record.dir, record.name);
//! }
//! proxy.stop().await;
//! # Ok(()) }
//! ```

pub mod capture;
mod game;
mod login;
pub mod rewrite;

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use l2_crypto::{RsaError, RsaKeyPair};
use l2_net::{CancellationToken, ConnectionConfig, PacketError};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

pub use capture::{CaptureError, CaptureReader, CaptureWriter, Channel, Dir, Record};

use game::Relays;
use login::ProxyKeys;

/// Everything that can go wrong in the proxy.
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("RSA error: {0}")]
    Rsa(#[from] RsaError),

    #[error("bad packet: {0}")]
    Packet(#[from] PacketError),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("configuration error: {0}")]
    Config(String),
}

/// Settings of one proxy.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Where the client connects (the login port, normally 2106).
    pub listen: SocketAddr,
    /// The real login server.
    pub upstream: SocketAddr,
    /// Also record the account name of each login attempt. The password is never recorded.
    pub log_accounts: bool,
    /// Rewrite the `ServerList` so game connections go through the proxy (default `true`).
    /// When `false`, the client connects to the game server directly and only login is seen.
    pub rewrite_server_list: bool,
    /// The IPv4 address the client should use to reach the proxy's game ports. Defaults to the
    /// IP of `listen`; required when `listen` is `0.0.0.0`.
    pub advertise_ip: Option<Ipv4Addr>,
    /// Where the game ports are opened. Defaults to the IP of `listen`.
    pub game_bind_ip: Option<IpAddr>,
    /// `Some(base)`: the game server with id `n` listens on port `base + n`. `None`: any free port.
    pub game_port_base: Option<u16>,
    /// How many RSA-1024 pairs to generate at startup (the proxy picks one per client).
    pub rsa_pool: usize,
    /// How long to wait for the real login server's `Init`.
    pub init_timeout: Duration,
}

impl ProxyConfig {
    pub fn new(listen: SocketAddr, upstream: SocketAddr) -> Self {
        Self {
            listen,
            upstream,
            log_accounts: false,
            rewrite_server_list: true,
            advertise_ip: None,
            game_bind_ip: None,
            game_port_base: None,
            rsa_pool: 4,
            init_timeout: Duration::from_secs(10),
        }
    }
}

/// State shared by every connection of one proxy.
pub(crate) struct Shared {
    pub(crate) config: ProxyConfig,
    pub(crate) advertise_ip: Ipv4Addr,
    pub(crate) keys: Vec<Arc<ProxyKeys>>,
    pub(crate) relays: Relays,
    pub(crate) records: UnboundedSender<Record>,
    pub(crate) conn_config: Arc<ConnectionConfig>,
    pub(crate) cancel: CancellationToken,
    /// Connection numbers, shared with the game relays so every capture `conn` is unique.
    next_conn: Arc<AtomicU64>,
}

/// A running proxy.
pub struct Proxy {
    local_addr: SocketAddr,
    shared: Arc<Shared>,
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

impl Proxy {
    /// Generates the RSA pool, opens the login listener and starts accepting clients. Every
    /// packet is sent to `records` (unbounded, so the proxy never blocks on a slow consumer).
    pub async fn start(config: ProxyConfig, records: UnboundedSender<Record>) -> Result<Self, ProxyError> {
        let advertise_ip = match (config.advertise_ip, config.listen.ip()) {
            (Some(ip), _) => ip,
            (None, IpAddr::V4(ip)) if !ip.is_unspecified() => ip,
            _ => {
                return Err(ProxyError::Config(
                    "the listen address has no usable IPv4 address to give to the client; set advertise_ip"
                        .into(),
                ));
            }
        };
        if config.rsa_pool == 0 {
            return Err(ProxyError::Config("rsa_pool must be at least 1".into()));
        }
        let game_bind_ip = config.game_bind_ip.unwrap_or_else(|| config.listen.ip());

        let pool = config.rsa_pool;
        let keys = tokio::task::spawn_blocking(move || generate_keys(pool))
            .await
            .map_err(|e| ProxyError::Config(format!("key generation thread failed: {e}")))??;

        let listener = TcpListener::bind(config.listen).await?;
        let local_addr = listener.local_addr()?;
        let cancel = CancellationToken::new();
        let next_conn = Arc::new(AtomicU64::new(1));
        let shared = Arc::new(Shared {
            relays: Relays::new(
                game_bind_ip,
                config.game_port_base,
                cancel.clone(),
                records.clone(),
                Arc::clone(&next_conn),
            ),
            config,
            advertise_ip,
            keys,
            records,
            conn_config: Arc::new(ConnectionConfig::default()),
            cancel: cancel.clone(),
            next_conn,
        });
        tracing::info!(
            "login proxy listening on {local_addr}, upstream {}",
            shared.config.upstream
        );
        let task = tokio::spawn(accept_loop(listener, Arc::clone(&shared)));
        Ok(Self {
            local_addr,
            shared,
            cancel,
            task,
        })
    }

    /// The address the login listener is bound to (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// The game relays opened so far: `(server id, local port, real address)`.
    pub fn game_relays(&self) -> Vec<(u8, u16, SocketAddr)> {
        self.shared.relays.snapshot()
    }

    /// Stops accepting, closes every connection and the game relays.
    pub async fn stop(self) {
        self.cancel.cancel();
        let _ = self.task.await;
    }
}

fn generate_keys(count: usize) -> Result<Vec<Arc<ProxyKeys>>, RsaError> {
    let mut rng = rand::rng();
    (0..count)
        .map(|_| {
            let pair = RsaKeyPair::generate(1024, &mut rng)?;
            let scrambled_modulus = pair.scrambled_modulus()?;
            Ok(Arc::new(ProxyKeys {
                pair,
                scrambled_modulus,
            }))
        })
        .collect()
}

async fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = shared.cancel.cancelled() => return,
        };
        match accepted {
            Ok((stream, peer)) => {
                let conn = shared.next_conn.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(login::serve_client(Arc::clone(&shared), stream, peer, conn));
            }
            Err(e) => tracing::warn!("accept failed: {e}"),
        }
    }
}
