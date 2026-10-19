//! Combat server packets: attacks and their hits, auto attack on and off, death and revive, the
//! move and wait type changes, and the shared-group item cooldown.
//!
//! Same rules as [`super::interaction`]: these carry values only. The world works out the damage,
//! the hit flags and the death buttons, and passes the numbers in.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Hit flag: the attack used a soulshot. The soulshot grade is ORed into the low bits of the
/// same byte (Java `HITFLAG_USESS | ssGrade`), so the grade must stay below `0x10`.
pub const HITFLAG_USESS: u8 = 0x10;
/// Hit flag: critical hit.
pub const HITFLAG_CRIT: u8 = 0x20;
/// Hit flag: the target's shield blocked the hit (Java `shld > 0`).
pub const HITFLAG_SHLD: u8 = 0x40;
/// Hit flag: the attack missed.
pub const HITFLAG_MISS: u8 = 0x80;

// Java: gameserver/entity/actor/holders/creature/Hit.java
/// One hit of an [`Attack`]: the target, the damage and the `HITFLAG_*` bits. Java writes the
/// flags with `writeByte`, so only the low 8 bits reach the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackHit {
    pub target_id: i32,
    pub damage: i32,
    pub flags: u8,
}

impl AttackHit {
    fn write(&self, w: &mut PacketWriter) {
        w.i32(self.target_id).i32(self.damage).u8(self.flags);
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            target_id: r.i32()?,
            damage: r.i32()?,
            flags: r.u8()?,
        })
    }
}

// Java: gameserver/network/serverpackets/Attack.java
/// An attack: the attacker, its position, the hits and the target's position (`ATTACK`).
///
/// The first hit sits right after the attacker id. Java then writes the number of hits after it
/// as a short and the rest of the hits; dual weapons send two hits. There is always one first hit
/// (Java throws on an attack without hits, so the type makes it impossible here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attack {
    pub attacker_id: i32,
    pub attacker_x: i32,
    pub attacker_y: i32,
    pub attacker_z: i32,
    pub first_hit: AttackHit,
    /// Hits after the first one, in order (Java `addHit`).
    pub other_hits: Vec<AttackHit>,
    pub target_x: i32,
    pub target_y: i32,
    pub target_z: i32,
}

impl Attack {
    pub const OPCODE: ServerOpcode = ServerOpcode::Attack;

    /// Java `addHit`: appends a hit after the first one.
    pub fn add_hit(&mut self, hit: AttackHit) {
        self.other_hits.push(hit);
    }
}

impl Encode for Attack {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.attacker_id);
        self.first_hit.write(w);
        w.i32(self.attacker_x).i32(self.attacker_y).i32(self.attacker_z);
        // NOTE(java-quirk): Java `writeShort(size - 1)`. A short keeps the low 16 bits, so `as`
        // does the same for a (never seen) huge hit list.
        w.i16(self.other_hits.len() as i16);
        for hit in &self.other_hits {
            hit.write(w);
        }
        w.i32(self.target_x).i32(self.target_y).i32(self.target_z);
    }
}

impl Decode for Attack {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let attacker_id = r.i32()?;
        let first_hit = AttackHit::read(r)?;
        let attacker_x = r.i32()?;
        let attacker_y = r.i32()?;
        let attacker_z = r.i32()?;
        let extra = usize::from(r.u16()?);
        let mut other_hits = Vec::new();
        for _ in 0..extra {
            other_hits.push(AttackHit::read(r)?);
        }
        Ok(Self {
            attacker_id,
            attacker_x,
            attacker_y,
            attacker_z,
            first_hit,
            other_hits,
            target_x: r.i32()?,
            target_y: r.i32()?,
            target_z: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/AutoAttackStart.java
/// The player starts auto attacking a target (`AUTO_ATTACK_START`). Java marks it droppable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoAttackStart {
    pub target_id: i32,
}

impl AutoAttackStart {
    pub const OPCODE: ServerOpcode = ServerOpcode::AutoAttackStart;
}

impl Encode for AutoAttackStart {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.target_id);
    }
}

impl Decode for AutoAttackStart {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { target_id: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/AutoAttackStop.java
/// The player stops auto attacking (`AUTO_ATTACK_STOP`). Java marks it droppable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoAttackStop {
    pub target_id: i32,
}

impl AutoAttackStop {
    pub const OPCODE: ServerOpcode = ServerOpcode::AutoAttackStop;
}

impl Encode for AutoAttackStop {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.target_id);
    }
}

impl Decode for AutoAttackStop {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { target_id: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/Die.java
/// The death window (`DIE`). The flags decide which buttons the client shows.
///
/// The world works them out (Java's constructor and `writeImpl`). The four clan buttons are only
/// true when `can_teleport` is set, the player has a clan and is not jailed; Java writes zeros for
/// them otherwise, so the world passes `false` in that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Die {
    pub object_id: i32,
    /// Java `canRevive() && !isPendingRevive()`.
    pub can_teleport: bool,
    /// The clan hideout button ("to the clan hall").
    pub to_clan_hall: bool,
    /// The clan castle button.
    pub to_castle: bool,
    /// The siege headquarters button (territory war HQ, siege flag, clan hall siege).
    pub to_siege_hq: bool,
    /// The blue glow: the corpse can be swept (`isSweepActive`).
    pub sweepable: bool,
    /// The fixed resurrection button (a self-resurrection item, or an access level that allows it).
    pub fixed_res: bool,
    /// The clan fortress button.
    pub to_fortress: bool,
}

impl Die {
    pub const OPCODE: ServerOpcode = ServerOpcode::Die;
}

impl Encode for Die {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(i32::from(self.can_teleport))
            .i32(i32::from(self.to_clan_hall))
            .i32(i32::from(self.to_castle))
            .i32(i32::from(self.to_siege_hq))
            .i32(i32::from(self.sweepable))
            .i32(i32::from(self.fixed_res))
            .i32(i32::from(self.to_fortress));
    }
}

impl Decode for Die {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let can_teleport = r.i32()? != 0;
        let to_clan_hall = r.i32()? != 0;
        let to_castle = r.i32()? != 0;
        let to_siege_hq = r.i32()? != 0;
        let sweepable = r.i32()? != 0;
        let fixed_res = r.i32()? != 0;
        let to_fortress = r.i32()? != 0;
        Ok(Self {
            object_id,
            can_teleport,
            to_clan_hall,
            to_castle,
            to_siege_hq,
            sweepable,
            fixed_res,
            to_fortress,
        })
    }
}

// Java: gameserver/network/serverpackets/Revive.java
/// The creature is revived (`REVIVE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revive {
    pub object_id: i32,
}

impl Revive {
    pub const OPCODE: ServerOpcode = ServerOpcode::Revive;
}

impl Encode for Revive {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
    }
}

impl Decode for Revive {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { object_id: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/ChangeMoveType.java
/// Walking or running (`CHANGE_MOVE_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeMoveType {
    pub object_id: i32,
    pub running: bool,
}

impl ChangeMoveType {
    pub const OPCODE: ServerOpcode = ServerOpcode::ChangeMoveType;
    pub const WALK: i32 = 0;
    pub const RUN: i32 = 1;
}

impl Encode for ChangeMoveType {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        let move_type = if self.running { Self::RUN } else { Self::WALK };
        // Java writes a constant 0 after the move type (`c2`).
        w.i32(self.object_id).i32(move_type).i32(0);
    }
}

impl Decode for ChangeMoveType {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let running = r.i32()? == Self::RUN;
        r.skip(4)?; // constant 0
        Ok(Self { object_id, running })
    }
}

// Java: gameserver/network/serverpackets/ChangeWaitType.java
/// Sitting, standing or fake death (`CHANGE_WAIT_TYPE`), with the creature's position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeWaitType {
    pub object_id: i32,
    /// [`Self::WT_SITTING`] and the other `WT_*` values (Java `newMoveType`).
    pub move_type: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChangeWaitType {
    pub const OPCODE: ServerOpcode = ServerOpcode::ChangeWaitType;
    pub const WT_SITTING: i32 = 0;
    pub const WT_STANDING: i32 = 1;
    pub const WT_START_FAKEDEATH: i32 = 2;
    pub const WT_STOP_FAKEDEATH: i32 = 3;
}

impl Encode for ChangeWaitType {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.move_type)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z);
    }
}

impl Decode for ChangeWaitType {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            move_type: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExUseSharedGroupItem.java
/// The cooldown of a shared item group (`EX_USE_SHARED_GROUP_ITEM`, ex id `0x4A`), shown after an
/// item was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExUseSharedGroupItem {
    pub item_id: i32,
    pub group_id: i32,
    /// Seconds left (Java `remainingTime / 1000`).
    pub remaining_secs: i32,
    /// Seconds in total (Java `totalTime / 1000`).
    pub total_secs: i32,
}

impl ExUseSharedGroupItem {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExUseSharedGroupItem;

    /// Java constructor: the times come in milliseconds and go out in seconds. Java casts the
    /// long remaining time to `int`, which keeps the low 32 bits; `as` does the same.
    pub fn new(item_id: i32, group_id: i32, remaining_ms: i64, total_ms: i32) -> Self {
        Self {
            item_id,
            group_id,
            remaining_secs: (remaining_ms / 1000) as i32,
            total_secs: total_ms / 1000,
        }
    }
}

impl Encode for ExUseSharedGroupItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_id)
            .i32(self.group_id)
            .i32(self.remaining_secs)
            .i32(self.total_secs);
    }
}

impl Decode for ExUseSharedGroupItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            item_id: r.i32()?,
            group_id: r.i32()?,
            remaining_secs: r.i32()?,
            total_secs: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/SetupGauge.java
/// A progress bar on the client (`SETUP_GAUGE`): a bow's reload, a cast. Java writes the time
/// twice with the three-argument constructor (current and maximum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupGauge {
    pub object_id: i32,
    /// [`Self::BLUE`], [`Self::RED`] or [`Self::CYAN`] (Java `_dat1`).
    pub color: i32,
    /// Milliseconds (Java `_time`).
    pub time: i32,
    /// Milliseconds (Java `_time2`).
    pub max_time: i32,
}

impl SetupGauge {
    pub const OPCODE: ServerOpcode = ServerOpcode::SetupGauge;
    pub const BLUE: i32 = 0;
    pub const RED: i32 = 1;
    pub const CYAN: i32 = 2;

    /// Java `new SetupGauge(objectId, color, time)`: the time is both current and maximum.
    pub fn new(object_id: i32, color: i32, time: i32) -> Self {
        Self {
            object_id,
            color,
            time,
            max_time: time,
        }
    }
}

impl Encode for SetupGauge {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.color)
            .i32(self.time)
            .i32(self.max_time);
    }
}

impl Decode for SetupGauge {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            color: r.i32()?,
            time: r.i32()?,
            max_time: r.i32()?,
        })
    }
}

#[cfg(test)]
mod setup_gauge_tests {
    use super::*;

    #[test]
    fn setup_gauge_round_trip_and_layout() {
        let packet = SetupGauge::new(0x1234, SetupGauge::RED, 1551);
        let bytes = packet.to_bytes();
        // Opcode 0x6B, then four ints.
        assert_eq!(bytes[0], 0x6B);
        assert_eq!(bytes.len(), 1 + 16);
        assert_eq!(&bytes[5..9], &1_i32.to_le_bytes());
        assert_eq!(&bytes[9..13], &1551_i32.to_le_bytes());
        assert_eq!(&bytes[13..17], &1551_i32.to_le_bytes());
        let decoded = SetupGauge::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
    }
}
