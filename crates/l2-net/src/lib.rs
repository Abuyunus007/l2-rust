//! Network building blocks shared by every server, the proxy and the tools.
//!
//! - [`PacketReader`] / [`PacketWriter`]: little-endian primitives and L2 strings.
//! - [`Encode`] / [`Decode`]: the traits every packet struct implements.

pub mod error;
pub mod packet;
pub mod reader;
pub mod writer;

pub use error::PacketError;
pub use packet::{Decode, Encode};
pub use reader::PacketReader;
pub use writer::PacketWriter;
