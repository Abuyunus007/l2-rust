//! Server packets of the castle siege and the clan hall siege: the siege window, the attacker and
//! defender lists and the castle list.
//!
//! Same rules as [`super::party`]: values only. The world reads the castle, the owner clan, the
//! siege lists and the clock; what Java reads from those arrives as plain fields. Times are unix
//! seconds, already computed in the local zone where Java uses the calendar (D-48).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// The clan data the siege lists write for one clan (Java `Clan` from `ClanTable`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeClan {
    pub clan_id: i32,
    pub name: String,
    pub leader_name: String,
    pub crest_id: i32,
    pub ally_id: i32,
    pub ally_name: String,
    pub ally_crest_id: i32,
}

/// The part every clan record starts with: id, name, leader name, crest and the "signed time",
/// which Java always writes as 0.
fn write_clan_head(w: &mut PacketWriter, clan: &SiegeClan) {
    w.i32(clan.clan_id)
        .string(&clan.name)
        .string(&clan.leader_name)
        .i32(clan.crest_id)
        .i32(0);
}

/// The part every clan record ends with: the alliance and the alliance crest. The ally leader
/// name is always empty in Java.
fn write_clan_tail(w: &mut PacketWriter, clan: &SiegeClan) {
    w.i32(clan.ally_id)
        .string(&clan.ally_name)
        .string("")
        .i32(clan.ally_crest_id);
}

/// Reads the clan record up to the state (the signed time is read and dropped). The ally fields
/// are filled by [`read_clan_tail`].
fn read_clan_head(r: &mut PacketReader) -> Result<SiegeClan, PacketError> {
    let clan_id = r.i32()?;
    let name = r.string();
    let leader_name = r.string();
    let crest_id = r.i32()?;
    let _signed_time = r.i32()?;
    Ok(SiegeClan {
        clan_id,
        name,
        leader_name,
        crest_id,
        ally_id: 0,
        ally_name: String::new(),
        ally_crest_id: 0,
    })
}

fn read_clan_tail(r: &mut PacketReader, clan: &mut SiegeClan) -> Result<(), PacketError> {
    clan.ally_id = r.i32()?;
    clan.ally_name = r.string();
    let _ally_leader_name = r.string();
    clan.ally_crest_id = r.i32()?;
    Ok(())
}

/// Java writes the same header in both list packets: the residence id, 0, 1, 0, then the count
/// twice.
fn write_list_header(w: &mut PacketWriter, residence_id: i32, count: usize) {
    // NOTE(java-quirk): the count is the number of siege clans, including the ones Java skips
    // because `ClanTable` does not know them, so the list can hold fewer records than it says.
    w.i32(residence_id)
        .i32(0)
        .i32(1)
        .i32(0)
        .i32(count as i32)
        .i32(count as i32);
}

/// Reads the list header: the residence id and the count (the second copy of the count is
/// dropped).
fn read_list_header(r: &mut PacketReader) -> Result<(i32, usize), PacketError> {
    let residence_id = r.i32()?;
    r.i32()?;
    r.i32()?;
    r.i32()?;
    let count = read_count(r, "siege list count")?;
    r.i32()?;
    Ok((residence_id, count))
}

// Java: gameserver/network/serverpackets/SiegeAttackerList.java
/// The attackers of a castle or clan hall siege (`CASTLE_SIEGE_ATTACKER_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeAttackerList {
    /// The castle or clan hall id.
    pub residence_id: i32,
    /// One entry per attacker. `None`: the clan is missing from `ClanTable`; Java writes no
    /// record for it, but the count still includes it. Decoding never produces `None`.
    pub clans: Vec<Option<SiegeClan>>,
}

impl SiegeAttackerList {
    pub const OPCODE: ServerOpcode = ServerOpcode::CastleSiegeAttackerList;
}

impl Encode for SiegeAttackerList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_list_header(w, self.residence_id, self.clans.len());
        for clan in self.clans.iter().flatten() {
            write_clan_head(w, clan);
            write_clan_tail(w, clan);
        }
    }
}

impl Decode for SiegeAttackerList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (residence_id, count) = read_list_header(r)?;
        let mut clans = Vec::new();
        for _ in 0..count {
            let mut clan = read_clan_head(r)?;
            read_clan_tail(r, &mut clan)?;
            clans.push(Some(clan));
        }
        Ok(Self { residence_id, clans })
    }
}

/// The state of a defender clan on the wire (Java `SiegeClan.Type`, mapped in
/// `SiegeDefenderList`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefenderState {
    /// 0: any other state.
    Other,
    /// 1: the castle owner.
    Owner,
    /// 2: a defender the lord approved (Java `DEFENDER_PENDING`). The waiting list also writes 2.
    DefenderPending,
    /// 3: a defender that is approved (Java `DEFENDER`).
    Defender,
}

impl DefenderState {
    /// The int Java writes.
    pub fn id(self) -> i32 {
        match self {
            Self::Other => 0,
            Self::Owner => 1,
            Self::DefenderPending => 2,
            Self::Defender => 3,
        }
    }

    fn from_id(id: i32) -> Result<Self, PacketError> {
        Ok(match id {
            0 => Self::Other,
            1 => Self::Owner,
            2 => Self::DefenderPending,
            3 => Self::Defender,
            _ => {
                return Err(PacketError::invalid(
                    "siege defender state",
                    format!("unknown state {id}"),
                ));
            }
        })
    }
}

/// One defender record of [`SiegeDefenderList`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeDefender {
    pub clan: SiegeClan,
    pub state: DefenderState,
}

// Java: gameserver/network/serverpackets/SiegeDefenderList.java
/// The defenders of a castle: the owner, the approved clans and the clans waiting for approval
/// (`CASTLE_SIEGE_DEFENDER_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeDefenderList {
    /// The castle id.
    pub castle_id: i32,
    /// One entry per defender, the approved ones first, then the waiting ones. `None`: the clan
    /// is missing from `ClanTable`, see [`SiegeAttackerList::clans`].
    pub clans: Vec<Option<SiegeDefender>>,
}

impl SiegeDefenderList {
    pub const OPCODE: ServerOpcode = ServerOpcode::CastleSiegeDefenderList;
}

impl Encode for SiegeDefenderList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_list_header(w, self.castle_id, self.clans.len());
        for defender in self.clans.iter().flatten() {
            write_clan_head(w, &defender.clan);
            w.i32(defender.state.id());
            write_clan_tail(w, &defender.clan);
        }
    }
}

impl Decode for SiegeDefenderList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (castle_id, count) = read_list_header(r)?;
        let mut clans = Vec::new();
        for _ in 0..count {
            let mut clan = read_clan_head(r)?;
            let state = DefenderState::from_id(r.i32()?)?;
            read_clan_tail(r, &mut clan)?;
            clans.push(Some(SiegeDefender { clan, state }));
        }
        Ok(Self { castle_id, clans })
    }
}

/// The owner clan block of [`SiegeInfo`], present when the residence has an owner that
/// `ClanTable` knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeOwnerClan {
    pub name: String,
    pub leader_name: String,
    pub ally_id: i32,
    pub ally_name: String,
}

/// The time block of [`SiegeInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiegeTime {
    /// `[date, 0]`: the siege date (a castle the viewer may not change, or a clan hall's next
    /// siege time). A date of 0 reads back as [`SiegeTime::Hours`] with no hours.
    Date(i32),
    /// `[0, count, hours...]`: the owner leader picks one of these hours (the epoch seconds of
    /// each hour of the siege day, from `FeatureConfig.SIEGE_HOUR_LIST`).
    Hours(Vec<i32>),
}

// Java: gameserver/network/serverpackets/SiegeInfo.java
/// The siege window of a castle or a clan hall (`CASTLE_SIEGE_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegeInfo {
    /// The castle or clan hall id.
    pub residence_id: i32,
    /// Java: the owner clan is the viewer's clan and the viewer is its leader.
    pub viewer_is_owner_leader: bool,
    /// The owner clan id (0 for none).
    pub owner_id: i32,
    /// The owner's data, written when `owner_id > 0`. With `owner_id > 0` and `None`, the clan is
    /// missing from `ClanTable`: Java then writes no owner block at all (a shorter packet). The
    /// decoder reads a block there, so that case is not a round trip.
    pub owner: Option<SiegeOwnerClan>,
    /// The current time in unix seconds.
    pub now_secs: i32,
    pub time: SiegeTime,
}

impl SiegeInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::CastleSiegeInfo;
}

impl Encode for SiegeInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.residence_id)
            .i32(i32::from(self.viewer_is_owner_leader))
            .i32(self.owner_id);
        match (self.owner_id > 0, &self.owner) {
            (true, Some(owner)) => {
                w.string(&owner.name)
                    .string(&owner.leader_name)
                    .i32(owner.ally_id)
                    .string(&owner.ally_name);
            }
            // NOTE(java-quirk): an owner id that names no clan writes no block (see the field).
            (true, None) => {}
            (false, _) => {
                w.string("").string("").i32(0).string("");
            }
        }
        w.i32(self.now_secs);
        match &self.time {
            SiegeTime::Date(date) => {
                w.i32(*date).i32(0);
            }
            SiegeTime::Hours(hours) => {
                w.i32(0).i32(hours.len() as i32);
                for hour in hours {
                    w.i32(*hour);
                }
            }
        }
    }
}

impl Decode for SiegeInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let residence_id = r.i32()?;
        let viewer_is_owner_leader = r.i32()? != 0;
        let owner_id = r.i32()?;
        let owner = if owner_id > 0 {
            Some(SiegeOwnerClan {
                name: r.string(),
                leader_name: r.string(),
                ally_id: r.i32()?,
                ally_name: r.string(),
            })
        } else {
            r.string();
            r.string();
            r.i32()?;
            r.string();
            None
        };
        let now_secs = r.i32()?;
        let first = r.i32()?;
        let second = r.i32()?;
        let time = if first == 0 {
            // `second` is the hour count here.
            let count = usize::try_from(second)
                .map_err(|_| PacketError::invalid("siege hour count", format!("negative count {second}")))?;
            let mut hours = Vec::new();
            for _ in 0..count {
                hours.push(r.i32()?);
            }
            SiegeTime::Hours(hours)
        } else {
            SiegeTime::Date(first)
        };
        Ok(Self {
            residence_id,
            viewer_is_owner_leader,
            owner_id,
            owner,
            now_secs,
            time,
        })
    }
}

/// One castle line of [`ExShowCastleInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastleInfo {
    pub residence_id: i32,
    /// The owner clan's name, empty when there is no owner or `ClanTable` does not know it.
    pub owner_name: String,
    pub tax_percent: i32,
    /// The siege date in unix seconds.
    pub siege_date_secs: i32,
}

// Java: gameserver/network/serverpackets/ExShowCastleInfo.java
/// The castle list for the castle window (`EX_SHOW_CASTLE_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowCastleInfo {
    pub castles: Vec<CastleInfo>,
}

impl ExShowCastleInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowCastleInfo;
}

impl Encode for ExShowCastleInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.castles.len() as i32);
        for castle in &self.castles {
            w.i32(castle.residence_id)
                .string(&castle.owner_name)
                .i32(castle.tax_percent)
                .i32(castle.siege_date_secs);
        }
    }
}

impl Decode for ExShowCastleInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "castle count")?;
        let mut castles = Vec::new();
        for _ in 0..count {
            castles.push(CastleInfo {
                residence_id: r.i32()?,
                owner_name: r.string(),
                tax_percent: r.i32()?,
                siege_date_secs: r.i32()?,
            });
        }
        Ok(Self { castles })
    }
}
