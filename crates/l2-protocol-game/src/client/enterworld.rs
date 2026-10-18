//! Client packets sent while the character enters the world.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

// Java: gameserver/network/clientpackets/EnterWorld.java
/// The "enter world" packet. Most of its fields are unknown to Java as well: it reads and
/// ignores them. Only the trace route is used, Java passes it to the login server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterWorld {
    pub unknown1: [u8; 32],
    pub unknown2: [i32; 4],
    pub unknown3: [u8; 32],
    pub unknown4: i32,
    /// Five hops of four unsigned bytes each (Java `_tracert`).
    pub tracert: [[u8; 4]; 5],
}

impl EnterWorld {
    pub const OPCODE: ClientOpcode = ClientOpcode::EnterWorld;
    /// Body length after the opcode byte: 32 + 4 * 4 + 32 + 4 + 5 * 4.
    pub const BODY_LEN: usize = 104;
}

impl Encode for EnterWorld {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.bytes(&self.unknown1);
        for value in self.unknown2 {
            w.i32(value);
        }
        w.bytes(&self.unknown3).i32(self.unknown4);
        for hop in &self.tracert {
            w.bytes(hop);
        }
    }
}

impl Decode for EnterWorld {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let unknown1 = r.byte_array::<32>()?;
        let unknown2 = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];
        let unknown3 = r.byte_array::<32>()?;
        let unknown4 = r.i32()?;
        let mut tracert = [[0; 4]; 5];
        for hop in &mut tracert {
            *hop = r.byte_array::<4>()?;
        }
        Ok(Self {
            unknown1,
            unknown2,
            unknown3,
            unknown4,
            tracert,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestKeyMapping.java
/// Asks for the stored UI key mapping (ex packet `0xD0 0x0021`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestKeyMapping;

impl RequestKeyMapping {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestKeyMapping;
}

impl Encode for RequestKeyMapping {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
    }
}

impl Decode for RequestKeyMapping {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}
