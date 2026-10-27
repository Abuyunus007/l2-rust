//! Server packets of the raid boss points record and of the cursed weapons.
//!
//! Values only: the points, the ranking and the cursed weapon places are the world's.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// One raid boss of the points record: the boss id and the points gained from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BossRecordEntry {
    pub boss_id: i32,
    pub points: i32,
}

// Java: gameserver/network/serverpackets/ExGetBossRecord.java
/// The player's raid boss points, ranking and the points per boss (`EX_GET_BOSS_RECORD`).
///
/// Java writes a third zero after each boss. Java's null list is written as four zeros; the
/// world always passes a list (possibly empty), so that case does not come up here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExGetBossRecord {
    pub ranking: i32,
    pub total_points: i32,
    pub records: Vec<BossRecordEntry>,
}

impl ExGetBossRecord {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExGetBossRecord;
}

impl Encode for ExGetBossRecord {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.ranking)
            .i32(self.total_points)
            .i32(self.records.len() as i32);
        for record in &self.records {
            // NOTE(java-quirk): Java writes an unexplained zero (`// ??`) after each boss.
            w.i32(record.boss_id).i32(record.points).i32(0);
        }
    }
}

impl Decode for ExGetBossRecord {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let ranking = r.i32()?;
        let total_points = r.i32()?;
        let count = read_count(r, "boss record count")?;
        let mut records = Vec::new();
        for _ in 0..count {
            let boss_id = r.i32()?;
            let points = r.i32()?;
            let _unknown = r.i32()?;
            records.push(BossRecordEntry { boss_id, points });
        }
        Ok(Self {
            ranking,
            total_points,
            records,
        })
    }
}

// Java: gameserver/network/serverpackets/ExCursedWeaponList.java
/// The ids of the cursed weapons in the world (`EX_CURSED_WEAPON_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExCursedWeaponList {
    pub ids: Vec<i32>,
}

impl ExCursedWeaponList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExCursedWeaponList;
}

impl Encode for ExCursedWeaponList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.ids.len() as i32);
        for id in &self.ids {
            w.i32(*id);
        }
    }
}

impl Decode for ExCursedWeaponList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "cursed weapon count")?;
        let mut ids = Vec::new();
        for _ in 0..count {
            ids.push(r.i32()?);
        }
        Ok(Self { ids })
    }
}

/// Where one cursed weapon is (Java `ExCursedWeaponLocation.CursedWeaponInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursedWeaponLocation {
    pub id: i32,
    /// `0` not activated, `1` activated (carried).
    pub activated: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// Java: gameserver/network/serverpackets/ExCursedWeaponLocation.java
/// Where the cursed weapons are (`EX_CURSED_WEAPON_LOCATION`).
///
/// Java writes two zeros when the list is empty (a count of 0 and one more zero), so an empty
/// list keeps that layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExCursedWeaponLocation {
    pub locations: Vec<CursedWeaponLocation>,
}

impl ExCursedWeaponLocation {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExCursedWeaponLocation;
}

impl Encode for ExCursedWeaponLocation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        if self.locations.is_empty() {
            w.i32(0).i32(0);
            return;
        }
        w.i32(self.locations.len() as i32);
        for location in &self.locations {
            w.i32(location.id)
                .i32(location.activated)
                .i32(location.x)
                .i32(location.y)
                .i32(location.z);
        }
    }
}

impl Decode for ExCursedWeaponLocation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "cursed weapon location count")?;
        if count == 0 {
            let _zero = r.i32()?;
            return Ok(Self {
                locations: Vec::new(),
            });
        }
        let mut locations = Vec::new();
        for _ in 0..count {
            locations.push(CursedWeaponLocation {
                id: r.i32()?,
                activated: r.i32()?,
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            });
        }
        Ok(Self { locations })
    }
}
