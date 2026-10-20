//! Client packets of the party: invite, answer, leave, expel and the leader change.
//!
//! These only read the packet. The checks (banned, event, olympiad, blocked, full party ...) and
//! the party changes are the world's job (M6 party cards).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

// Java: gameserver/network/clientpackets/RequestJoinParty.java
/// The player invites another player into its party (`REQUEST_JOIN_PARTY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestJoinParty {
    /// The name of the invited player.
    pub name: String,
    /// The loot rule chosen in the invite dialog. Kept as the raw int: Java ignores the request
    /// (when a new party would be created) if no `PartyDistributionType` has this id, so the
    /// world decides with `PartyDistributionType::from_id`.
    pub party_distribution_type_id: i32,
}

impl RequestJoinParty {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestJoinParty;
}

impl Encode for RequestJoinParty {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name).i32(self.party_distribution_type_id);
    }
}

impl Decode for RequestJoinParty {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            name: r.string(),
            party_distribution_type_id: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestAnswerJoinParty.java
/// The invited player answers (`REQUEST_ANSWER_JOIN_PARTY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAnswerJoinParty {
    /// 1 accept, 0 decline, -1 the player's client refuses party requests. Java acts on those
    /// three values only.
    pub response: i32,
}

impl RequestAnswerJoinParty {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestAnswerJoinParty;
}

impl Encode for RequestAnswerJoinParty {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.response);
    }
}

impl Decode for RequestAnswerJoinParty {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { response: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/RequestWithDrawalParty.java
/// The player leaves its party (`REQUEST_WITH_DRAWAL_PARTY`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestWithDrawalParty;

impl RequestWithDrawalParty {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestWithDrawalParty;
}

impl Encode for RequestWithDrawalParty {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for RequestWithDrawalParty {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestOustPartyMember.java
/// The leader expels a member (`REQUEST_OUST_PARTY_MEMBER`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestOustPartyMember {
    pub name: String,
}

impl RequestOustPartyMember {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestOustPartyMember;
}

impl Encode for RequestOustPartyMember {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name);
    }
}

impl Decode for RequestOustPartyMember {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { name: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestChangePartyLeader.java
/// The leader hands the party over to another member (`REQUEST_CHANGE_PARTY_LEADER`). Ex packet
/// `0xD0 0x000C`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestChangePartyLeader {
    pub name: String,
}

impl RequestChangePartyLeader {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestChangePartyLeader;
}

impl Encode for RequestChangePartyLeader {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.name);
    }
}

impl Decode for RequestChangePartyLeader {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { name: r.string() })
    }
}
