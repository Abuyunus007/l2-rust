//! Client packets of the Seven Signs status and the Seed of Destruction / Infinity map.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/RequestSSQStatus.java
/// The player asks for one page of the Seven Signs status (`0xC8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestSsqStatus {
    /// Java `_page` (`readByte()`, so signed).
    pub page: i8,
}

impl RequestSsqStatus {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestSsqStatus;
}

impl Encode for RequestSsqStatus {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i8(self.page);
    }
}

impl Decode for RequestSsqStatus {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { page: r.i8()? })
    }
}

// Java: gameserver/network/clientpackets/RequestSeedPhase.java
client_unit_packet!(
    /// The player asks for the Seed map (`0xD0` 0x0063). Java answers with `ExShowSeedMapInfo`.
    RequestSeedPhase,
    ex RequestSeedPhase
);
