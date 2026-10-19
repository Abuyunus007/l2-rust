//! Skill, buff and item-use server packets: the casting packets (start, launch, cancel), the buff
//! icons, the short buff bar, the auto soulshot answer and the skill learning lists.
//!
//! Same rules as [`super::combat`]: these carry values only. The world decides which skill runs,
//! who the targets are and which buffs are in use, and passes the numbers in. Java's constructors
//! that fill in defaults (a self target, a missing target list) are the caller's job.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// A position in the world, as the casting packets send it (three ints, Java `getX/Y/Z`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SkillPosition {
    fn write(&self, w: &mut PacketWriter) {
        w.i32(self.x).i32(self.y).i32(self.z);
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/MagicSkillUse.java
/// A skill cast starts (`MAGIC_SKILL_USE`): the caster, the target, the skill and the cast time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicSkillUse {
    pub creature_id: i32,
    pub target_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    /// Milliseconds until the skill lands (Java `_hitTime`).
    pub hit_time: i32,
    /// Milliseconds of reuse delay shown on the skill icon (Java `_reuseDelay`).
    pub reuse_delay: i32,
    /// The caster's position (Java `_creature.getX/Y/Z`).
    pub creature: SkillPosition,
    /// The ground point a player aimed at (Java `getCurrentSkillWorldPosition`), `None` for
    /// everyone else.
    pub ground_location: Option<SkillPosition>,
    /// The target's position (Java `_target.getX/Y/Z`).
    pub target: SkillPosition,
}

impl MagicSkillUse {
    pub const OPCODE: ServerOpcode = ServerOpcode::MagicSkillUse;
}

impl Encode for MagicSkillUse {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.creature_id)
            .i32(self.target_id)
            .i32(self.skill_id)
            .i32(self.skill_level)
            .i32(self.hit_time)
            .i32(self.reuse_delay);
        self.creature.write(w);
        // NOTE(java-quirk): Java writes the constant `0` here. Its comment says
        // `isGroundTargetSkill ? 65535 : 0`, but the code never does the 65535 part.
        w.i16(0);
        match self.ground_location {
            None => {
                w.i16(0);
            }
            Some(ground) => {
                w.i16(1);
                ground.write(w);
            }
        }
        self.target.write(w);
    }
}

impl Decode for MagicSkillUse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let creature_id = r.i32()?;
        let target_id = r.i32()?;
        let skill_id = r.i32()?;
        let skill_level = r.i32()?;
        let hit_time = r.i32()?;
        let reuse_delay = r.i32()?;
        let creature = SkillPosition::read(r)?;
        // The ground-target flag is always 0 from Java (see `encode`), so it is read and dropped.
        r.skip(2)?;
        let ground_location = if r.i16()? != 0 {
            Some(SkillPosition::read(r)?)
        } else {
            None
        };
        Ok(Self {
            creature_id,
            target_id,
            skill_id,
            skill_level,
            hit_time,
            reuse_delay,
            creature,
            ground_location,
            target: SkillPosition::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/MagicSkillLaunched.java
/// The skill has been launched and hits its targets (`MAGIC_SKILL_LAUNCHED`). Sent after
/// [`MagicSkillUse`], when the cast time is over.
///
/// Java's constructors default `targets` to the caster (or to the single target when given); the
/// caller builds the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicSkillLaunched {
    pub creature_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    /// The object ids of the targets, in order.
    pub target_ids: Vec<i32>,
}

impl MagicSkillLaunched {
    pub const OPCODE: ServerOpcode = ServerOpcode::MagicSkillLaunched;
}

impl Encode for MagicSkillLaunched {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java `writeInt(size)`: the count is an int, so `as` keeps the same bits.
        w.i32(self.creature_id)
            .i32(self.skill_id)
            .i32(self.skill_level)
            .i32(self.target_ids.len() as i32);
        for &id in &self.target_ids {
            w.i32(id);
        }
    }
}

impl Decode for MagicSkillLaunched {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let creature_id = r.i32()?;
        let skill_id = r.i32()?;
        let skill_level = r.i32()?;
        let count = read_count(r, "magic skill targets")?;
        let mut target_ids = Vec::new();
        for _ in 0..count {
            target_ids.push(r.i32()?);
        }
        Ok(Self {
            creature_id,
            skill_id,
            skill_level,
            target_ids,
        })
    }
}

// Java: gameserver/network/serverpackets/MagicSkillCanceled.java
/// The cast of a skill was canceled (`MAGIC_SKILL_CANCELED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagicSkillCanceled {
    pub object_id: i32,
}

impl MagicSkillCanceled {
    pub const OPCODE: ServerOpcode = ServerOpcode::MagicSkillCanceled;
}

impl Encode for MagicSkillCanceled {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
    }
}

impl Decode for MagicSkillCanceled {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { object_id: r.i32()? })
    }
}

/// One buff or debuff icon: the display id and level of its skill, and the time left. This is
/// Java's `BuffInfo` as the buff packets see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectIcon {
    /// The skill's display id (Java `getDisplayId()`).
    pub display_id: i32,
    /// The skill's display level (Java `getDisplayLevel()`), sent as a short.
    pub display_level: i32,
    /// Seconds left (Java `getTime()`).
    pub time: i32,
    /// Java `BuffInfo.isInUse()`. Only icons in use are written, see [`AbnormalStatusUpdate`].
    pub in_use: bool,
}

impl EffectIcon {
    fn write(&self, w: &mut PacketWriter) {
        // Java `writeShort(getDisplayLevel())`: the short keeps the low 16 bits.
        w.i32(self.display_id)
            .i16(self.display_level as i16)
            .i32(self.time);
    }

    /// Every icon read off the wire is in use: Java writes only those.
    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            display_id: r.i32()?,
            display_level: i32::from(r.i16()?),
            time: r.i32()?,
            in_use: true,
        })
    }
}

// Java: gameserver/network/serverpackets/AbnormalStatusUpdate.java
/// The buff and debuff icons of the creature (`ABNORMAL_STATUS_UPDATE`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbnormalStatusUpdate {
    /// Java `_effects`, in the order the icons are shown.
    pub effects: Vec<EffectIcon>,
}

impl AbnormalStatusUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::AbnormalStatusUpdate;

    /// Java `addSkill`: a healing potion skill has no icon, so it is not added.
    pub fn add_skill(&mut self, effect: EffectIcon, is_healing_potion: bool) {
        if !is_healing_potion {
            self.effects.push(effect);
        }
    }
}

impl Encode for AbnormalStatusUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // NOTE(java-quirk): Java counts every icon in the list (`writeShort(_effects.size())`) but
        // writes only the ones in use, so a buff that is not in use makes the count too high.
        // Kept as Java does it.
        w.i16(self.effects.len() as i16);
        for effect in self.effects.iter().filter(|e| e.in_use) {
            effect.write(w);
        }
    }
}

impl Decode for AbnormalStatusUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = usize::from(r.u16()?);
        let mut effects = Vec::new();
        for _ in 0..count {
            effects.push(EffectIcon::read(r)?);
        }
        Ok(Self { effects })
    }
}

// Java: gameserver/network/serverpackets/ShortBuffStatusUpdate.java
/// The short buff bar, the one buff with a timer above the skill bar (`SHORT_BUFF_STATUS_UPDATE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortBuffStatusUpdate {
    pub skill_id: i32,
    pub skill_level: i32,
    /// Seconds (Java `_duration`).
    pub duration: i32,
}

impl ShortBuffStatusUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShortBuffStatusUpdate;
    /// Java `RESET_SHORT_BUFF`: hides the bar.
    pub const RESET_SHORT_BUFF: Self = Self {
        skill_id: 0,
        skill_level: 0,
        duration: 0,
    };
}

impl Encode for ShortBuffStatusUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.skill_id).i32(self.skill_level).i32(self.duration);
    }
}

impl Decode for ShortBuffStatusUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            skill_id: r.i32()?,
            skill_level: r.i32()?,
            duration: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExAutoSoulShot.java
/// The auto soulshot state of an item (`EX_AUTO_SOULSHOT`, ex id `0x0C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExAutoSoulShot {
    pub item_id: i32,
    /// [`Self::ON`] or [`Self::OFF`], as the client sent it (Java `_type`).
    pub shot_type: i32,
}

impl ExAutoSoulShot {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExAutoSoulshot;
    /// The shot is set to be used automatically.
    pub const ON: i32 = 1;
    /// The shot is no longer used automatically.
    pub const OFF: i32 = 0;
}

impl Encode for ExAutoSoulShot {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_id).i32(self.shot_type);
    }
}

impl Decode for ExAutoSoulShot {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            item_id: r.i32()?,
            shot_type: r.i32()?,
        })
    }
}

// Java: gameserver/mechanics/skill/enums/AcquireSkillType.java
/// The window a skill is learned in (Java `AcquireSkillType`). The wire value is the Java
/// `ordinal()`, so the order of the variants must not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AcquireSkillType {
    Class = 0,
    Fishing = 1,
    Pledge = 2,
    Subpledge = 3,
    Transform = 4,
    Transfer = 5,
    Subclass = 6,
    Collect = 7,
}

impl AcquireSkillType {
    /// Every type, in wire order.
    pub const ALL: [Self; 8] = [
        Self::Class,
        Self::Fishing,
        Self::Pledge,
        Self::Subpledge,
        Self::Transform,
        Self::Transfer,
        Self::Subclass,
        Self::Collect,
    ];

    /// The wire value (Java `ordinal()`).
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Java `values()[id]` throws for an id out of range, so an unknown id is `None` here.
    pub fn from_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }
}

// Java: gameserver/network/serverpackets/AcquireSkillList.java
/// One skill a trainer can teach in the list (Java `AcquireSkillList.Skill`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquireSkillEntry {
    pub id: i32,
    pub next_level: i32,
    pub max_level: i32,
    /// SP the level costs (Java `spCost`).
    pub sp_cost: i32,
    /// The requirement value shown with the skill (Java `requirements`, 0 in the normal lists).
    pub requirements: i32,
}

/// The list of skills a trainer teaches (`ACQUIRE_SKILL_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireSkillList {
    pub skill_type: AcquireSkillType,
    pub skills: Vec<AcquireSkillEntry>,
}

impl AcquireSkillList {
    pub const OPCODE: ServerOpcode = ServerOpcode::AcquireSkillList;

    pub fn new(skill_type: AcquireSkillType) -> Self {
        Self {
            skill_type,
            skills: Vec::new(),
        }
    }

    /// Java `addSkill`.
    pub fn add_skill(&mut self, id: i32, next_level: i32, max_level: i32, sp_cost: i32, requirements: i32) {
        self.skills.push(AcquireSkillEntry {
            id,
            next_level,
            max_level,
            sp_cost,
            requirements,
        });
    }
}

impl Encode for AcquireSkillList {
    fn encode(&self, w: &mut PacketWriter) {
        // Java returns before the opcode when the list is empty: nothing is sent at all.
        if self.skills.is_empty() {
            return;
        }
        write_opcode(w, Self::OPCODE);
        w.i32(self.skill_type.id()).i32(self.skills.len() as i32);
        for skill in &self.skills {
            w.i32(skill.id)
                .i32(skill.next_level)
                .i32(skill.max_level)
                .i32(skill.sp_cost)
                .i32(skill.requirements);
            if self.skill_type == AcquireSkillType::Subpledge {
                // Java writes a zero here with a `TODO: ?` comment. Nothing reads it.
                w.i32(0);
            }
        }
    }
}

impl Decode for AcquireSkillList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let skill_type = read_skill_type(r)?;
        let count = read_count(r, "skill count")?;
        let mut skills = Vec::new();
        for _ in 0..count {
            let entry = AcquireSkillEntry {
                id: r.i32()?,
                next_level: r.i32()?,
                max_level: r.i32()?,
                sp_cost: r.i32()?,
                requirements: r.i32()?,
            };
            if skill_type == AcquireSkillType::Subpledge {
                r.skip(4)?; // the zero Java writes
            }
            skills.push(entry);
        }
        Ok(Self { skill_type, skills })
    }
}

/// One requirement of a skill (Java `AcquireSkillInfo.Req`): an item and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquireSkillRequirement {
    /// The requirement type. Java always sends [`Self::ITEM_TYPE`] (99).
    pub req_type: i32,
    pub item_id: i32,
    /// Java `long count`.
    pub count: i64,
    /// Java `unk`. Java always sends [`Self::UNK`] (50).
    pub unk: i32,
}

impl AcquireSkillRequirement {
    /// Java's requirement type for an item.
    pub const ITEM_TYPE: i32 = 99;
    /// Java's constant second value of an item requirement.
    pub const UNK: i32 = 50;

    /// Java `new Req(99, itemId, count, 50)`.
    pub const fn item(item_id: i32, count: i64) -> Self {
        Self {
            req_type: Self::ITEM_TYPE,
            item_id,
            count,
            unk: Self::UNK,
        }
    }
}

// Java: gameserver/network/serverpackets/AcquireSkillInfo.java
/// The details of one skill to learn: its SP and the items it needs (`ACQUIRE_SKILL_INFO`).
///
/// Java's constructors pick which requirements are listed (the life crystal and divine
/// inspiration config switches); the caller builds the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireSkillInfo {
    pub skill_type: AcquireSkillType,
    pub skill_id: i32,
    pub level: i32,
    pub sp_cost: i32,
    pub requirements: Vec<AcquireSkillRequirement>,
}

impl AcquireSkillInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::AcquireSkillInfo;
}

impl Encode for AcquireSkillInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.skill_id)
            .i32(self.level)
            .i32(self.sp_cost)
            .i32(self.skill_type.id())
            .i32(self.requirements.len() as i32);
        for req in &self.requirements {
            w.i32(req.req_type).i32(req.item_id).i64(req.count).i32(req.unk);
        }
    }
}

impl Decode for AcquireSkillInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let skill_id = r.i32()?;
        let level = r.i32()?;
        let sp_cost = r.i32()?;
        let skill_type = read_skill_type(r)?;
        let count = read_count(r, "requirement count")?;
        let mut requirements = Vec::new();
        for _ in 0..count {
            requirements.push(AcquireSkillRequirement {
                req_type: r.i32()?,
                item_id: r.i32()?,
                count: r.i64()?,
                unk: r.i32()?,
            });
        }
        Ok(Self {
            skill_type,
            skill_id,
            level,
            sp_cost,
            requirements,
        })
    }
}

// Java: gameserver/network/serverpackets/AcquireSkillDone.java
/// The skill was learned: the learn window closes (`ACQUIRE_SKILL_DONE`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquireSkillDone;

impl AcquireSkillDone {
    pub const OPCODE: ServerOpcode = ServerOpcode::AcquireSkillDone;
}

impl Encode for AcquireSkillDone {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for AcquireSkillDone {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/PartySpelled.java
/// The buffs of a party member, shown on the party window (`PARTY_SPELLED`). Structure only: the
/// party code that sends it comes in M6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartySpelled {
    /// [`Self::PLAYER`], [`Self::PET`] or [`Self::SERVITOR`] (Java `isServitor ? 2 : isPet ? 1 : 0`).
    pub creature_type: i32,
    pub creature_id: i32,
    /// Java `_effects`, the same way as [`AbnormalStatusUpdate`].
    pub effects: Vec<EffectIcon>,
}

impl PartySpelled {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartySpelled;
    pub const PLAYER: i32 = 0;
    pub const PET: i32 = 1;
    pub const SERVITOR: i32 = 2;

    /// Java `addSkill`: every icon is added, with no healing potion filter.
    pub fn add_skill(&mut self, effect: EffectIcon) {
        self.effects.push(effect);
    }
}

impl Encode for PartySpelled {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // NOTE(java-quirk): the same as ABNORMAL_STATUS_UPDATE: the count is of every icon, the
        // icons written are only the ones in use.
        w.i32(self.creature_type)
            .i32(self.creature_id)
            .i32(self.effects.len() as i32);
        for effect in self.effects.iter().filter(|e| e.in_use) {
            effect.write(w);
        }
    }
}

impl Decode for PartySpelled {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let creature_type = r.i32()?;
        let creature_id = r.i32()?;
        let count = read_count(r, "effect count")?;
        let mut effects = Vec::new();
        for _ in 0..count {
            effects.push(EffectIcon::read(r)?);
        }
        Ok(Self {
            creature_type,
            creature_id,
            effects,
        })
    }
}

/// Reads an int count, which Java never writes negative. Anything negative is an error.
fn read_count(r: &mut PacketReader, field: &'static str) -> Result<usize, PacketError> {
    let count = r.i32()?;
    usize::try_from(count).map_err(|_| PacketError::invalid(field, format!("negative count {count}")))
}

fn read_skill_type(r: &mut PacketReader) -> Result<AcquireSkillType, PacketError> {
    let id = r.i32()?;
    AcquireSkillType::from_id(id)
        .ok_or_else(|| PacketError::invalid("acquire skill type", format!("unknown type {id}")))
}
