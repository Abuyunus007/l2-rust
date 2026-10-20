//! Party server packets: the invitation, the party window and the party pet window.
//!
//! Same rules as [`super::skills`]: values only. The world decides who is in the party and
//! builds the member records; what Java reads from `Player`/`Party`/`Summon` arrives as plain
//! fields. Hit points and the like are already the `int` Java writes (`(int) getCurrentHp()`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// Java adds this to a summon's template id so the client shows the right icon.
const SUMMON_ID_OFFSET: i32 = 1_000_000;

// Java: gameserver/entity/groups/PartyDistributionType.java
/// How the party shares loot. The id goes on the wire; the string ids are the client's
/// `SystemString` ids for the loot setting text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyDistributionType {
    FindersKeepers,
    Random,
    RandomIncludingSpoil,
    ByTurn,
    ByTurnIncludingSpoil,
}

impl PartyDistributionType {
    /// Java `getId()`.
    pub fn id(self) -> i32 {
        match self {
            Self::FindersKeepers => 0,
            Self::Random => 1,
            Self::RandomIncludingSpoil => 2,
            Self::ByTurn => 3,
            Self::ByTurnIncludingSpoil => 4,
        }
    }

    /// Java `getSysStringId()`.
    pub fn sys_string_id(self) -> i32 {
        match self {
            Self::FindersKeepers => 487,
            Self::Random => 488,
            Self::RandomIncludingSpoil => 798,
            Self::ByTurn => 799,
            Self::ByTurnIncludingSpoil => 800,
        }
    }

    /// Java `findById`: `None` for an unknown id.
    pub fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            0 => Self::FindersKeepers,
            1 => Self::Random,
            2 => Self::RandomIncludingSpoil,
            3 => Self::ByTurn,
            4 => Self::ByTurnIncludingSpoil,
            _ => return None,
        })
    }
}

fn read_distribution(r: &mut PacketReader) -> Result<PartyDistributionType, PacketError> {
    let id = r.i32()?;
    PartyDistributionType::from_id(id)
        .ok_or_else(|| PacketError::invalid("party distribution type", format!("unknown type {id}")))
}

// Java: gameserver/network/serverpackets/AskJoinParty.java
/// The invited player is asked to join a party (`ASK_JOIN_PARTY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskJoinParty {
    pub requestor_name: String,
    pub distribution_type: PartyDistributionType,
}

impl AskJoinParty {
    pub const OPCODE: ServerOpcode = ServerOpcode::AskJoinParty;
}

impl Encode for AskJoinParty {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.requestor_name).i32(self.distribution_type.id());
    }
}

impl Decode for AskJoinParty {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            requestor_name: r.string(),
            distribution_type: read_distribution(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/JoinParty.java
/// The answer of the invited player, passed on to the inviter (`JOIN_PARTY`): 1 accepted,
/// 0 declined, -1 refuses party requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinParty {
    pub response: i32,
}

impl JoinParty {
    pub const OPCODE: ServerOpcode = ServerOpcode::JoinParty;
}

impl Encode for JoinParty {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.response);
    }
}

impl Decode for JoinParty {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { response: r.i32()? })
    }
}

/// The vital numbers of a party member that [`PartySmallWindowAll`], [`PartySmallWindowAdd`] and
/// [`PartySmallWindowUpdate`] all write (after the packet specific header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberVitals {
    pub object_id: i32,
    pub name: String,
    pub cp: i32,
    pub max_cp: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub level: i32,
    pub class_id: i32,
}

impl PartyMemberVitals {
    fn write(&self, w: &mut PacketWriter) {
        w.i32(self.object_id)
            .string(&self.name)
            .i32(self.cp)
            .i32(self.max_cp)
            .i32(self.hp)
            .i32(self.max_hp)
            .i32(self.mp)
            .i32(self.max_mp)
            .i32(self.level)
            .i32(self.class_id);
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            name: r.string(),
            cp: r.i32()?,
            max_cp: r.i32()?,
            hp: r.i32()?,
            max_hp: r.i32()?,
            mp: r.i32()?,
            max_mp: r.i32()?,
            level: r.i32()?,
            class_id: r.i32()?,
        })
    }
}

/// The summon block of a member in [`PartySmallWindowAll`] and the body of the party pet window
/// packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySummon {
    pub object_id: i32,
    /// The template id the packet shows: Java writes `id + 1000000` (`displayId + 1000000` for the
    /// pet window packets); the offset is added on the wire only.
    pub display_id: i32,
    /// 1 servitor, 2 pet (Java `getSummonType()`).
    pub summon_type: i32,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub level: i32,
}

impl PartySummon {
    /// The name and the five numbers, shared with the pet window packets.
    fn write_stats(&self, w: &mut PacketWriter) {
        w.string(&self.name)
            .i32(self.hp)
            .i32(self.max_hp)
            .i32(self.mp)
            .i32(self.max_mp)
            .i32(self.level);
    }
}

/// One member entry of [`PartySmallWindowAll`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyWindowMember {
    pub vitals: PartyMemberVitals,
    /// Java `getRace().ordinal()`.
    pub race: i32,
    pub summon: Option<PartySummon>,
}

// Java: gameserver/network/serverpackets/PartySmallWindowAll.java
/// The whole party window, sent to a player who just joined (`PARTY_SMALL_WINDOW_ALL`). `members`
/// is the party without the receiver (Java's `_exclude`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySmallWindowAll {
    pub leader_object_id: i32,
    pub distribution_type: PartyDistributionType,
    pub members: Vec<PartyWindowMember>,
}

impl PartySmallWindowAll {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartySmallWindowAll;
}

impl Encode for PartySmallWindowAll {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java writes `getMemberCount() - 1`, which is the list length when the receiver is in
        // the party; the caller passes the list without the receiver.
        w.i32(self.leader_object_id)
            .i32(self.distribution_type.id())
            .i32(self.members.len() as i32);
        for member in &self.members {
            member.vitals.write(w);
            w.i32(0) // Java: `writeInt(0); // writeInt(1); ??`
                .i32(member.race)
                .i32(0) // T2.3
                .i32(0); // T2.3
            match &member.summon {
                None => {
                    w.i32(0);
                }
                Some(s) => {
                    w.i32(s.object_id)
                        .i32(s.display_id + SUMMON_ID_OFFSET)
                        .i32(s.summon_type);
                    s.write_stats(w);
                }
            }
        }
    }
}

impl Decode for PartySmallWindowAll {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let leader_object_id = r.i32()?;
        let distribution_type = read_distribution(r)?;
        let count = read_count(r, "party members")?;
        let mut members = Vec::new();
        for _ in 0..count {
            let vitals = PartyMemberVitals::read(r)?;
            r.skip(4)?;
            let race = r.i32()?;
            r.skip(8)?;
            // The first int of the summon block is its object id; Java writes a single 0 when the
            // member has no summon.
            let summon_id = r.i32()?;
            let summon = if summon_id == 0 {
                None
            } else {
                Some(PartySummon {
                    object_id: summon_id,
                    display_id: r.i32()?.wrapping_sub(SUMMON_ID_OFFSET),
                    summon_type: r.i32()?,
                    name: r.string(),
                    hp: r.i32()?,
                    max_hp: r.i32()?,
                    mp: r.i32()?,
                    max_mp: r.i32()?,
                    level: r.i32()?,
                })
            };
            members.push(PartyWindowMember { vitals, race, summon });
        }
        Ok(Self {
            leader_object_id,
            distribution_type,
            members,
        })
    }
}

// Java: gameserver/network/serverpackets/PartySmallWindowAdd.java
/// A member joined the party (`PARTY_SMALL_WINDOW_ADD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySmallWindowAdd {
    pub leader_object_id: i32,
    pub distribution_type: PartyDistributionType,
    pub member: PartyMemberVitals,
}

impl PartySmallWindowAdd {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartySmallWindowAdd;
}

impl Encode for PartySmallWindowAdd {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.leader_object_id).i32(self.distribution_type.id());
        self.member.write(w);
        w.i32(0).i32(0); // Java: `writeInt(0); // ?` twice
    }
}

impl Decode for PartySmallWindowAdd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let leader_object_id = r.i32()?;
        let distribution_type = read_distribution(r)?;
        let member = PartyMemberVitals::read(r)?;
        r.skip(8)?;
        Ok(Self {
            leader_object_id,
            distribution_type,
            member,
        })
    }
}

// Java: gameserver/network/serverpackets/PartySmallWindowDelete.java
/// A member left the party (`PARTY_SMALL_WINDOW_DELETE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySmallWindowDelete {
    pub object_id: i32,
    pub name: String,
}

impl PartySmallWindowDelete {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartySmallWindowDelete;
}

impl Encode for PartySmallWindowDelete {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).string(&self.name);
    }
}

impl Decode for PartySmallWindowDelete {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            name: r.string(),
        })
    }
}

// Java: gameserver/network/serverpackets/PartySmallWindowDeleteAll.java
/// The party window is closed (`PARTY_SMALL_WINDOW_DELETE_ALL`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartySmallWindowDeleteAll;

impl PartySmallWindowDeleteAll {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartySmallWindowDeleteAll;
}

impl Encode for PartySmallWindowDeleteAll {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for PartySmallWindowDeleteAll {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/PartySmallWindowUpdate.java
/// The vitals of one member changed (`PARTY_SMALL_WINDOW_UPDATE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySmallWindowUpdate {
    pub member: PartyMemberVitals,
}

impl PartySmallWindowUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartySmallWindowUpdate;
}

impl Encode for PartySmallWindowUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        self.member.write(w);
    }
}

impl Decode for PartySmallWindowUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            member: PartyMemberVitals::read(r)?,
        })
    }
}

/// One position of [`PartyMemberPosition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyMemberLocation {
    pub object_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// Java: gameserver/network/serverpackets/PartyMemberPosition.java
/// The positions of the party members for the minimap (`PARTY_MEMBER_POSITION`).
///
/// NOTE(java-quirk): Java keeps the positions in a `HashMap` keyed by object id, so the order on
/// the wire is the map's iteration order. The client does not care about the order; the caller
/// picks one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartyMemberPosition {
    pub locations: Vec<PartyMemberLocation>,
}

impl PartyMemberPosition {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartyMemberPosition;
}

impl Encode for PartyMemberPosition {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.locations.len() as i32);
        for loc in &self.locations {
            w.i32(loc.object_id).i32(loc.x).i32(loc.y).i32(loc.z);
        }
    }
}

impl Decode for PartyMemberPosition {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "party member positions")?;
        let mut locations = Vec::new();
        for _ in 0..count {
            locations.push(PartyMemberLocation {
                object_id: r.i32()?,
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            });
        }
        Ok(Self { locations })
    }
}

/// Body of [`ExPartyPetWindowAdd`] and [`ExPartyPetWindowUpdate`]: the same fields in the same
/// order. The summon's owner is separate from [`PartySummon`] because the member list does not
/// repeat it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyPetWindow {
    pub summon: PartySummon,
    pub owner_object_id: i32,
}

impl PartyPetWindow {
    fn write(&self, w: &mut PacketWriter) {
        let s = &self.summon;
        w.i32(s.object_id)
            .i32(s.display_id + SUMMON_ID_OFFSET)
            .i32(s.summon_type)
            .i32(self.owner_object_id);
        s.write_stats(w);
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let display_id = r.i32()?.wrapping_sub(SUMMON_ID_OFFSET);
        let summon_type = r.i32()?;
        let owner_object_id = r.i32()?;
        Ok(Self {
            summon: PartySummon {
                object_id,
                display_id,
                summon_type,
                name: r.string(),
                hp: r.i32()?,
                max_hp: r.i32()?,
                mp: r.i32()?,
                max_mp: r.i32()?,
                level: r.i32()?,
            },
            owner_object_id,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPartyPetWindowAdd.java
/// A party member's summon appears in the party window (`EX_PARTY_PET_WINDOW_ADD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExPartyPetWindowAdd {
    pub window: PartyPetWindow,
}

impl ExPartyPetWindowAdd {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPartyPetWindowAdd;
}

impl Encode for ExPartyPetWindowAdd {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        self.window.write(w);
    }
}

impl Decode for ExPartyPetWindowAdd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            window: PartyPetWindow::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPartyPetWindowUpdate.java
/// A party member's summon changed (`EX_PARTY_PET_WINDOW_UPDATE`); the layout of
/// [`ExPartyPetWindowAdd`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExPartyPetWindowUpdate {
    pub window: PartyPetWindow,
}

impl ExPartyPetWindowUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPartyPetWindowUpdate;
}

impl Encode for ExPartyPetWindowUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        self.window.write(w);
    }
}

impl Decode for ExPartyPetWindowUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            window: PartyPetWindow::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPartyPetWindowDelete.java
/// A party member's summon is gone (`EX_PARTY_PET_WINDOW_DELETE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExPartyPetWindowDelete {
    pub summon_object_id: i32,
    pub owner_object_id: i32,
    pub summon_name: String,
}

impl ExPartyPetWindowDelete {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPartyPetWindowDelete;
}

impl Encode for ExPartyPetWindowDelete {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.summon_object_id)
            .i32(self.owner_object_id)
            .string(&self.summon_name);
    }
}

impl Decode for ExPartyPetWindowDelete {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            summon_object_id: r.i32()?,
            owner_object_id: r.i32()?,
            summon_name: r.string(),
        })
    }
}
