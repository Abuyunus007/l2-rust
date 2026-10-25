//! Server packets of the Territory War: the war start and end, the registration window and the
//! territory list.
//!
//! Same rules as [`super::party`]: values only. The owner clan names, the wards (emblems), the
//! registrations and the war time come from the territory tables; the fallback strings Java
//! writes when there is no owner are the world's to pass ([`NO_OWNER`], [`NO_ALLY`]).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// Java's text for a territory without an owner clan (`ExShowDominionRegistry`).
pub const NO_OWNER: &str = "No Owner";
/// Java's text for a territory without an owner alliance (`ExShowDominionRegistry`).
pub const NO_ALLY: &str = "No Ally";

/// Java adds this to the castle id to get the territory id (`ExShowDominionRegistry` writes
/// `MINID + castleId`).
const TERRITORY_ID_BASE: i32 = 80;

/// The emblem (ward) ids a territory owns, in the order Java lists them.
fn write_wards(w: &mut PacketWriter, ward_ids: &[i32]) {
    w.i32(ward_ids.len() as i32);
    for ward in ward_ids {
        w.i32(*ward);
    }
}

fn read_wards(r: &mut PacketReader) -> Result<Vec<i32>, PacketError> {
    let count = read_count(r, "ward count")?;
    let mut wards = Vec::new();
    for _ in 0..count {
        wards.push(r.i32()?);
    }
    Ok(wards)
}

// Java: gameserver/network/serverpackets/ExDominionWarStart.java
/// The player's territory war starts (`EX_DOMINION_WAR_START`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExDominionWarStart {
    /// The object id of the player.
    pub object_id: i32,
    /// The territory the player registered for (0 for none).
    pub territory_id: i32,
    /// Java `isDisguised(objectId)`.
    pub is_disguised: bool,
}

impl ExDominionWarStart {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExDominionWarStart;
}

impl Encode for ExDominionWarStart {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java writes a constant 1 here (`// ??`).
        w.i32(self.object_id)
            .i32(1)
            .i32(self.territory_id)
            .i32(i32::from(self.is_disguised));
        // The territory id again when disguised, else 0.
        w.i32(if self.is_disguised { self.territory_id } else { 0 });
    }
}

impl Decode for ExDominionWarStart {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        r.i32()?;
        let territory_id = r.i32()?;
        let is_disguised = r.i32()? != 0;
        // The last int is derived from the two above, so it is read and not kept.
        r.i32()?;
        Ok(Self {
            object_id,
            territory_id,
            is_disguised,
        })
    }
}

// Java: gameserver/network/serverpackets/ExDominionWarEnd.java
server_unit_packet!(
    /// The player's territory war ends (`EX_DOMINION_WAR_END`). No fields.
    ExDominionWarEnd,
    ExDominionWarEnd
);

/// The wards of one territory in a territory list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerritoryWards {
    pub territory_id: i32,
    /// The owned emblem (ward) ids.
    pub ward_ids: Vec<i32>,
}

// Java: gameserver/network/serverpackets/ExShowDominionRegistry.java
/// The registration window of one territory (`EX_SHOW_DOMINION_REGISTRY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowDominionRegistry {
    /// The castle id. The packet writes the territory id, `80 + castle_id`.
    pub castle_id: i32,
    /// The owner clan's name, or [`NO_OWNER`].
    pub owner_clan_name: String,
    /// The owner clan leader's name, or [`NO_OWNER`].
    pub owner_leader_name: String,
    /// The owner alliance's name, or [`NO_ALLY`].
    pub owner_ally_name: String,
    /// How many clans registered.
    pub clan_requests: i32,
    /// How many mercenaries registered.
    pub merc_requests: i32,
    /// The start of the war, unix seconds.
    pub war_time: i32,
    /// The current time, unix seconds.
    pub current_time: i32,
    /// Java: the viewer's clan is registered here.
    pub clan_registered: bool,
    /// Java: the viewer is registered here as a mercenary.
    pub merc_registered: bool,
    /// Every territory and its wards.
    pub territories: Vec<TerritoryWards>,
}

impl ExShowDominionRegistry {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowDominionRegistry;
}

impl Encode for ExShowDominionRegistry {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(TERRITORY_ID_BASE + self.castle_id)
            .string(&self.owner_clan_name)
            .string(&self.owner_leader_name)
            .string(&self.owner_ally_name)
            .i32(self.clan_requests)
            .i32(self.merc_requests)
            .i32(self.war_time)
            .i32(self.current_time)
            .i32(i32::from(self.clan_registered))
            .i32(i32::from(self.merc_registered))
            // Java writes a constant 1 here (`// unknown`).
            .i32(1)
            .i32(self.territories.len() as i32);
        for territory in &self.territories {
            w.i32(territory.territory_id);
            write_wards(w, &territory.ward_ids);
        }
    }
}

impl Decode for ExShowDominionRegistry {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let castle_id = r.i32()?.wrapping_sub(TERRITORY_ID_BASE);
        let owner_clan_name = r.string();
        let owner_leader_name = r.string();
        let owner_ally_name = r.string();
        let clan_requests = r.i32()?;
        let merc_requests = r.i32()?;
        let war_time = r.i32()?;
        let current_time = r.i32()?;
        let clan_registered = r.i32()? != 0;
        let merc_registered = r.i32()? != 0;
        r.i32()?;
        let count = read_count(r, "territory count")?;
        let mut territories = Vec::new();
        for _ in 0..count {
            territories.push(TerritoryWards {
                territory_id: r.i32()?,
                ward_ids: read_wards(r)?,
            });
        }
        Ok(Self {
            castle_id,
            owner_clan_name,
            owner_leader_name,
            owner_ally_name,
            clan_requests,
            merc_requests,
            war_time,
            current_time,
            clan_registered,
            merc_registered,
            territories,
        })
    }
}

/// One territory of the territory list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominionTerritory {
    pub territory_id: i32,
    /// Java `castle.getName().toLowerCase() + "_dominion"`.
    pub name: String,
    /// The owner clan's name.
    pub owner_clan_name: String,
    /// The owned emblem (ward) ids.
    pub ward_ids: Vec<i32>,
    /// The start of the war, unix seconds.
    pub war_start_secs: i32,
}

// Java: gameserver/network/serverpackets/ExReplyDominionInfo.java
/// The territory list (`EX_REPLY_DOMINION_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExReplyDominionInfo {
    pub territories: Vec<DominionTerritory>,
}

impl ExReplyDominionInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReplyDominionInfo;
}

impl Encode for ExReplyDominionInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.territories.len() as i32);
        for territory in &self.territories {
            w.i32(territory.territory_id)
                .string(&territory.name)
                .string(&territory.owner_clan_name);
            write_wards(w, &territory.ward_ids);
            w.i32(territory.war_start_secs);
        }
    }
}

impl Decode for ExReplyDominionInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "territory count")?;
        let mut territories = Vec::new();
        for _ in 0..count {
            territories.push(DominionTerritory {
                territory_id: r.i32()?,
                name: r.string(),
                owner_clan_name: r.string(),
                ward_ids: read_wards(r)?,
                war_start_secs: r.i32()?,
            });
        }
        Ok(Self { territories })
    }
}

/// Where one ward is (`ExShowOwnthingPos`): on the ground as an NPC, with the player who carries
/// it, or `0, 0, 0` when it is neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WardPosition {
    /// The territory the ward belongs to (81 to 89).
    pub territory_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// Java: gameserver/network/serverpackets/ExShowOwnthingPos.java
/// The positions of the territory wards on the map (`EX_SHOW_OWNTHING_POS`). Java writes the
/// wards only while the war runs; otherwise the list is empty (a count of 0).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExShowOwnthingPos {
    pub wards: Vec<WardPosition>,
}

impl ExShowOwnthingPos {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowOwnthingPos;
}

impl Encode for ExShowOwnthingPos {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.wards.len() as i32);
        for ward in &self.wards {
            w.i32(ward.territory_id).i32(ward.x).i32(ward.y).i32(ward.z);
        }
    }
}

impl Decode for ExShowOwnthingPos {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "ward count")?;
        let mut wards = Vec::new();
        for _ in 0..count {
            wards.push(WardPosition {
                territory_id: r.i32()?,
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            });
        }
        Ok(Self { wards })
    }
}
