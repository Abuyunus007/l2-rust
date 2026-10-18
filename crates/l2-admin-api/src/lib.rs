//! Admin protocol between the GUI, the supervisor and the servers (D-30).
//!
//! Topology: GUI → supervisor (local or remote) → each server's local admin endpoint.
//! Transport: WebSocket, one JSON [`Message`] per text frame. The first message from the
//! client is [`Message::Hello`] with the shared token. See `docs/admin-api.md`.

pub mod auth;
pub mod messages;
pub mod transport;

pub use auth::Token;
pub use messages::*;
pub use transport::{AdminClient, AdminError, AdminHandler, Session, serve};
