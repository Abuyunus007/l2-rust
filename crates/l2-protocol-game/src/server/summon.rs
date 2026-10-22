//! Server packets about summons and pets: the info window, the status updates, the pet
//! inventory and the servitor timer.
//!
//! Same rules as [`super::skills`]: values only. Everything Java computes from the `Summon`, the
//! `Pet` or the `Servitor` (speeds divided by the move multiplier, hit points, food, experience
//! ...) is prepared by the caller and arrives as plain fields.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, ItemUpdate, read_item, read_item_updates, write_item, write_item_updates};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Java adds this to a summon's template display id.
const SUMMON_ID_OFFSET: i32 = 1_000_000;

// Java: gameserver/network/serverpackets/PetInfo.java
/// The full state of a summon or pet (`PET_INFO`), sent to its owner.
#[derive(Debug, Clone, PartialEq)]
pub struct PetInfo {
    /// `getSummonType()`: 1 servitor, 2 pet.
    pub summon_type: i32,
    pub object_id: i32,
    /// The template's display id; the offset of 1000000 is added on the wire.
    pub display_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub m_atk_spd: i32,
    pub p_atk_spd: i32,
    /// The speeds are already divided by `move_multiplier` and rounded (Java `Math.round`).
    pub run_speed: i32,
    pub walk_speed: i32,
    pub swim_run_speed: i32,
    pub swim_walk_speed: i32,
    /// `isFlying() ? runSpeed : 0`; Java writes the pair twice.
    pub fly_run_speed: i32,
    pub fly_walk_speed: i32,
    pub move_multiplier: f64,
    pub attack_speed_multiplier: f64,
    pub collision_radius: f64,
    pub collision_height: f64,
    /// `getWeapon()`: the right hand item id.
    pub weapon: i32,
    /// `getArmor()`: the body armor item id.
    pub armor: i32,
    /// `getOwner() != null`: false when the pet is dead and its owner left the game.
    pub has_owner: bool,
    pub running: bool,
    pub in_combat: bool,
    pub alike_dead: bool,
    /// `0` teleported, `1` default, `2` summoned. Java writes 2 when `isShowSummonAnimation()`
    /// and the `value` it was built with otherwise.
    pub spawn_state: u8,
    /// A pet's name; a servitor's name only if its template uses a server side name, else `""`.
    pub name: String,
    /// The owner's name.
    pub title: String,
    pub pvp_flag: i32,
    pub karma: i32,
    /// A pet's food, or a servitor's remaining life time.
    pub cur_fed: i32,
    /// A pet's maximum food, or a servitor's life time.
    pub max_fed: i32,
    pub cur_hp: i32,
    pub max_hp: i32,
    pub cur_mp: i32,
    pub max_mp: i32,
    pub sp: i32,
    pub level: i32,
    pub exp: i64,
    /// `getExpForThisLevel()`. See the quirk note on the encoder.
    pub exp_for_this_level: i64,
    pub exp_for_next_level: i64,
    /// Pets only; servitors send `0`.
    pub weight: i32,
    pub max_load: i32,
    pub p_atk: i32,
    pub p_def: i32,
    pub m_atk: i32,
    pub m_def: i32,
    pub accuracy: i32,
    pub evasion: i32,
    pub critical: i32,
    pub move_speed: i32,
    pub atk_speed: i32,
    pub casting_speed: i32,
    pub abnormal_visual_effects: i32,
    /// `isMountable()`: shows the ride button.
    pub mountable: bool,
    /// `1` in water, `2` flying, `0` otherwise.
    pub water_or_flying: u8,
    /// `getTeam().getId()`.
    pub team_id: u8,
    pub soul_shots_per_hit: i32,
    pub spirit_shots_per_hit: i32,
    /// "CT1.5 Pet form and skills".
    pub form_id: i32,
    pub abnormal_visual_effect_special: i32,
}

impl PetInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::PetInfo;
}

impl Encode for PetInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.summon_type)
            .i32(self.object_id)
            .i32(self.display_id + SUMMON_ID_OFFSET)
            .i32(0) // 1 = attackable
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
            .f64(self.attack_speed_multiplier)
            .f64(self.collision_radius)
            .f64(self.collision_height)
            .i32(self.weapon)
            .i32(self.armor)
            .i32(0) // left hand weapon
            .u8(u8::from(self.has_owner))
            .u8(u8::from(self.running))
            .u8(u8::from(self.in_combat))
            .u8(u8::from(self.alike_dead))
            .u8(self.spawn_state)
            .i32(-1) // High Five NPCString id
            .string(&self.name)
            .i32(-1) // High Five NPCString id
            .string(&self.title)
            .i32(1)
            .i32(self.pvp_flag)
            .i32(self.karma)
            .i32(self.cur_fed)
            .i32(self.max_fed)
            .i32(self.cur_hp)
            .i32(self.max_hp)
            .i32(self.cur_mp)
            .i32(self.max_mp)
            .i32(self.sp)
            .i32(self.level)
            .i64(self.exp);
        // NOTE(java-quirk): "0% absolute value" is never above the current experience: Java
        // writes the experience itself when the level's start is higher (a summon whose
        // experience is below its level's start).
        w.i64(if self.exp_for_this_level > self.exp {
            self.exp
        } else {
            self.exp_for_this_level
        })
        .i64(self.exp_for_next_level)
        .i32(self.weight)
        .i32(self.max_load)
        .i32(self.p_atk)
        .i32(self.p_def)
        .i32(self.m_atk)
        .i32(self.m_def)
        .i32(self.accuracy)
        .i32(self.evasion)
        .i32(self.critical)
        .i32(self.move_speed)
        .i32(self.atk_speed)
        .i32(self.casting_speed)
        .i32(self.abnormal_visual_effects)
        .u16(u16::from(self.mountable))
        .u8(self.water_or_flying)
        .i16(0)
        .u8(self.team_id)
        .i32(self.soul_shots_per_hit)
        .i32(self.spirit_shots_per_hit)
        .i32(self.form_id)
        .i32(self.abnormal_visual_effect_special);
    }
}

impl Decode for PetInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let summon_type = r.i32()?;
        let object_id = r.i32()?;
        let display_id = r.i32()?.wrapping_sub(SUMMON_ID_OFFSET);
        r.skip(4)?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        let heading = r.i32()?;
        r.skip(4)?;
        let m_atk_spd = r.i32()?;
        let p_atk_spd = r.i32()?;
        let run_speed = r.i32()?;
        let walk_speed = r.i32()?;
        let swim_run_speed = r.i32()?;
        let swim_walk_speed = r.i32()?;
        let fly_run_speed = r.i32()?;
        let fly_walk_speed = r.i32()?;
        r.skip(8)?; // the fly speeds, written a second time
        let move_multiplier = r.f64()?;
        let attack_speed_multiplier = r.f64()?;
        let collision_radius = r.f64()?;
        let collision_height = r.f64()?;
        let weapon = r.i32()?;
        let armor = r.i32()?;
        r.skip(4)?;
        let has_owner = r.u8()? != 0;
        let running = r.u8()? != 0;
        let in_combat = r.u8()? != 0;
        let alike_dead = r.u8()? != 0;
        let spawn_state = r.u8()?;
        r.skip(4)?;
        let name = r.string();
        r.skip(4)?;
        let title = r.string();
        r.skip(4)?;
        Ok(Self {
            summon_type,
            object_id,
            display_id,
            x,
            y,
            z,
            heading,
            m_atk_spd,
            p_atk_spd,
            run_speed,
            walk_speed,
            swim_run_speed,
            swim_walk_speed,
            fly_run_speed,
            fly_walk_speed,
            move_multiplier,
            attack_speed_multiplier,
            collision_radius,
            collision_height,
            weapon,
            armor,
            has_owner,
            running,
            in_combat,
            alike_dead,
            spawn_state,
            name,
            title,
            pvp_flag: r.i32()?,
            karma: r.i32()?,
            cur_fed: r.i32()?,
            max_fed: r.i32()?,
            cur_hp: r.i32()?,
            max_hp: r.i32()?,
            cur_mp: r.i32()?,
            max_mp: r.i32()?,
            sp: r.i32()?,
            level: r.i32()?,
            exp: r.i64()?,
            exp_for_this_level: r.i64()?,
            exp_for_next_level: r.i64()?,
            weight: r.i32()?,
            max_load: r.i32()?,
            p_atk: r.i32()?,
            p_def: r.i32()?,
            m_atk: r.i32()?,
            m_def: r.i32()?,
            accuracy: r.i32()?,
            evasion: r.i32()?,
            critical: r.i32()?,
            move_speed: r.i32()?,
            atk_speed: r.i32()?,
            casting_speed: r.i32()?,
            abnormal_visual_effects: r.i32()?,
            mountable: r.u16()? != 0,
            water_or_flying: r.u8()?,
            team_id: {
                r.skip(2)?;
                r.u8()?
            },
            soul_shots_per_hit: r.i32()?,
            spirit_shots_per_hit: r.i32()?,
            form_id: r.i32()?,
            abnormal_visual_effect_special: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/AbstractNpcInfo.java (inner class `SummonInfo`)
/// Describes a summon or pet to a player who is not its owner (`NPC_INFO`, the summon variant).
///
/// Same opcode as [`super::NpcInfo`], but Java's `SummonInfo.writeImpl` writes its own layout: the
/// body armor in the chest slot, the running byte always 1, the owner's PvP flag and karma, the
/// pet form, and no display effect at the end (one int shorter than `NpcInfo`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SummonInfo {
    pub object_id: i32,
    /// `getTemplate().getDisplayId()`; `encode` adds 1000000.
    pub display_id: i32,
    /// `isAutoAttackable(attacker)`, written as an int.
    pub attackable: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub m_atk_spd: i32,
    pub p_atk_spd: i32,
    /// The speeds divided by the move multiplier (`AbstractNpcInfo`'s constructor).
    pub run_speed: i32,
    pub walk_speed: i32,
    pub swim_run_speed: i32,
    pub swim_walk_speed: i32,
    /// `isFlying() ? speed : 0`; written twice.
    pub fly_run_speed: i32,
    pub fly_walk_speed: i32,
    pub move_multiplier: f64,
    /// `getAttackSpeedMultiplier()`: a Java `float`, widened to double on the wire.
    pub attack_speed_multiplier: f32,
    /// `getTemplate().getFCollisionRadius()` / `getFCollisionHeight()`: floats; written twice.
    pub collision_radius: f32,
    pub collision_height: f32,
    /// `getWeapon()` (right hand).
    pub weapon: i32,
    /// `getArmor()` (the chest slot).
    pub armor: i32,
    pub in_combat: bool,
    pub alike_dead: bool,
    /// `_isSummoned ? 2 : _value`: 2 plays the summon animation, else 0 (teleported) or 1.
    pub spawn_state: u8,
    pub name: String,
    /// The owner's name while the owner is online, else empty.
    pub title: String,
    pub pvp_flag: i32,
    pub karma: i32,
    pub abnormal_visual_effects: i32,
    /// One byte: 1 inside a water zone, else 2 when flying, else 0.
    pub water_or_flying: u8,
    /// `getTeam().getId()`.
    pub team: u8,
    /// `getTemplate().getWeaponEnchant()`.
    pub enchant_effect: i32,
    /// `getFormId()`.
    pub form_id: i32,
    pub abnormal_visual_effect_special: i32,
}

impl SummonInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::NpcInfo;
}

impl Encode for SummonInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.display_id.wrapping_add(SUMMON_ID_OFFSET))
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
            .i32(self.weapon)
            .i32(self.armor)
            .i32(0) // left hand weapon
            .u8(1) // name above char
            .u8(1) // always running
            .bool(self.in_combat)
            .bool(self.alike_dead)
            .u8(self.spawn_state)
            .i32(-1) // High Five NPCString id
            .string(&self.name)
            .i32(-1) // High Five NPCString id
            .string(&self.title)
            .i32(1) // title color
            .i32(self.pvp_flag)
            .i32(self.karma)
            .i32(self.abnormal_visual_effects)
            .i32(0) // clan id
            .i32(0) // crest id
            .i32(0)
            .i32(0)
            .u8(self.water_or_flying)
            .u8(self.team)
            .f64(f64::from(self.collision_radius))
            .f64(f64::from(self.collision_height))
            .i32(self.enchant_effect)
            .i32(0)
            .i32(0)
            .i32(self.form_id)
            .u8(1)
            .u8(1)
            .i32(self.abnormal_visual_effect_special);
    }
}

impl Decode for SummonInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut p = Self {
            object_id: r.i32()?,
            display_id: r.i32()?.wrapping_sub(SUMMON_ID_OFFSET),
            attackable: r.i32()? != 0,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
            ..Self::default()
        };
        r.skip(4)?;
        p.m_atk_spd = r.i32()?;
        p.p_atk_spd = r.i32()?;
        p.run_speed = r.i32()?;
        p.walk_speed = r.i32()?;
        p.swim_run_speed = r.i32()?;
        p.swim_walk_speed = r.i32()?;
        p.fly_run_speed = r.i32()?;
        p.fly_walk_speed = r.i32()?;
        r.skip(8)?; // the fly speeds, written a second time
        p.move_multiplier = r.f64()?;
        p.attack_speed_multiplier = r.f64()? as f32;
        p.collision_radius = r.f64()? as f32;
        p.collision_height = r.f64()? as f32;
        p.weapon = r.i32()?;
        p.armor = r.i32()?;
        r.skip(4 + 2)?; // left hand, name above char, always running
        p.in_combat = r.bool()?;
        p.alike_dead = r.bool()?;
        p.spawn_state = r.u8()?;
        r.skip(4)?;
        p.name = r.string();
        r.skip(4)?;
        p.title = r.string();
        r.skip(4)?; // title color
        p.pvp_flag = r.i32()?;
        p.karma = r.i32()?;
        p.abnormal_visual_effects = r.i32()?;
        r.skip(4 * 4)?; // clan, crest, ally, ally crest
        p.water_or_flying = r.u8()?;
        p.team = r.u8()?;
        r.skip(2 * 8)?; // collision radius and height again
        p.enchant_effect = r.i32()?;
        r.skip(2 * 4)?;
        p.form_id = r.i32()?;
        r.skip(2)?;
        p.abnormal_visual_effect_special = r.i32()?;
        Ok(p)
    }
}

// Java: gameserver/network/serverpackets/PetStatusShow.java
server_packet!(
    /// Opens the pet status window (`PET_STATUS_SHOW`).
    PetStatusShow,
    PetStatusShow {
        summon_type: i32,
    }
);

// Java: gameserver/network/serverpackets/PetStatusUpdate.java
/// The numbers of the pet status window changed (`PET_STATUS_UPDATE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetStatusUpdate {
    pub summon_type: i32,
    pub object_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub title: String,
    /// A pet's food, or a servitor's remaining life time.
    pub cur_fed: i32,
    pub max_fed: i32,
    pub cur_hp: i32,
    pub max_hp: i32,
    pub cur_mp: i32,
    pub max_mp: i32,
    pub level: i32,
    pub exp: i64,
    pub exp_for_this_level: i64,
    pub exp_for_next_level: i64,
}

impl PetStatusUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::PetStatusUpdate;
}

impl Encode for PetStatusUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.summon_type)
            .i32(self.object_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .string(&self.title)
            .i32(self.cur_fed)
            .i32(self.max_fed)
            .i32(self.cur_hp)
            .i32(self.max_hp)
            .i32(self.cur_mp)
            .i32(self.max_mp)
            .i32(self.level)
            .i64(self.exp)
            .i64(self.exp_for_this_level)
            .i64(self.exp_for_next_level);
    }
}

impl Decode for PetStatusUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            summon_type: r.i32()?,
            object_id: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            title: r.string(),
            cur_fed: r.i32()?,
            max_fed: r.i32()?,
            cur_hp: r.i32()?,
            max_hp: r.i32()?,
            cur_mp: r.i32()?,
            max_mp: r.i32()?,
            level: r.i32()?,
            exp: r.i64()?,
            exp_for_this_level: r.i64()?,
            exp_for_next_level: r.i64()?,
        })
    }
}

// Java: gameserver/network/serverpackets/PetDelete.java
server_packet!(
    /// The summon is gone from the owner's screen (`PET_DELETE`).
    PetDelete,
    PetDelete {
        pet_type: i32,
        pet_object_id: i32,
    }
);

// Java: gameserver/network/serverpackets/SetSummonRemainTime.java
server_packet!(
    /// The remaining life time of a servitor (`SET_SUMMON_REMAIN_TIME`).
    SetSummonRemainTime,
    SetSummonRemainTime {
        max_time: i32,
        remaining_time: i32,
    }
);

// Java: gameserver/network/serverpackets/ExChangeNpcState.java
server_packet!(
    /// The state of an NPC changes (`EX_CHANGE_NPC_STATE`). A pet sends `0x64` to its owner when it
    /// runs out of food and `0x65` when it has food again (`Pet.setCurrentFed`).
    ExChangeNpcState,
    ExChangeNpcState {
        object_id: i32,
        state: i32,
    }
);

// Java: gameserver/network/serverpackets/PetItemList.java
/// The pet's inventory (`PET_ITEM_LIST`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PetItemList {
    pub items: Vec<ItemInfo>,
}

impl PetItemList {
    pub const OPCODE: ServerOpcode = ServerOpcode::PetItemList;
}

impl Encode for PetItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java `writeShort(size)`: the low 16 bits of the count.
        w.u16(self.items.len() as u16);
        for item in &self.items {
            write_item(w, item);
        }
    }
}

impl Decode for PetItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
        }
        Ok(Self { items })
    }
}

// Java: gameserver/network/serverpackets/PetInventoryUpdate.java
/// Changes to the pet's inventory (`PET_INVENTORY_UPDATE`): the entries of
/// [`super::InventoryUpdate`], with the pet's opcode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PetInventoryUpdate {
    pub updates: Vec<ItemUpdate>,
}

impl PetInventoryUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::PetInventoryUpdate;
}

impl Encode for PetInventoryUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_item_updates(w, &self.updates);
    }
}

impl Decode for PetInventoryUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            updates: read_item_updates(r)?,
        })
    }
}
