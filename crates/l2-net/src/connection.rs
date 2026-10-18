//! One task per TCP connection: framing, the cipher, the protocol handler and an outbox.
//!
//! Java: `commons/network/{ConnectionManager,Connection,Client}.java`, `handler/*`.
//!
//! Differences from Java, on purpose:
//! - Frames of one connection are handled **strictly in order**, and the handler finishes a
//!   frame before the next one is read or decrypted (D-10). Java runs packets on a pool, so two
//!   packets from one client can run at the same time. In-order handling also makes key changes
//!   safe: a handler that switches the cipher after frame N guarantees frame N+1 is decrypted
//!   with the new key.
//! - Sends from the handler (through [`Control::send`]) are encrypted immediately, with the
//!   cipher current at that moment, and written before the next frame is read.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::CancellationToken;

use crate::cipher::FrameCipher;
use crate::codec::FrameCodec;

/// Connection settings. Java: `Network.ini` (`DropPackets`, `DropPacketThreshold`) + `useNagle`.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Java `DropPackets`: drop *droppable* outgoing packets while the outbox is too full.
    pub drop_packets: bool,
    /// Java `DropPacketThreshold` (default 250): outbox size above which droppable packets go.
    pub drop_packet_threshold: usize,
    /// `TCP_NODELAY`. Java default `useNagle = false`, so `true` here.
    pub tcp_nodelay: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            drop_packets: false,
            drop_packet_threshold: 250,
            tcp_nodelay: true,
        }
    }
}

enum Outgoing {
    Frame { payload: Vec<u8>, close_after: bool },
    Close,
}

/// Lets any task send to a connection. Cheap to clone. Sends after the connection is gone
/// return `false` and do nothing.
#[derive(Clone)]
pub struct ConnectionHandle {
    tx: mpsc::UnboundedSender<Outgoing>,
    queued: Arc<AtomicUsize>,
    config: Arc<ConnectionConfig>,
    peer: SocketAddr,
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("peer", &self.peer)
            .finish_non_exhaustive()
    }
}

impl ConnectionHandle {
    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    /// Queues a plaintext payload; it's encrypted when written.
    pub fn send(&self, payload: Vec<u8>) -> bool {
        self.push(Outgoing::Frame {
            payload,
            close_after: false,
        })
    }

    /// Like [`ConnectionHandle::send`], but the packet may be dropped when `DropPackets` is on
    /// and the outbox holds more than `DropPacketThreshold` packets (Java
    /// `WritablePacket.canBeDropped`, e.g. movement or status spam). Returns `false` if dropped.
    pub fn send_droppable(&self, payload: Vec<u8>) -> bool {
        if self.config.drop_packets && self.queued.load(Ordering::Relaxed) > self.config.drop_packet_threshold
        {
            tracing::trace!("dropping packet to {}: outbox over threshold", self.peer);
            return false;
        }
        self.send(payload)
    }

    /// Sends the payload, then closes the connection (Java `close(packet)`).
    pub fn send_and_close(&self, payload: Vec<u8>) -> bool {
        self.push(Outgoing::Frame {
            payload,
            close_after: true,
        })
    }

    /// Closes after the packets already queued have been written.
    pub fn close(&self) {
        let _ = self.push(Outgoing::Close);
    }

    fn push(&self, out: Outgoing) -> bool {
        self.queued.fetch_add(1, Ordering::Relaxed);
        if self.tx.send(out).is_err() {
            self.queued.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
        true
    }
}

/// What a handler can do with its own connection while handling a frame.
pub struct Control<'a> {
    cipher: &'a mut Box<dyn FrameCipher>,
    pending: &'a mut Vec<Vec<u8>>,
    closing: &'a mut bool,
    handle: &'a ConnectionHandle,
}

impl Control<'_> {
    /// Encrypts with the current cipher now; written before the next frame is read.
    pub fn send(&mut self, payload: Vec<u8>) {
        let body = self.cipher.encrypt(payload);
        self.pending.push(body);
    }

    /// Sends this payload, then closes the connection.
    pub fn send_and_close(&mut self, payload: Vec<u8>) {
        self.send(payload);
        *self.closing = true;
    }

    /// Closes after pending sends are written. No more frames are handled.
    pub fn close(&mut self) {
        *self.closing = true;
    }

    /// Replaces the cipher. Applies to every frame read and every send after this call.
    pub fn set_cipher(&mut self, cipher: Box<dyn FrameCipher>) {
        *self.cipher = cipher;
    }

    /// A handle for other tasks (store it in shared state to reach this connection later).
    pub fn handle(&self) -> &ConnectionHandle {
        self.handle
    }

    pub fn peer(&self) -> SocketAddr {
        self.handle.peer
    }
}

/// Protocol logic for one connection. Frames arrive decrypted, in order, one at a time.
pub trait FrameHandler: Send + 'static {
    /// Called once before the first read (e.g. the login server sends `Init` here).
    fn on_connect(&mut self, ctl: &mut Control<'_>) -> impl Future<Output = ()> + Send {
        let _ = ctl;
        async {}
    }

    /// One decrypted frame body.
    fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) -> impl Future<Output = ()> + Send;

    /// Called once when the connection has ended, for whatever reason.
    fn on_disconnect(&mut self) -> impl Future<Output = ()> + Send {
        async {}
    }
}

/// Starts the connection task and returns its handle right away.
pub fn spawn_connection<H: FrameHandler>(
    stream: TcpStream,
    config: Arc<ConnectionConfig>,
    cipher: Box<dyn FrameCipher>,
    handler: H,
    cancel: CancellationToken,
) -> io::Result<ConnectionHandle> {
    stream.set_nodelay(config.tcp_nodelay)?;
    let peer = stream.peer_addr()?;
    let (tx, rx) = mpsc::unbounded_channel();
    let handle = ConnectionHandle {
        tx,
        queued: Arc::new(AtomicUsize::new(0)),
        config,
        peer,
    };
    let task_handle = handle.clone();
    tokio::spawn(async move {
        if let Err(e) = run(stream, cipher, handler, task_handle, rx, cancel).await {
            tracing::debug!("connection {peer} ended with error: {e}");
        }
    });
    Ok(handle)
}

async fn run<H: FrameHandler>(
    stream: TcpStream,
    mut cipher: Box<dyn FrameCipher>,
    mut handler: H,
    handle: ConnectionHandle,
    mut rx: mpsc::UnboundedReceiver<Outgoing>,
    cancel: CancellationToken,
) -> io::Result<()> {
    let peer = handle.peer;
    let (read_half, write_half) = stream.into_split();
    let mut reader = FramedRead::new(read_half, FrameCodec);
    let mut writer = FramedWrite::new(write_half, FrameCodec);
    let mut pending: Vec<Vec<u8>> = Vec::new();
    let mut closing = false;

    {
        let mut ctl = Control {
            cipher: &mut cipher,
            pending: &mut pending,
            closing: &mut closing,
            handle: &handle,
        };
        handler.on_connect(&mut ctl).await;
    }
    let mut result = flush(&mut writer, &mut pending).await;

    while result.is_ok() && !closing {
        tokio::select! {
            frame = reader.next() => match frame {
                Some(Ok(body)) => {
                    let mut body = body.to_vec();
                    if let Err(e) = cipher.decrypt(&mut body) {
                        tracing::debug!("closing {peer}: {e}");
                        break;
                    }
                    let mut ctl = Control {
                        cipher: &mut cipher,
                        pending: &mut pending,
                        closing: &mut closing,
                        handle: &handle,
                    };
                    handler.on_frame(body, &mut ctl).await;
                    result = flush(&mut writer, &mut pending).await;
                }
                Some(Err(e)) => result = Err(e),
                None => break,
            },
            out = rx.recv() => match out {
                Some(Outgoing::Frame { payload, close_after }) => {
                    handle.queued.fetch_sub(1, Ordering::Relaxed);
                    result = writer.send(cipher.encrypt(payload)).await;
                    closing |= close_after;
                }
                Some(Outgoing::Close) | None => {
                    handle.queued.fetch_sub(1, Ordering::Relaxed);
                    closing = true;
                }
            },
            () = cancel.cancelled() => break,
        }
    }

    // Stop accepting new sends, then end the connection cleanly.
    rx.close();
    let _ = writer.close().await;
    handler.on_disconnect().await;
    result
}

async fn flush(
    writer: &mut FramedWrite<tokio::net::tcp::OwnedWriteHalf, FrameCodec>,
    pending: &mut Vec<Vec<u8>>,
) -> io::Result<()> {
    for body in pending.drain(..) {
        writer.feed(body).await?;
    }
    writer.flush().await
}

/// Accepts connections until `cancel` fires. For each one, `factory` decides: `None` closes it
/// right away (e.g. a banned IP), `Some((handler, cipher))` starts a connection task.
pub async fn listen<H, F>(
    listener: TcpListener,
    config: Arc<ConnectionConfig>,
    cancel: CancellationToken,
    mut factory: F,
) where
    H: FrameHandler,
    F: FnMut(SocketAddr) -> Option<(H, Box<dyn FrameCipher>)>,
{
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = cancel.cancelled() => return,
        };
        let (stream, peer) = match accepted {
            Ok(accepted) => accepted,
            Err(e) => {
                tracing::warn!("accept failed: {e}");
                continue;
            }
        };
        let Some((handler, cipher)) = factory(peer) else {
            tracing::debug!("connection from {peer} refused");
            continue;
        };
        if let Err(e) = spawn_connection(stream, Arc::clone(&config), cipher, handler, cancel.child_token()) {
            tracing::warn!("could not start connection from {peer}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(drop_packets: bool, threshold: usize) -> (ConnectionHandle, mpsc::UnboundedReceiver<Outgoing>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let config = Arc::new(ConnectionConfig {
            drop_packets,
            drop_packet_threshold: threshold,
            tcp_nodelay: true,
        });
        let peer = SocketAddr::from(([127, 0, 0, 1], 1));
        (
            ConnectionHandle {
                tx,
                queued: Arc::new(AtomicUsize::new(0)),
                config,
                peer,
            },
            rx,
        )
    }

    #[test]
    fn droppable_packets_are_dropped_over_threshold() {
        let (h, _rx) = handle(true, 2);
        assert!(h.send_droppable(vec![1]));
        assert!(h.send_droppable(vec![2]));
        assert!(h.send_droppable(vec![3])); // queue was 2, not > 2
        assert!(!h.send_droppable(vec![4])); // queue is 3 > 2: dropped
        assert!(h.send(vec![5]), "normal packets are never dropped");
    }

    #[test]
    fn nothing_is_dropped_when_disabled() {
        let (h, _rx) = handle(false, 0);
        for i in 0..10 {
            assert!(h.send_droppable(vec![i]));
        }
    }

    #[test]
    fn sends_fail_after_the_connection_is_gone() {
        let (h, rx) = handle(false, 0);
        drop(rx);
        assert!(!h.send(vec![1]));
        assert!(h.is_closed());
        assert_eq!(
            h.queued.load(Ordering::Relaxed),
            0,
            "failed sends don't count as queued"
        );
    }
}
