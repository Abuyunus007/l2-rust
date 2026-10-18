//! Per-server game listeners: one local port per game server id from the `ServerList`.
//!
//! Until M1-PROXY-02 adds game packet decoding, a listener is a plain TCP relay to the real
//! game server, so the real client can get past the server list and into the game through the
//! proxy. M1-PROXY-02 replaces the relay loop with a decoding one.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};

use l2_net::CancellationToken;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};

struct RelayEntry {
    port: u16,
    /// The real game server address. Updated when a later `ServerList` reports another one.
    target: Arc<Mutex<SocketAddr>>,
}

/// All game listeners of one proxy run.
pub(crate) struct Relays {
    bind_ip: IpAddr,
    /// `Some(base)`: server id `n` listens on `base + n`. `None`: an ephemeral port.
    port_base: Option<u16>,
    cancel: CancellationToken,
    entries: Mutex<HashMap<u8, RelayEntry>>,
}

impl Relays {
    pub(crate) fn new(bind_ip: IpAddr, port_base: Option<u16>, cancel: CancellationToken) -> Self {
        Self {
            bind_ip,
            port_base,
            cancel,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Makes sure a listener exists for `server_id` that relays to `real`, and returns its local
    /// port. A second call for the same id keeps the port and only updates the target.
    pub(crate) async fn ensure(&self, server_id: u8, real: SocketAddr) -> io::Result<u16> {
        if let Some(port) = self.update_existing(server_id, real) {
            return Ok(port);
        }
        let port = match self.port_base {
            Some(base) => base.checked_add(u16::from(server_id)).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "game port base + server id overflows",
                )
            })?,
            None => 0,
        };
        let listener = TcpListener::bind(SocketAddr::new(self.bind_ip, port)).await?;
        let port = listener.local_addr()?.port();

        let target = Arc::new(Mutex::new(real));
        {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(existing) = entries.get(&server_id) {
                // Another connection created it while we were binding; use that one.
                *existing.target.lock().unwrap_or_else(PoisonError::into_inner) = real;
                return Ok(existing.port);
            }
            entries.insert(
                server_id,
                RelayEntry {
                    port,
                    target: Arc::clone(&target),
                },
            );
        }
        tracing::info!(
            "game relay for server {server_id}: {}:{port} -> {real}",
            self.bind_ip
        );
        tokio::spawn(accept_loop(listener, target, self.cancel.clone(), server_id));
        Ok(port)
    }

    fn update_existing(&self, server_id: u8, real: SocketAddr) -> Option<u16> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = entries.get(&server_id)?;
        let mut target = entry.target.lock().unwrap_or_else(PoisonError::into_inner);
        if *target != real {
            tracing::info!("game server {server_id} moved: {} -> {real}", *target);
            *target = real;
        }
        Some(entry.port)
    }

    /// `(server id, local port, real address)` for every listener, sorted by id.
    pub(crate) fn snapshot(&self) -> Vec<(u8, u16, SocketAddr)> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let mut all: Vec<_> = entries
            .iter()
            .map(|(id, e)| {
                (
                    *id,
                    e.port,
                    *e.target.lock().unwrap_or_else(PoisonError::into_inner),
                )
            })
            .collect();
        all.sort_by_key(|(id, _, _)| *id);
        all
    }
}

async fn accept_loop(
    listener: TcpListener,
    target: Arc<Mutex<SocketAddr>>,
    cancel: CancellationToken,
    server_id: u8,
) {
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = cancel.cancelled() => return,
        };
        let (client, peer) = match accepted {
            Ok(accepted) => accepted,
            Err(e) => {
                tracing::warn!("game relay {server_id}: accept failed: {e}");
                continue;
            }
        };
        let real = *target.lock().unwrap_or_else(PoisonError::into_inner);
        let cancel = cancel.clone();
        tokio::spawn(async move {
            tracing::info!("game relay {server_id}: {peer} -> {real}");
            tokio::select! {
                result = relay(client, real) => {
                    if let Err(e) = result {
                        tracing::debug!("game relay {server_id} for {peer} ended: {e}");
                    }
                }
                () = cancel.cancelled() => {}
            }
        });
    }
}

async fn relay(mut client: TcpStream, real: SocketAddr) -> io::Result<()> {
    let mut server = TcpStream::connect(real).await?;
    client.set_nodelay(true)?;
    server.set_nodelay(true)?;
    copy_bidirectional(&mut client, &mut server).await?;
    Ok(())
}
