//! Admin protocol between the GUI, the supervisor and the servers (D-30).
//!
//! Topology: GUI → supervisor (local or remote) → each server's local admin endpoint.
//! Transport: WebSocket, one JSON [`Message`] per text frame. The first message from the
//! client is [`Message::Hello`] with the shared token. See `docs/admin-api.md`.

pub mod auth;
pub mod messages;
pub mod transport;

pub use auth::Token;

/// The environment variable a supervisor sets for the servers it starts: the token of their admin
/// endpoint, used when `Admin.ini` has none (D-56). Its presence also turns the endpoint on.
pub const ADMIN_TOKEN_ENV: &str = "L2_ADMIN_TOKEN";
pub use messages::*;
pub use transport::{AdminClient, AdminError, AdminHandler, Session, serve};
