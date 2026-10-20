//! Server packets of clans (pledges), clan wars and alliances.
//!
//! Same rules as [`super::skills`]: values only. The clan, its members and its sub pledges come
//! from the world; what Java reads from `Clan`, `ClanMember` and `Player` is a plain field here.
//! Packets that are only ints and strings use the [`server_packet!`] macro of [`super`].

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

// ---------------------------------------------------------------------------------------------
// Joining a clan
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/serverpackets/AskJoinPledge.java
/// A player is invited into a clan (`ASK_JOIN_PLEDGE`).
///
/// NOTE(java-quirk): the layout depends on the values. The sub pledge name is written only when
/// it is not null, as the sub pledge name when `pledge_type > 0` and as the clan name otherwise;
/// the type is written only when it is not 0; the clan name always ends the packet. Decoding has
/// no marker to go by, so it tries the shapes Java can produce and takes the one that uses every
/// byte, encodes back to the same bytes and has a pledge type the game knows. A sub pledge name with `pledge_type <= 0` is not
/// kept on the wire (the clan name replaces it), so it decodes as the clan name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskJoinPledge {
    pub requestor_object_id: i32,
    /// `None` when the clan has no sub pledge of this type.
    pub sub_pledge_name: Option<String>,
    pub pledge_type: i32,
    pub pledge_name: String,
}

impl AskJoinPledge {
    pub const OPCODE: ServerOpcode = ServerOpcode::AskJoinPledge;
}

/// One way [`AskJoinPledge`] can be laid out on the wire.
type AskJoinPledgeLayout = fn(&[u8]) -> Result<AskJoinPledge, PacketError>;

/// Java `Clan.SUBUNIT_*`: the academy, the two royal guards and the four knight units.
const KNOWN_PLEDGE_TYPES: [i32; 7] = [-1, 100, 200, 1001, 1002, 2001, 2002];

impl Encode for AskJoinPledge {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.requestor_object_id);
        if let Some(sub) = &self.sub_pledge_name {
            w.string(if self.pledge_type > 0 {
                sub
            } else {
                &self.pledge_name
            });
        }
        if self.pledge_type != 0 {
            w.i32(self.pledge_type);
        }
        w.string(&self.pledge_name);
    }
}

impl Decode for AskJoinPledge {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let rest = r.rest();
        let candidates: [AskJoinPledgeLayout; 3] = [
            // [requestor][clan name]
            |bytes| {
                let mut r = PacketReader::new(bytes);
                let requestor_object_id = r.i32()?;
                Ok(AskJoinPledge {
                    requestor_object_id,
                    sub_pledge_name: None,
                    pledge_type: 0,
                    pledge_name: r.string(),
                })
            },
            // [requestor][sub or clan name][type][clan name]
            |bytes| {
                let mut r = PacketReader::new(bytes);
                let requestor_object_id = r.i32()?;
                let first = r.string();
                let pledge_type = r.i32()?;
                Ok(AskJoinPledge {
                    requestor_object_id,
                    sub_pledge_name: Some(first),
                    pledge_type,
                    pledge_name: r.string(),
                })
            },
            // [requestor][type][clan name]
            |bytes| {
                let mut r = PacketReader::new(bytes);
                let requestor_object_id = r.i32()?;
                let pledge_type = r.i32()?;
                Ok(AskJoinPledge {
                    requestor_object_id,
                    sub_pledge_name: None,
                    pledge_type,
                    pledge_name: r.string(),
                })
            },
        ];
        for parse in candidates {
            if let Ok(packet) = parse(rest) {
                // A layout fits when it uses every byte, writes back the same bytes (the encoder
                // writes the opcode first) and carries a pledge type the game knows. The last
                // test keeps a clan name's first characters from passing as a type.
                let known_type = packet.pledge_type == 0 || KNOWN_PLEDGE_TYPES.contains(&packet.pledge_type);
                if known_type && packet.to_bytes().get(1..) == Some(rest) {
                    r.skip(rest.len())?;
                    return Ok(packet);
                }
            }
        }
        Err(PacketError::invalid(
            "ask join pledge",
            "no layout of the packet fits the bytes".to_owned(),
        ))
    }
}

server_packet!(
    /// The invited player joined the clan (`JOIN_PLEDGE`).
    JoinPledge,
    JoinPledge {
        pledge_id: i32,
    }
);

// ---------------------------------------------------------------------------------------------
// Clan info and crests
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/serverpackets/PledgeInfo.java
server_packet!(
    /// The name of a clan and of its alliance, the answer to `RequestPledgeInfo`
    /// (`PLEDGE_INFO`).
    PledgeInfo,
    PledgeInfo {
        clan_id: i32,
        clan_name: String,
        ally_name: String,
    }
);

/// Reads an int length and that many bytes, the way Java's crest packets write them. Java writes
/// `0` for a missing crest, which reads back as an empty crest.
fn read_crest_data(r: &mut PacketReader) -> Result<Vec<u8>, PacketError> {
    let length = read_count(r, "crest length")?;
    Ok(r.bytes(length)?.to_vec())
}

fn write_crest_data(w: &mut PacketWriter, data: &[u8]) {
    w.i32(data.len() as i32).bytes(data);
}

// Java: gameserver/network/serverpackets/PledgeCrest.java
/// A clan crest image (`PLEDGE_CREST`). `data` is empty when the crest does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeCrest {
    pub crest_id: i32,
    pub data: Vec<u8>,
}

impl PledgeCrest {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeCrest;
}

impl Encode for PledgeCrest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.crest_id);
        write_crest_data(w, &self.data);
    }
}

impl Decode for PledgeCrest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            crest_id: r.i32()?,
            data: read_crest_data(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPledgeEmblem.java
/// A large clan crest (`EX_PLEDGE_EMBLEM`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExPledgeEmblem {
    pub crest_id: i32,
    pub data: Vec<u8>,
}

impl ExPledgeEmblem {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPledgeEmblem;
}

impl Encode for ExPledgeEmblem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(0).i32(self.crest_id); // Java: the first int is a constant 0
        write_crest_data(w, &self.data);
    }
}

impl Decode for ExPledgeEmblem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?;
        Ok(Self {
            crest_id: r.i32()?,
            data: read_crest_data(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/AllyCrest.java
/// An alliance crest image (`ALLIANCE_CREST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllyCrest {
    pub crest_id: i32,
    pub data: Vec<u8>,
}

impl AllyCrest {
    pub const OPCODE: ServerOpcode = ServerOpcode::AllianceCrest;
}

impl Encode for AllyCrest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.crest_id);
        write_crest_data(w, &self.data);
    }
}

impl Decode for AllyCrest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            crest_id: r.i32()?,
            data: read_crest_data(r)?,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Member list
// ---------------------------------------------------------------------------------------------

/// One member row of [`PledgeShowMemberListAll`] and [`GmViewPledgeInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeMember {
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    /// Java writes the sex as an int; for a member that is offline [`PledgeShowMemberListAll`]
    /// writes a constant `1` (its comment says "no visible effect").
    pub sex: i32,
    /// Java `getRace().ordinal()`; a constant `1` for an offline member in
    /// [`PledgeShowMemberListAll`].
    pub race: i32,
    /// The member's object id when online, `0` when offline.
    pub online_object_id: i32,
    /// Java `getSponsor() != 0`.
    pub has_sponsor: bool,
}

impl PledgeMember {
    fn write(&self, w: &mut PacketWriter) {
        w.string(&self.name)
            .i32(self.level)
            .i32(self.class_id)
            .i32(self.sex)
            .i32(self.race)
            .i32(self.online_object_id)
            .i32(i32::from(self.has_sponsor));
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            name: r.string(),
            level: r.i32()?,
            class_id: r.i32()?,
            sex: r.i32()?,
            race: r.i32()?,
            online_object_id: r.i32()?,
            has_sponsor: r.i32()? != 0,
        })
    }
}

fn read_members(r: &mut PacketReader, field: &'static str) -> Result<Vec<PledgeMember>, PacketError> {
    let count = read_count(r, field)?;
    let mut members = Vec::new();
    for _ in 0..count {
        members.push(PledgeMember::read(r)?);
    }
    Ok(members)
}

fn write_members(w: &mut PacketWriter, members: &[PledgeMember]) {
    w.i32(members.len() as i32);
    for member in members {
        member.write(w);
    }
}

// Java: gameserver/network/serverpackets/PledgeShowMemberListAll.java
/// The clan window: the clan's data and the members of one unit (`PLEDGE_SHOW_MEMBER_LIST_ALL`).
///
/// Java sends the main clan in this packet and the sub pledges as separate
/// `PledgeReceiveSubPledgeCreated` and `PledgeShowMemberListAdd` packets, then refreshes the
/// player's user info; the caller does those.
///
/// NOTE(java-quirk): `writeImpl` always writes `0` for both `main_or_subpledge` and
/// `pledge_type` (a FIXME in the source says retail sends the packet once per sub pledge).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeShowMemberListAll {
    pub main_or_subpledge: i32,
    pub clan_id: i32,
    pub pledge_type: i32,
    pub clan_name: String,
    pub leader_name: String,
    pub crest_id: i32,
    pub level: i32,
    pub castle_id: i32,
    pub hideout_id: i32,
    pub fort_id: i32,
    pub rank: i32,
    pub reputation_score: i32,
    pub ally_id: i32,
    pub ally_name: String,
    pub ally_crest_id: i32,
    pub at_war: bool,
    /// The members whose pledge type is `pledge_type`; the count on the wire is their number.
    pub members: Vec<PledgeMember>,
}

impl PledgeShowMemberListAll {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeShowMemberListAll;
}

impl Encode for PledgeShowMemberListAll {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.main_or_subpledge)
            .i32(self.clan_id)
            .i32(self.pledge_type)
            .string(&self.clan_name)
            .string(&self.leader_name)
            .i32(self.crest_id)
            .i32(self.level)
            .i32(self.castle_id)
            .i32(self.hideout_id)
            .i32(self.fort_id)
            .i32(self.rank)
            .i32(self.reputation_score)
            .i32(0)
            .i32(0)
            .i32(self.ally_id)
            .string(&self.ally_name)
            .i32(self.ally_crest_id)
            .i32(i32::from(self.at_war))
            .i32(0); // Java: "Territory castle ID"
        write_members(w, &self.members);
    }
}

impl Decode for PledgeShowMemberListAll {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let main_or_subpledge = r.i32()?;
        let clan_id = r.i32()?;
        let pledge_type = r.i32()?;
        let clan_name = r.string();
        let leader_name = r.string();
        let crest_id = r.i32()?;
        let level = r.i32()?;
        let castle_id = r.i32()?;
        let hideout_id = r.i32()?;
        let fort_id = r.i32()?;
        let rank = r.i32()?;
        let reputation_score = r.i32()?;
        r.skip(8)?;
        let ally_id = r.i32()?;
        let ally_name = r.string();
        let ally_crest_id = r.i32()?;
        let at_war = r.i32()? != 0;
        r.skip(4)?;
        Ok(Self {
            main_or_subpledge,
            clan_id,
            pledge_type,
            clan_name,
            leader_name,
            crest_id,
            level,
            castle_id,
            hideout_id,
            fort_id,
            rank,
            reputation_score,
            ally_id,
            ally_name,
            ally_crest_id,
            at_war,
            members: read_members(r, "pledge members")?,
        })
    }
}

/// A member that is online, in [`PledgeShowMemberListUpdate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PledgeMemberOnline {
    pub object_id: i32,
    pub pledge_type: i32,
}

// Java: gameserver/network/serverpackets/PledgeShowMemberListUpdate.java
/// One member changed (`PLEDGE_SHOW_MEMBER_LIST_UPDATE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeShowMemberListUpdate {
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub female: bool,
    pub race: i32,
    /// `None` writes `0, 0` ("when going offline send as 0").
    pub online: Option<PledgeMemberOnline>,
    /// Java: only set for the academy.
    pub has_sponsor: bool,
}

impl PledgeShowMemberListUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeShowMemberListUpdate;
}

impl Encode for PledgeShowMemberListUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name)
            .i32(self.level)
            .i32(self.class_id)
            .i32(i32::from(self.female))
            .i32(self.race);
        match self.online {
            Some(online) => w.i32(online.object_id).i32(online.pledge_type),
            None => w.i32(0).i32(0),
        };
        w.i32(i32::from(self.has_sponsor));
    }
}

impl Decode for PledgeShowMemberListUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let level = r.i32()?;
        let class_id = r.i32()?;
        let female = r.i32()? != 0;
        let race = r.i32()?;
        let object_id = r.i32()?;
        let pledge_type = r.i32()?;
        Ok(Self {
            name,
            level,
            class_id,
            female,
            race,
            online: (object_id != 0).then_some(PledgeMemberOnline {
                object_id,
                pledge_type,
            }),
            has_sponsor: r.i32()? != 0,
        })
    }
}

// Java: gameserver/network/serverpackets/PledgeShowMemberListAdd.java
/// A member joined, or a sub pledge member is listed (`PLEDGE_SHOW_MEMBER_LIST_ADD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeShowMemberListAdd {
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    /// The member's object id when online, `0` when offline.
    pub online_object_id: i32,
    pub pledge_type: i32,
}

impl PledgeShowMemberListAdd {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeShowMemberListAdd;
}

impl Encode for PledgeShowMemberListAdd {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name)
            .i32(self.level)
            .i32(self.class_id)
            .i32(0)
            .i32(1)
            .i32(self.online_object_id)
            .i32(self.pledge_type);
    }
}

impl Decode for PledgeShowMemberListAdd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let level = r.i32()?;
        let class_id = r.i32()?;
        r.skip(8)?;
        Ok(Self {
            name,
            level,
            class_id,
            online_object_id: r.i32()?,
            pledge_type: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/PledgeShowMemberListDelete.java
server_packet!(
    /// A member left the clan (`PLEDGE_SHOW_MEMBER_LIST_DELETE`).
    PledgeShowMemberListDelete,
    PledgeShowMemberListDelete {
        player_name: String,
    }
);

// Java: gameserver/network/serverpackets/PledgeShowMemberListDeleteAll.java
server_unit_packet!(
    /// The player's clan is gone (`PLEDGE_SHOW_MEMBER_LIST_DELETE_ALL`).
    PledgeShowMemberListDeleteAll,
    PledgeShowMemberListDeleteAll
);

// Java: gameserver/network/serverpackets/PledgeReceiveMemberInfo.java
server_packet!(
    /// The details of one member (`PLEDGE_RECEIVE_MEMBER_INFO`).
    PledgeReceiveMemberInfo,
    PledgeReceiveMemberInfo {
        pledge_type: i32,
        name: String,
        title: String,
        power_grade: i32,
        /// The sub pledge's name, or the clan's name for the main clan.
        pledge_name: String,
        /// The apprentice's or the sponsor's name.
        apprentice_or_sponsor_name: String,
    }
);

// Java: gameserver/network/serverpackets/PledgeReceivePowerInfo.java
server_packet!(
    /// The rank and privileges of one member (`PLEDGE_RECEIVE_POWER_INFO`).
    PledgeReceivePowerInfo,
    PledgeReceivePowerInfo {
        power_grade: i32,
        name: String,
        privileges: i32,
    }
);

// Java: gameserver/network/serverpackets/PledgeReceiveSubPledgeCreated.java
/// A sub pledge exists (`PLEDGE_RECEIVE_SUB_PLEDGE_CREATED`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeReceiveSubPledgeCreated {
    pub id: i32,
    pub name: String,
    /// The leader's name; Java sends `""` for the academy, for no leader, and (with a warning)
    /// for a leader who is not in the clan.
    pub leader_name: String,
}

impl PledgeReceiveSubPledgeCreated {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeReceiveSubPledgeCreated;
}

impl Encode for PledgeReceiveSubPledgeCreated {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(1).i32(self.id).string(&self.name).string(&self.leader_name);
    }
}

impl Decode for PledgeReceiveSubPledgeCreated {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?;
        Ok(Self {
            id: r.i32()?,
            name: r.string(),
            leader_name: r.string(),
        })
    }
}

// Java: gameserver/network/serverpackets/PledgeShowInfoUpdate.java
/// The clan's numbers changed (`PLEDGE_SHOW_INFO_UPDATE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeShowInfoUpdate {
    pub clan_id: i32,
    pub crest_id: i32,
    pub level: i32,
    pub castle_id: i32,
    pub hideout_id: i32,
    pub fort_id: i32,
    pub rank: i32,
    pub reputation_score: i32,
    pub ally_id: i32,
    pub ally_name: String,
    pub ally_crest_id: i32,
    pub at_war: bool,
}

impl PledgeShowInfoUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeShowInfoUpdate;
}

impl Encode for PledgeShowInfoUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.clan_id)
            .i32(self.crest_id)
            .i32(self.level)
            .i32(self.castle_id)
            .i32(self.hideout_id)
            .i32(self.fort_id)
            .i32(self.rank)
            .i32(self.reputation_score)
            .i32(0)
            .i32(0)
            .i32(self.ally_id)
            .string(&self.ally_name)
            .i32(self.ally_crest_id)
            .i32(i32::from(self.at_war));
    }
}

impl Decode for PledgeShowInfoUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let clan_id = r.i32()?;
        let crest_id = r.i32()?;
        let level = r.i32()?;
        let castle_id = r.i32()?;
        let hideout_id = r.i32()?;
        let fort_id = r.i32()?;
        let rank = r.i32()?;
        let reputation_score = r.i32()?;
        r.skip(8)?;
        Ok(Self {
            clan_id,
            crest_id,
            level,
            castle_id,
            hideout_id,
            fort_id,
            rank,
            reputation_score,
            ally_id: r.i32()?,
            ally_name: r.string(),
            ally_crest_id: r.i32()?,
            at_war: r.i32()? != 0,
        })
    }
}

// Java: gameserver/network/serverpackets/PledgeStatusChanged.java
/// The ids of the clan changed (`PLEDGE_STATUS_CHANGED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PledgeStatusChanged {
    pub leader_id: i32,
    pub clan_id: i32,
    pub crest_id: i32,
    pub ally_id: i32,
    pub ally_crest_id: i32,
}

impl PledgeStatusChanged {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeStatusChanged;
}

impl Encode for PledgeStatusChanged {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.leader_id)
            .i32(self.clan_id)
            .i32(self.crest_id)
            .i32(self.ally_id)
            .i32(self.ally_crest_id)
            .i32(0)
            .i32(0);
    }
}

impl Decode for PledgeStatusChanged {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            leader_id: r.i32()?,
            clan_id: r.i32()?,
            crest_id: r.i32()?,
            ally_id: r.i32()?,
            ally_crest_id: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/GMViewPledgeInfo.java
/// What a GM sees of a clan (`GM_VIEW_PLEDGE_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmViewPledgeInfo {
    /// The name of the player whose clan it is.
    pub player_name: String,
    pub clan_id: i32,
    pub clan_name: String,
    pub leader_name: String,
    pub crest_id: i32,
    pub level: i32,
    pub castle_id: i32,
    pub hideout_id: i32,
    pub fort_id: i32,
    pub rank: i32,
    pub reputation_score: i32,
    pub ally_id: i32,
    pub ally_name: String,
    pub ally_crest_id: i32,
    pub at_war: bool,
    pub members: Vec<PledgeMember>,
}

impl GmViewPledgeInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmViewPledgeInfo;
}

impl Encode for GmViewPledgeInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.player_name)
            .i32(self.clan_id)
            .i32(0)
            .string(&self.clan_name)
            .string(&self.leader_name)
            .i32(self.crest_id)
            .i32(self.level)
            .i32(self.castle_id)
            .i32(self.hideout_id)
            .i32(self.fort_id)
            .i32(self.rank)
            .i32(self.reputation_score)
            .i32(0)
            .i32(0)
            .i32(self.ally_id)
            .string(&self.ally_name)
            .i32(self.ally_crest_id)
            .i32(i32::from(self.at_war))
            .i32(0); // Java: "T3 Unknown"
        write_members(w, &self.members);
    }
}

impl Decode for GmViewPledgeInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_name = r.string();
        let clan_id = r.i32()?;
        r.skip(4)?;
        let clan_name = r.string();
        let leader_name = r.string();
        let crest_id = r.i32()?;
        let level = r.i32()?;
        let castle_id = r.i32()?;
        let hideout_id = r.i32()?;
        let fort_id = r.i32()?;
        let rank = r.i32()?;
        let reputation_score = r.i32()?;
        r.skip(8)?;
        let ally_id = r.i32()?;
        let ally_name = r.string();
        let ally_crest_id = r.i32()?;
        let at_war = r.i32()? != 0;
        r.skip(4)?;
        Ok(Self {
            player_name,
            clan_id,
            clan_name,
            leader_name,
            crest_id,
            level,
            castle_id,
            hideout_id,
            fort_id,
            rank,
            reputation_score,
            ally_id,
            ally_name,
            ally_crest_id,
            at_war,
            members: read_members(r, "gm view pledge members")?,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Privileges and skills
// ---------------------------------------------------------------------------------------------

/// One rank and its privilege mask (Java `RankPrivs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankPrivileges {
    pub rank: i32,
    /// Java `getParty()`: the privilege bit mask.
    pub privileges: i32,
}

// Java: gameserver/network/serverpackets/PledgePowerGradeList.java
/// The privileges of every rank (`PLEDGE_POWER_GRADE_LIST`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PledgePowerGradeList {
    pub privs: Vec<RankPrivileges>,
}

impl PledgePowerGradeList {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgePowerGradeList;
}

impl Encode for PledgePowerGradeList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.privs.len() as i32);
        for p in &self.privs {
            w.i32(p.rank).i32(p.privileges);
        }
    }
}

impl Decode for PledgePowerGradeList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "pledge power grades")?;
        let mut privs = Vec::new();
        for _ in 0..count {
            privs.push(RankPrivileges {
                rank: r.i32()?,
                privileges: r.i32()?,
            });
        }
        Ok(Self { privs })
    }
}

// Java: gameserver/network/serverpackets/ManagePledgePower.java
/// The privileges of one rank (`MANAGE_PLEDGE_POWER`).
///
/// NOTE(java-quirk): Java writes this packet only when the `action` it was built with is `1`; for
/// any other action `writeImpl` writes nothing. Whoever sends it checks the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagePledgePower {
    pub privileges: i32,
}

impl ManagePledgePower {
    pub const OPCODE: ServerOpcode = ServerOpcode::ManagePledgePower;
}

impl Encode for ManagePledgePower {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(0).i32(0).i32(self.privileges);
    }
}

impl Decode for ManagePledgePower {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(8)?;
        Ok(Self { privileges: r.i32()? })
    }
}

/// A sub pledge skill in [`PledgeSkillList`] (Java `SubPledgeSkill`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubPledgeSkill {
    pub sub_type: i32,
    pub skill_id: i32,
    pub skill_level: i32,
}

/// A clan skill in [`PledgeSkillList`]: the display id and level Java writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PledgeSkillEntry {
    pub display_id: i32,
    pub display_level: i32,
}

// Java: gameserver/network/serverpackets/PledgeSkillList.java
/// The clan skills and the sub pledge skills (`PLEDGE_SKILL_LIST`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PledgeSkillList {
    pub skills: Vec<PledgeSkillEntry>,
    pub sub_skills: Vec<SubPledgeSkill>,
}

impl PledgeSkillList {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeSkillList;
}

impl Encode for PledgeSkillList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.skills.len() as i32).i32(self.sub_skills.len() as i32);
        for skill in &self.skills {
            w.i32(skill.display_id).i32(skill.display_level);
        }
        for skill in &self.sub_skills {
            w.i32(skill.sub_type).i32(skill.skill_id).i32(skill.skill_level);
        }
    }
}

impl Decode for PledgeSkillList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let skill_count = read_count(r, "pledge skills")?;
        let sub_count = read_count(r, "sub pledge skills")?;
        let mut skills = Vec::new();
        for _ in 0..skill_count {
            skills.push(PledgeSkillEntry {
                display_id: r.i32()?,
                display_level: r.i32()?,
            });
        }
        let mut sub_skills = Vec::new();
        for _ in 0..sub_count {
            sub_skills.push(SubPledgeSkill {
                sub_type: r.i32()?,
                skill_id: r.i32()?,
                skill_level: r.i32()?,
            });
        }
        Ok(Self { skills, sub_skills })
    }
}

// Java: gameserver/network/serverpackets/PledgeSkillListAdd.java
server_packet!(
    /// The clan learned a skill (`PLEDGE_SKILL_LIST_ADD`).
    PledgeSkillListAdd,
    PledgeSkillListAdd {
        id: i32,
        level: i32,
    }
);

// Java: gameserver/network/serverpackets/ExSubPledgeSkillAdd.java
server_packet!(
    /// A sub pledge learned a skill (`EX_SUB_PLEDGE_SKILL_ADD`).
    ExSubPledgeSkillAdd,
    ExSubPledgeSkillAdd {
        sub_type: i32,
        skill_id: i32,
        skill_level: i32,
    }
);

// ---------------------------------------------------------------------------------------------
// Wars
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/serverpackets/PledgeReceiveWarList.java
/// The wars of the clan (`PLEDGE_RECEIVE_WAR_LIST`).
///
/// NOTE(java-quirk): the count on the wire is the size of the clan's war list, but Java skips
/// the clans it cannot find, so fewer rows than the count can follow. `list_size` is the written
/// count and `clan_names` the rows; the decoder reads rows until the packet ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PledgeReceiveWarList {
    /// 0 declared wars, 1 under attack.
    pub tab: i32,
    pub list_size: i32,
    pub clan_names: Vec<String>,
}

impl PledgeReceiveWarList {
    pub const OPCODE: ServerOpcode = ServerOpcode::PledgeReceiveWarList;
}

impl Encode for PledgeReceiveWarList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.tab).i32(0).i32(self.list_size); // the middle int is the page
        for name in &self.clan_names {
            w.string(name).i32(self.tab).i32(self.tab);
        }
    }
}

impl Decode for PledgeReceiveWarList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let tab = r.i32()?;
        r.skip(4)?;
        let list_size = r.i32()?;
        let mut clan_names = Vec::new();
        while r.remaining() > 0 {
            clan_names.push(r.string());
            r.skip(8)?;
        }
        Ok(Self {
            tab,
            list_size,
            clan_names,
        })
    }
}

// Java: gameserver/network/serverpackets/StartPledgeWar.java
server_packet!(
    /// A clan declared war on yours (`START_PLEDGE_WAR`).
    StartPledgeWar,
    StartPledgeWar {
        player_name: String,
        pledge_name: String,
    }
);

// Java: gameserver/network/serverpackets/StopPledgeWar.java
server_packet!(
    /// A war ended (`STOP_PLEDGE_WAR`).
    StopPledgeWar,
    StopPledgeWar {
        pledge_name: String,
        player_name: String,
    }
);

// Java: gameserver/network/serverpackets/SurrenderPledgeWar.java
server_packet!(
    /// A clan surrendered (`SURRENDER_PLEDGE_WAR`).
    SurrenderPledgeWar,
    SurrenderPledgeWar {
        pledge_name: String,
        player_name: String,
    }
);

// ---------------------------------------------------------------------------------------------
// Alliances
// ---------------------------------------------------------------------------------------------

// Java: gameserver/network/serverpackets/AskJoinAlly.java
server_packet!(
    /// A clan leader is invited into an alliance (`ASK_JOIN_ALLIANCE`).
    AskJoinAlly,
    AskJoinAlliance {
        requestor_object_id: i32,
        requestor_name: String,
    }
);

/// One clan row of [`AllianceInfo`] (Java `ClanInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllianceClanInfo {
    pub clan_name: String,
    pub level: i32,
    pub leader_name: String,
    /// The number of members.
    pub total: i32,
    /// The number of members online.
    pub online: i32,
}

// Java: gameserver/network/serverpackets/AllianceInfo.java
/// The alliance window (`ALLIANCE_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllianceInfo {
    pub name: String,
    /// The sum of the members of all clans.
    pub total: i32,
    /// The sum of the members online.
    pub online: i32,
    /// The name of the leading clan.
    pub leader_clan: String,
    /// The name of the alliance leader.
    pub leader_player: String,
    pub allies: Vec<AllianceClanInfo>,
}

impl AllianceInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::AllianceInfo;
}

impl Encode for AllianceInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.name)
            .i32(self.total)
            .i32(self.online)
            .string(&self.leader_clan)
            .string(&self.leader_player)
            .i32(self.allies.len() as i32);
        for ally in &self.allies {
            w.string(&ally.clan_name)
                .i32(0)
                .i32(ally.level)
                .string(&ally.leader_name)
                .i32(ally.total)
                .i32(ally.online);
        }
    }
}

impl Decode for AllianceInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let name = r.string();
        let total = r.i32()?;
        let online = r.i32()?;
        let leader_clan = r.string();
        let leader_player = r.string();
        let count = read_count(r, "alliance clans")?;
        let mut allies = Vec::new();
        for _ in 0..count {
            let clan_name = r.string();
            r.skip(4)?;
            allies.push(AllianceClanInfo {
                clan_name,
                level: r.i32()?,
                leader_name: r.string(),
                total: r.i32()?,
                online: r.i32()?,
            });
        }
        Ok(Self {
            name,
            total,
            online,
            leader_clan,
            leader_player,
            allies,
        })
    }
}
