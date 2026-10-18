//! WebSocket transport: [`serve`] on the listening side, [`AdminClient`] on the connecting side.
//!
//! This is server-process code, not game state: shared maps behind `std::sync::Mutex` are fine
//! here (D-10 only covers the world).

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::auth::Token;
use crate::messages::{Event, Message, PROTOCOL_VERSION, Request, Response};

/// How long a new connection may take to send its `Hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum AdminError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("connection rejected: {0}")]
    Rejected(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("connection closed")]
    Closed,
}

/// Handles requests on the listening side (supervisor or server).
pub trait AdminHandler: Send + Sync + 'static {
    /// Answer one request. May push events to the client through `session` (e.g. after
    /// `SubscribeLogs`). Requests run concurrently; responses carry the request id.
    fn handle(&self, request: Request, session: &Session) -> impl Future<Output = Response> + Send;
}

/// One authenticated connection, as seen by the handler.
#[derive(Clone, Debug)]
pub struct Session {
    id: u64,
    peer: SocketAddr,
    out: mpsc::UnboundedSender<Message>,
}

impl Session {
    /// Unique per listener, for keeping per-connection state (subscriptions).
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    /// Pushes an event to this client. Returns `false` once the connection is gone,
    /// which is the signal to drop a subscription.
    pub fn send_event(&self, event: Event) -> bool {
        self.out.send(Message::Event { event }).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.out.is_closed()
    }
}

/// Accepts admin connections on `listener` until the task is dropped.
///
/// `name` is sent in the `Welcome` message ("supervisor", "login", ...). Every client must
/// send a `Hello` with the right `token` and [`PROTOCOL_VERSION`] first, or it is rejected.
pub async fn serve<H: AdminHandler>(listener: TcpListener, name: String, token: Token, handler: Arc<H>) {
    let next_session = Arc::new(AtomicU64::new(1));
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                tracing::warn!("admin accept failed: {e}");
                continue;
            }
        };
        let session_id = next_session.fetch_add(1, Ordering::Relaxed);
        let (name, token, handler) = (name.clone(), token.clone(), Arc::clone(&handler));
        tokio::spawn(async move {
            if let Err(e) = serve_connection(stream, peer, session_id, name, token, handler).await {
                tracing::debug!("admin connection {peer} ended: {e}");
            }
        });
    }
}

async fn serve_connection<H: AdminHandler>(
    stream: TcpStream,
    peer: SocketAddr,
    session_id: u64,
    name: String,
    token: Token,
    handler: Arc<H>,
) -> Result<(), AdminError> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut source) = ws.split();

    // Handshake.
    let hello = tokio::time::timeout(HELLO_TIMEOUT, next_message(&mut source))
        .await
        .map_err(|_| AdminError::Protocol("no hello within timeout".into()))??;
    let rejection = match hello {
        Message::Hello { version, .. } if version != PROTOCOL_VERSION => Some(format!(
            "protocol version {version} not supported, expected {PROTOCOL_VERSION}"
        )),
        Message::Hello { token: given, .. } if !token.matches(&given) => Some("invalid token".to_string()),
        Message::Hello { .. } => None,
        other => Some(format!("expected hello, got {}", kind_of(&other))),
    };
    if let Some(reason) = rejection {
        tracing::warn!("admin connection from {peer} rejected: {reason}");
        send(
            &mut sink,
            &Message::Rejected {
                reason: reason.clone(),
            },
        )
        .await?;
        let _ = sink.close().await;
        return Err(AdminError::Rejected(reason));
    }
    send(
        &mut sink,
        &Message::Welcome {
            version: PROTOCOL_VERSION,
            name,
        },
    )
    .await?;
    tracing::info!("admin client {peer} connected");

    // Writer: everything for this client goes through one channel.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if send(&mut sink, &msg).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let session = Session {
        id: session_id,
        peer,
        out: out_tx,
    };
    let result = loop {
        let msg = match next_message(&mut source).await {
            Ok(msg) => msg,
            Err(AdminError::Closed) => break Ok(()),
            Err(e) => break Err(e),
        };
        match msg {
            Message::Request { id, request } => {
                let (handler, session) = (Arc::clone(&handler), session.clone());
                tokio::spawn(async move {
                    let response = handler.handle(request, &session).await;
                    let _ = session.out.send(Message::Response { id, response });
                });
            }
            other => tracing::warn!("admin client {peer} sent unexpected {}", kind_of(&other)),
        }
    };
    drop(session);
    writer.abort();
    tracing::info!("admin client {peer} disconnected");
    result
}

/// The connecting side (GUI → supervisor, supervisor → server).
#[derive(Debug)]
pub struct AdminClient {
    endpoint_name: String,
    out: mpsc::UnboundedSender<Message>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>>,
    next_id: AtomicU64,
}

impl AdminClient {
    /// Connects to `url` (`ws://host:port`) and authenticates. Returns the client and the
    /// stream of events pushed by the other side. The connection ends when both are dropped
    /// or the other side closes it.
    pub async fn connect(
        url: &str,
        token: &str,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Event>), AdminError> {
        let (ws, _) = tokio_tungstenite::connect_async(url).await?;
        let (mut sink, mut source) = ws.split();
        send(
            &mut sink,
            &Message::Hello {
                version: PROTOCOL_VERSION,
                token: token.to_string(),
            },
        )
        .await?;
        let endpoint_name = match tokio::time::timeout(HELLO_TIMEOUT, next_message(&mut source))
            .await
            .map_err(|_| AdminError::Protocol("no welcome within timeout".into()))??
        {
            Message::Welcome { name, .. } => name,
            Message::Rejected { reason } => return Err(AdminError::Rejected(reason)),
            other => {
                return Err(AdminError::Protocol(format!(
                    "expected welcome, got {}",
                    kind_of(&other)
                )));
            }
        };

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>> = Arc::default();
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                if send(&mut sink, &msg).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        let reader_pending = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Ok(msg) = next_message(&mut source).await {
                match msg {
                    Message::Response { id, response } => {
                        let waiter = reader_pending.lock().ok().and_then(|mut p| p.remove(&id));
                        if let Some(waiter) = waiter {
                            let _ = waiter.send(response);
                        }
                    }
                    Message::Event { event } => {
                        let _ = events_tx.send(event);
                    }
                    other => tracing::warn!("admin endpoint sent unexpected {}", kind_of(&other)),
                }
            }
            // Connection gone: dropping the waiters makes pending requests fail with Closed.
            if let Ok(mut p) = reader_pending.lock() {
                p.clear();
            }
        });

        let client = Self {
            endpoint_name,
            out: out_tx,
            pending,
            next_id: AtomicU64::new(1),
        };
        Ok((client, events_rx))
    }

    /// The name the endpoint sent in its `Welcome`.
    pub fn endpoint_name(&self) -> &str {
        &self.endpoint_name
    }

    /// Sends a request and waits for its response.
    pub async fn request(&self, request: Request) -> Result<Response, AdminError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| AdminError::Closed)?
            .insert(id, tx);
        if self.out.send(Message::Request { id, request }).is_err() {
            return Err(AdminError::Closed);
        }
        rx.await.map_err(|_| AdminError::Closed)
    }
}

async fn send<S>(sink: &mut S, msg: &Message) -> Result<(), AdminError>
where
    S: SinkExt<WsMessage, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = serde_json::to_string(msg)?;
    sink.send(WsMessage::text(json)).await?;
    Ok(())
}

/// Next protocol message; skips pings/pongs, fails on close or a non-text frame.
async fn next_message<S>(source: &mut S) -> Result<Message, AdminError>
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match source.next().await {
            None | Some(Ok(WsMessage::Close(_))) => return Err(AdminError::Closed),
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(WsMessage::Text(text))) => return Ok(serde_json::from_str(text.as_str())?),
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_))) => continue,
            Some(Ok(_)) => return Err(AdminError::Protocol("binary frames are not used".into())),
        }
    }
}

fn kind_of(msg: &Message) -> &'static str {
    match msg {
        Message::Hello { .. } => "hello",
        Message::Welcome { .. } => "welcome",
        Message::Rejected { .. } => "rejected",
        Message::Request { .. } => "request",
        Message::Response { .. } => "response",
        Message::Event { .. } => "event",
    }
}
