//! Network building blocks shared by every server, the proxy and the tools.
//!
//! - [`PacketReader`] / [`PacketWriter`]: little-endian primitives and L2 strings.
//! - [`Encode`] / [`Decode`]: the traits every packet struct implements.
//! - [`FrameCodec`]: `u16 LE` length framing.
//! - [`spawn_connection`] / [`listen`]: one task per connection with a [`FrameCipher`], a
//!   [`FrameHandler`] and a [`ConnectionHandle`] outbox.

pub mod cipher;
pub mod codec;
pub mod connection;
pub mod error;
pub mod packet;
pub mod reader;
pub mod writer;

pub use cipher::{CipherError, FrameCipher, NoCipher};
pub use codec::{FrameCodec, HEADER_SIZE, MAX_PAYLOAD};
pub use connection::{ConnectionConfig, ConnectionHandle, Control, FrameHandler, listen, spawn_connection};
pub use error::PacketError;
pub use packet::{Decode, Encode};
pub use reader::PacketReader;
pub use writer::PacketWriter;

/// Re-exported so users don't need a direct `tokio-util` dependency for shutdown.
pub use tokio_util::sync::CancellationToken;
