//! `TrapInfo`: the NPC info of a skill trap (a variant of `NPC_INFO`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

const NPC_TYPE_ID_OFFSET: i32 = 1_000_000;

// Java: gameserver/network/serverpackets/AbstractNpcInfo.java (inner class `TrapInfo`)
/// Describes a trap to a player who may see it (`NPC_INFO`, the trap variant).
///
/// Same opcode as [`super::NpcInfo`], its own layout: no weapons, the owner's name as title and
/// the owner's PvP flag and karma. Speeds are already divided by `move_multiplier` and rounded.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrapInfo {
    pub object_id: i32,
    /// `getTemplate().getDisplayId()`; `encode` adds 1000000.
    pub display_id: i32,
    /// `cha.isAutoAttackable(attacker)`.
    pub attackable: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub m_atk_spd: i32,
    pub p_atk_spd: i32,
    pub run_speed: i32,
    pub walk_speed: i32,
    pub swim_run_speed: i32,
    pub swim_walk_speed: i32,
    /// `isFlying() ? speed : 0`; written twice.
    pub fly_run_speed: i32,
    pub fly_walk_speed: i32,
    pub move_multiplier: f64,
    /// `getAttackSpeedMultiplier()`: a Java `float`, widened to double.
    pub attack_speed_multiplier: f32,
    pub collision_radius: f32,
    pub collision_height: f32,
    pub in_combat: bool,
    pub alike_dead: bool,
    /// `isShowSummonAnimation()`: Java writes 2 for true, else 0.
    pub show_summon_animation: bool,
    /// The template name when it uses a server side name, else empty.
    pub name: String,
    /// The owner's name, or empty.
    pub title: String,
    /// `getPvpFlag()` of the owner.
    pub pvp_flag: i32,
    /// `getKarma()` of the owner.
    pub karma: i32,
    pub abnormal_visual_effects: i32,
    /// `getTeam().getId()`.
    pub team: u8,
}

impl TrapInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::NpcInfo;
}

impl Encode for TrapInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.display_id.wrapping_add(NPC_TYPE_ID_OFFSET))
            .i32(i32::from(self.attackable))
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.heading)
            .i32(0)
            .i32(self.m_atk_spd)
            .i32(self.p_atk_spd)
            .i32(self.run_speed)
            .i32(self.walk_speed)
            .i32(self.swim_run_speed)
            .i32(self.swim_walk_speed)
            .i32(self.fly_run_speed)
            .i32(self.fly_walk_speed)
            .i32(self.fly_run_speed)
            .i32(self.fly_walk_speed)
            .f64(self.move_multiplier)
            .f64(f64::from(self.attack_speed_multiplier))
            .f64(f64::from(self.collision_radius))
            .f64(f64::from(self.collision_height))
            .i32(0) // right hand
            .i32(0) // chest
            .i32(0) // left hand
            .u8(1) // name above char
            .u8(1) // always running
            .bool(self.in_combat)
            .bool(self.alike_dead)
            .u8(if self.show_summon_animation { 2 } else { 0 })
            .i32(-1) // High Five NPCString id
            .string(&self.name)
            .i32(-1) // High Five NPCString id
            .string(&self.title)
            .i32(0) // title color, 0 = client default
            .i32(self.pvp_flag)
            .i32(self.karma)
            .i32(self.abnormal_visual_effects)
            .i32(0) // clan id
            .i32(0) // crest id
            .i32(0)
            .i32(0)
            .u8(0)
            .u8(self.team)
            .f64(f64::from(self.collision_radius))
            .f64(f64::from(self.collision_height))
            .i32(0)
            .i32(0)
            .i32(0)
            .i32(0)
            .u8(1)
            .u8(1)
            .i32(0);
    }
}

impl Decode for TrapInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut p = Self {
            object_id: r.i32()?,
            display_id: r.i32()?.wrapping_sub(NPC_TYPE_ID_OFFSET),
            attackable: r.i32()? != 0,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
            ..Self::default()
        };
        r.i32()?;
        p.m_atk_spd = r.i32()?;
        p.p_atk_spd = r.i32()?;
        p.run_speed = r.i32()?;
        p.walk_speed = r.i32()?;
        p.swim_run_speed = r.i32()?;
        p.swim_walk_speed = r.i32()?;
        p.fly_run_speed = r.i32()?;
        p.fly_walk_speed = r.i32()?;
        r.i32()?;
        r.i32()?;
        p.move_multiplier = r.f64()?;
        p.attack_speed_multiplier = r.f64()? as f32;
        p.collision_radius = r.f64()? as f32;
        p.collision_height = r.f64()? as f32;
        for _ in 0..3 {
            r.i32()?;
        }
        r.u8()?;
        r.u8()?;
        p.in_combat = r.bool()?;
        p.alike_dead = r.bool()?;
        p.show_summon_animation = r.u8()? == 2;
        r.i32()?;
        p.name = r.string();
        r.i32()?;
        p.title = r.string();
        r.i32()?;
        p.pvp_flag = r.i32()?;
        p.karma = r.i32()?;
        p.abnormal_visual_effects = r.i32()?;
        for _ in 0..4 {
            r.i32()?;
        }
        r.u8()?;
        p.team = r.u8()?;
        r.f64()?;
        r.f64()?;
        for _ in 0..4 {
            r.i32()?;
        }
        r.u8()?;
        r.u8()?;
        r.i32()?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trap_info_round_trips() {
        let info = TrapInfo {
            object_id: 268_480_001,
            display_id: 13_071,
            attackable: true,
            x: -80_000,
            y: 150_000,
            z: -3_000,
            heading: 16_384,
            m_atk_spd: 333,
            p_atk_spd: 300,
            run_speed: 120,
            walk_speed: 50,
            swim_run_speed: 120,
            swim_walk_speed: 50,
            move_multiplier: 1.0,
            attack_speed_multiplier: 1.0,
            collision_radius: 8.0,
            collision_height: 14.0,
            title: "Owner".to_owned(),
            pvp_flag: 1,
            karma: 5,
            team: 2,
            ..TrapInfo::default()
        };
        let bytes = info.to_bytes();
        let mut r = PacketReader::new(&bytes);
        r.u8().unwrap();
        assert_eq!(TrapInfo::decode(&mut r).unwrap(), info);
    }
}
