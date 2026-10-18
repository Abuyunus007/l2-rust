//! The traits every packet struct implements.
//!
//! **Pattern for all packet structs** (login, link and ~700 game packets copy it):
//! one plain struct per packet, `impl Encode` for packets we send, `impl Decode` for packets we
//! receive, and a `// Java:` line naming the class it was ported from.
//!
//! ```
//! use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};
//!
//! // Java: loginserver/network/serverpackets/PlayOk.java
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub struct PlayOk {
//!     pub play_ok1: i32,
//!     pub play_ok2: i32,
//! }
//!
//! impl PlayOk {
//!     pub const OPCODE: u8 = 0x07;
//! }
//!
//! impl Encode for PlayOk {
//!     fn encode(&self, w: &mut PacketWriter) {
//!         w.u8(Self::OPCODE).i32(self.play_ok1).i32(self.play_ok2);
//!     }
//! }
//!
//! // Decoding starts *after* the opcode: the dispatcher already read it (Java `readImpl`).
//! impl Decode for PlayOk {
//!     fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
//!         Ok(Self { play_ok1: r.i32()?, play_ok2: r.i32()? })
//!     }
//! }
//!
//! let packet = PlayOk { play_ok1: 1, play_ok2: -1 };
//! let bytes = packet.to_bytes();
//! assert_eq!(bytes[0], PlayOk::OPCODE);
//! let decoded = PlayOk::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
//! assert_eq!(decoded, packet);
//! ```

use crate::error::PacketError;
use crate::reader::PacketReader;
use crate::writer::PacketWriter;

/// A packet we send. `encode` writes the opcode(s) followed by the fields (Java `writeImpl`).
pub trait Encode {
    fn encode(&self, w: &mut PacketWriter);

    /// The plaintext payload: opcode + fields, no length header, no encryption.
    fn to_bytes(&self) -> Vec<u8> {
        let mut w = PacketWriter::new();
        self.encode(&mut w);
        w.into_bytes()
    }
}

/// A packet we receive. `decode` reads the fields **after** the opcode(s), which the dispatcher
/// has already consumed (Java `readImpl`). Trailing bytes are allowed (clients pad packets).
pub trait Decode: Sized {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError>;
}
