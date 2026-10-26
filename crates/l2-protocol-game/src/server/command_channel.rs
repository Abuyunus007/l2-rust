//! Server packets of the command channel: the window, the invitation and the party lists.
//!
//! Values only, like [`super::party`]: the world builds the lists from its parties.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExOpenMPCC.java
/// Opens the command channel window (`EX_OPEN_MPCC`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExOpenMpcc;

impl ExOpenMpcc {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExOpenMpcc;
}

impl Encode for ExOpenMpcc {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ExOpenMpcc {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExCloseMPCC.java
/// Closes the command channel window (`EX_CLOSE_MPCC`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCloseMpcc;

impl ExCloseMpcc {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExCloseMpcc;
}

impl Encode for ExCloseMpcc {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ExCloseMpcc {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExAskJoinMPCC.java
/// Asks a party leader to join the command channel of `requestor_name` (`EX_ASK_JOIN_MPCC`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExAskJoinMpcc {
    pub requestor_name: String,
}

impl ExAskJoinMpcc {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExAskJoinMpcc;
}

impl Encode for ExAskJoinMpcc {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.requestor_name);
    }
}

impl Decode for ExAskJoinMpcc {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            requestor_name: r.string(),
        })
    }
}

// Java: gameserver/network/serverpackets/ExMPCCPartyInfoUpdate.java
/// A party joined (`mode` 1) or left (`mode` 0) the channel (`EX_MPCC_PARTY_INFO_UPDATE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExMpccPartyInfoUpdate {
    /// The name of the party's leader.
    pub leader_name: String,
    pub leader_object_id: i32,
    pub member_count: i32,
    /// 0 = remove the party, 1 = add it.
    pub mode: i32,
}

impl ExMpccPartyInfoUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExMpccPartyInfoUpdate;
}

impl Encode for ExMpccPartyInfoUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.leader_name)
            .i32(self.leader_object_id)
            .i32(self.member_count)
            .i32(self.mode);
    }
}

impl Decode for ExMpccPartyInfoUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            leader_name: r.string(),
            leader_object_id: r.i32()?,
            member_count: r.i32()?,
            mode: r.i32()?,
        })
    }
}

/// One member of a party in [`ExMpccShowPartyMemberInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpccMember {
    pub name: String,
    pub object_id: i32,
    pub class_id: i32,
}

// Java: gameserver/network/serverpackets/ExMPCCShowPartyMemberInfo.java
/// The members of one party of the channel (`EX_MPCC_SHOW_PARTY_MEMBER_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExMpccShowPartyMemberInfo {
    pub members: Vec<MpccMember>,
}

impl ExMpccShowPartyMemberInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExMpccShowPartyMemberInfo;
}

impl Encode for ExMpccShowPartyMemberInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.members.len() as i32);
        for m in &self.members {
            w.string(&m.name).i32(m.object_id).i32(m.class_id);
        }
    }
}

impl Decode for ExMpccShowPartyMemberInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "member count")?;
        let mut members = Vec::new();
        for _ in 0..count {
            members.push(MpccMember {
                name: r.string(),
                object_id: r.i32()?,
                class_id: r.i32()?,
            });
        }
        Ok(Self { members })
    }
}

/// One party in [`ExMultiPartyCommandChannelInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpccPartyEntry {
    pub leader_name: String,
    pub leader_object_id: i32,
    pub member_count: i32,
}

// Java: gameserver/network/serverpackets/ExMultiPartyCommandChannelInfo.java
/// The channel window's content (`EX_MULTI_PARTY_COMMAND_CHANNEL_INFO`), the answer of the
/// `/channelinfo` user command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExMultiPartyCommandChannelInfo {
    pub leader_name: String,
    /// "Channel loot 0 or 1": Java always writes 0.
    pub channel_loot: i32,
    pub member_count: i32,
    pub parties: Vec<MpccPartyEntry>,
}

impl ExMultiPartyCommandChannelInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExMultiPartyCommandChannelInfo;
}

impl Encode for ExMultiPartyCommandChannelInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.leader_name)
            .i32(self.channel_loot)
            .i32(self.member_count)
            .i32(self.parties.len() as i32);
        for p in &self.parties {
            w.string(&p.leader_name)
                .i32(p.leader_object_id)
                .i32(p.member_count);
        }
    }
}

impl Decode for ExMultiPartyCommandChannelInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let leader_name = r.string();
        let channel_loot = r.i32()?;
        let member_count = r.i32()?;
        let count = read_count(r, "party count")?;
        let mut parties = Vec::new();
        for _ in 0..count {
            parties.push(MpccPartyEntry {
                leader_name: r.string(),
                leader_object_id: r.i32()?,
                member_count: r.i32()?,
            });
        }
        Ok(Self {
            leader_name,
            channel_loot,
            member_count,
            parties,
        })
    }
}
