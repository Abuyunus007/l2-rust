//! Client packets of clans (pledges), clan wars and alliances.
//!
//! These only read the packet. Every rule (leader only, clan level, reputation, dissolving,
//! cooldowns ...) is the world's job (the M6 clan cards). Most packets are a few ints and
//! strings, declared with the [`client_packet!`] macro of [`super`]; the crest uploads and
//! `RequestPledgePower` have a body that depends on a value and are written out.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

// ---------------------------------------------------------------------------------------------
// Joining and leaving a clan
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/clientpackets/RequestJoinPledge.java
client_packet!(
    /// A clan member invites a player (`REQUEST_JOIN_PLEDGE`). `pledge_type` is the unit: 0 main
    /// clan, -1 academy, 100/200 knights, 1001... royal guards.
    RequestJoinPledge,
    plain RequestJoinPledge {
        /// The object id of the invited player.
        target: i32,
        pledge_type: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestAnswerJoinPledge.java
client_packet!(
    /// The invited player answers (`REQUEST_ANSWER_JOIN_PLEDGE`): `1` accepts, anything else
    /// declines.
    RequestAnswerJoinPledge,
    plain RequestAnswerJoinPledge {
        answer: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestWithdrawalPledge.java
client_unit_packet!(
    /// The player leaves its clan (`REQUEST_WITHDRAWAL_PLEDGE`).
    RequestWithdrawalPledge,
    plain RequestWithdrawalPledge
);

// Java: gameserver/network/clientpackets/RequestOustPledgeMember.java
client_packet!(
    /// A clan member with the privilege expels another one (`REQUEST_OUST_PLEDGE_MEMBER`).
    RequestOustPledgeMember,
    plain RequestOustPledgeMember {
        target: String,
    }
);

// ---------------------------------------------------------------------------------------------
// Clan information windows
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/clientpackets/RequestPledgeMemberList.java
client_unit_packet!(
    /// Opens the clan window's member list (`REQUEST_PLEDGE_MEMBER_LIST`).
    RequestPledgeMemberList,
    plain RequestPledgeMemberList
);

// Java: gameserver/network/clientpackets/RequestPledgeInfo.java
client_packet!(
    /// Asks for the name of a clan (`REQUEST_PLEDGE_INFO`).
    RequestPledgeInfo,
    plain RequestPledgeInfo {
        clan_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgeExtendedInfo.java
client_packet!(
    /// Asks for the extended info of a clan (`REQUEST_PLEDGE_EXTENDED_INFO`). Java reads the
    /// string and ignores it (`readString(); // name?`).
    RequestPledgeExtendedInfo,
    plain RequestPledgeExtendedInfo {
        name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgeCrest.java
client_packet!(
    /// Asks for a clan crest image (`REQUEST_PLEDGE_CREST`).
    RequestPledgeCrest,
    plain RequestPledgeCrest {
        crest_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestExPledgeCrestLarge.java
client_packet!(
    /// Asks for a large clan crest (`REQUEST_EX_PLEDGE_CREST_LARGE`). Ex packet `0xD0 0x0010`.
    RequestExPledgeCrestLarge,
    ex RequestExPledgeCrestLarge {
        crest_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgePowerGradeList.java
client_unit_packet!(
    /// Asks for the privileges of the ranks (`REQUEST_PLEDGE_POWER_GRADE_LIST`). Ex packet
    /// `0xD0 0x0013`.
    RequestPledgePowerGradeList,
    ex RequestPledgePowerGradeList
);

// Java: gameserver/network/clientpackets/RequestPledgeMemberPowerInfo.java
client_packet!(
    /// Asks for the rank and privileges of a member (`REQUEST_PLEDGE_MEMBER_POWER_INFO`). Ex
    /// packet `0xD0 0x0014`.
    RequestPledgeMemberPowerInfo,
    ex RequestPledgeMemberPowerInfo {
        /// Java `_unk1`.
        unk1: i32,
        player: String,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgeMemberInfo.java
client_packet!(
    /// Asks for the details of a member (`REQUEST_PLEDGE_MEMBER_INFO`). Ex packet
    /// `0xD0 0x0016`.
    RequestPledgeMemberInfo,
    ex RequestPledgeMemberInfo {
        /// Java `_unk1`.
        unk1: i32,
        player: String,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgeWarList.java
client_packet!(
    /// Asks for the war list of the clan (`REQUEST_PLEDGE_WAR_LIST`). Ex packet `0xD0 0x0017`.
    RequestPledgeWarList,
    ex RequestPledgeWarList {
        /// Java `_unk1`.
        unk1: i32,
        /// 0 declared wars, 1 under attack.
        tab: i32,
    }
);

// ---------------------------------------------------------------------------------------------
// Clan management
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/clientpackets/RequestPledgeSetAcademyMaster.java
client_packet!(
    /// Sets or removes the sponsor of an academy member (`REQUEST_PLEDGE_SET_ACADEMY_MASTER`).
    /// Ex packet `0xD0 0x0012`.
    RequestPledgeSetAcademyMaster,
    ex RequestPledgeSetAcademyMaster {
        /// Java `_set`: 1 sets the sponsor, 0 removes it.
        set: i32,
        curr_player_name: String,
        target_player_name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgeSetMemberPowerGrade.java
client_packet!(
    /// Sets the rank of a member (`REQUEST_PLEDGE_SET_MEMBER_POWER_GRADE`). Ex packet
    /// `0xD0 0x0015`.
    RequestPledgeSetMemberPowerGrade,
    ex RequestPledgeSetMemberPowerGrade {
        member: String,
        power_grade: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgeReorganizeMember.java
client_packet!(
    /// Moves a member to another unit (`REQUEST_PLEDGE_REORGANIZE_MEMBER`). Ex packet
    /// `0xD0 0x002C`.
    RequestPledgeReorganizeMember,
    ex RequestPledgeReorganizeMember {
        is_member_selected: i32,
        member_name: String,
        new_pledge_type: i32,
        selected_member: String,
    }
);

// Java: gameserver/network/clientpackets/RequestPledgePower.java
/// Reads or sets the privileges of a rank (`REQUEST_PLEDGE_POWER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestPledgePower {
    pub rank: i32,
    /// 1 shows the privileges of the rank, 2 sets them.
    pub action: i32,
    /// Only on the wire when `action` is 2; 0 otherwise.
    pub privs: i32,
}

impl RequestPledgePower {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestPledgePower;
    /// Java `_action == 2`.
    pub const ACTION_SET: i32 = 2;
}

impl Encode for RequestPledgePower {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.rank).i32(self.action);
        if self.action == Self::ACTION_SET {
            w.i32(self.privs);
        }
    }
}

impl Decode for RequestPledgePower {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let rank = r.i32()?;
        let action = r.i32()?;
        let privs = if action == Self::ACTION_SET { r.i32()? } else { 0 };
        Ok(Self { rank, action, privs })
    }
}

// ---------------------------------------------------------------------------------------------
// Crest uploads
// ---------------------------------------------------------------------------------------------

/// Java's crest upload read: an int length, and the bytes unless the length is above the limit.
/// A negative length, or a length beyond the packet, is Java's `BufferUnderflowException`
/// (the packet is dropped), so it is a decode error here.
fn read_crest(r: &mut PacketReader, max_length: i32) -> Result<(i32, Option<Vec<u8>>), PacketError> {
    let length = r.i32()?;
    if length > max_length {
        return Ok((length, None));
    }
    let len = usize::try_from(length)
        .map_err(|_| PacketError::invalid("crest length", format!("negative length {length}")))?;
    Ok((length, Some(r.bytes(len)?.to_vec())))
}

fn write_crest(w: &mut PacketWriter, length: i32, data: &Option<Vec<u8>>) {
    w.i32(length);
    if let Some(data) = data {
        w.bytes(data);
    }
}

// Java: gameserver/network/clientpackets/RequestSetPledgeCrest.java
/// The leader uploads a clan crest (`REQUEST_SET_PLEDGE_CREST`).
///
/// NOTE(java-quirk): when the length is above [`Self::MAX_LENGTH`] Java stops reading and the
/// handler answers with a size message; `data` is `None` then. The handler's `_length < 0` check
/// can never run, because `readBytes` throws for a negative length first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSetPledgeCrest {
    pub length: i32,
    pub data: Option<Vec<u8>>,
}

impl RequestSetPledgeCrest {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestSetPledgeCrest;
    pub const MAX_LENGTH: i32 = 256;
}

impl Encode for RequestSetPledgeCrest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_crest(w, self.length, &self.data);
    }
}

impl Decode for RequestSetPledgeCrest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (length, data) = read_crest(r, Self::MAX_LENGTH)?;
        Ok(Self { length, data })
    }
}

// Java: gameserver/network/clientpackets/RequestExSetPledgeCrestLarge.java
/// The leader uploads a large clan crest (`REQUEST_EX_SET_PLEDGE_CREST_LARGE`). Ex packet
/// `0xD0 0x0011`. Same rules as [`RequestSetPledgeCrest`], limit [`Self::MAX_LENGTH`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestExSetPledgeCrestLarge {
    pub length: i32,
    pub data: Option<Vec<u8>>,
}

impl RequestExSetPledgeCrestLarge {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExSetPledgeCrestLarge;
    pub const MAX_LENGTH: i32 = 2176;
}

impl Encode for RequestExSetPledgeCrestLarge {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        write_crest(w, self.length, &self.data);
    }
}

impl Decode for RequestExSetPledgeCrestLarge {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (length, data) = read_crest(r, Self::MAX_LENGTH)?;
        Ok(Self { length, data })
    }
}

// Java: gameserver/network/clientpackets/RequestSetAllyCrest.java
/// The alliance leader uploads an alliance crest (`REQUEST_SET_ALLY_CREST`). Same rules as
/// [`RequestSetPledgeCrest`], limit [`Self::MAX_LENGTH`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSetAllyCrest {
    pub length: i32,
    pub data: Option<Vec<u8>>,
}

impl RequestSetAllyCrest {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestSetAllyCrest;
    pub const MAX_LENGTH: i32 = 192;
}

impl Encode for RequestSetAllyCrest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_crest(w, self.length, &self.data);
    }
}

impl Decode for RequestSetAllyCrest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (length, data) = read_crest(r, Self::MAX_LENGTH)?;
        Ok(Self { length, data })
    }
}

// ---------------------------------------------------------------------------------------------
// Clan wars
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/clientpackets/RequestStartPledgeWar.java
client_packet!(
    /// The leader declares war on a clan (`REQUEST_START_PLEDGE_WAR`).
    RequestStartPledgeWar,
    plain RequestStartPledgeWar {
        pledge_name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestReplyStartPledgeWar.java
client_packet!(
    /// The other leader answers a war declaration (`REQUEST_REPLY_START_PLEDGE`).
    RequestReplyStartPledgeWar,
    plain RequestReplyStartPledge {
        /// Java reads it and ignores it (`readString(); // _reqName`).
        req_name: String,
        answer: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestStopPledgeWar.java
client_packet!(
    /// The leader asks to end a war (`REQUEST_STOP_PLEDGE_WAR`).
    RequestStopPledgeWar,
    plain RequestStopPledgeWar {
        pledge_name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestReplyStopPledgeWar.java
client_packet!(
    /// The other leader answers a request to end the war (`REQUEST_REPLY_STOP_PLEDGE_WAR`).
    RequestReplyStopPledgeWar,
    plain RequestReplyStopPledgeWar {
        /// Java reads it and ignores it (`readString(); // _reqName`).
        req_name: String,
        answer: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestSurrenderPledgeWar.java
client_packet!(
    /// The leader surrenders in a war (`REQUEST_SURRENDER_PLEDGE_WAR`).
    RequestSurrenderPledgeWar,
    plain RequestSurrenderPledgeWar {
        pledge_name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestReplySurrenderPledgeWar.java
client_packet!(
    /// The other leader answers a surrender (`REQUEST_REPLY_SURRENDER_PLEDGE_WAR`).
    RequestReplySurrenderPledgeWar,
    plain RequestReplySurrenderPledgeWar {
        req_name: String,
        answer: i32,
    }
);

// ---------------------------------------------------------------------------------------------
// Alliances
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/clientpackets/RequestJoinAlly.java
client_packet!(
    /// The alliance leader invites a clan leader (`REQUEST_JOIN_ALLY`).
    RequestJoinAlly,
    plain RequestJoinAlly {
        /// The object id of the invited clan leader.
        id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestAnswerJoinAlly.java
client_packet!(
    /// The invited clan leader answers (`REQUEST_ANSWER_JOIN_ALLY`): `1` accepts.
    RequestAnswerJoinAlly,
    plain RequestAnswerJoinAlly {
        response: i32,
    }
);

// Java: gameserver/network/clientpackets/AllyLeave.java
client_unit_packet!(
    /// A clan leaves its alliance (`ALLY_LEAVE`).
    AllyLeave,
    plain AllyLeave
);

// Java: gameserver/network/clientpackets/AllyDismiss.java
client_packet!(
    /// The alliance leader expels a clan (`ALLY_DISMISS`).
    AllyDismiss,
    plain AllyDismiss {
        clan_name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestDismissAlly.java
client_unit_packet!(
    /// The alliance leader dissolves the alliance (`REQUEST_DISMISS_ALLY`).
    RequestDismissAlly,
    plain RequestDismissAlly
);

// Java: gameserver/network/clientpackets/RequestAllyCrest.java
client_packet!(
    /// Asks for an alliance crest image (`REQUEST_ALLY_CREST`).
    RequestAllyCrest,
    plain RequestAllyCrest {
        crest_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestAllyInfo.java
client_unit_packet!(
    /// Asks for the alliance info window (`REQUEST_ALLY_INFO`).
    RequestAllyInfo,
    plain RequestAllyInfo
);
