//! Server packets of skill enchanting.
//!
//! Values only, like the other server packets: the world works out the costs, the chances and the
//! routes (`l2_world::enchant::skill`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Java `Inventory.ADENA_ID`, the first of the two items `ExEnchantSkillInfoDetail` lists.
const ADENA_ID: i32 = 57;

// Java: gameserver/network/serverpackets/ExEnchantSkillInfo.java
/// The routes a skill can take from its level (`EX_ENCHANT_SKILL_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExEnchantSkillInfo {
    pub skill_id: i32,
    /// The level the player has.
    pub level: i32,
    /// True when the skill is at the last step of its route. Java writes `!_maxEnchanted`.
    pub max_enchanted: bool,
    /// The levels the client may pick next, current level first.
    pub routes: Vec<i32>,
}

impl ExEnchantSkillInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExEnchantSkillInfo;
}

impl Encode for ExEnchantSkillInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.skill_id).i32(self.level);
        w.i32(i32::from(!self.max_enchanted));
        // "enchanted?": the level is above 100.
        w.i32(i32::from(self.level > 100));
        w.i32(i32::try_from(self.routes.len()).unwrap_or(i32::MAX));
        for &level in &self.routes {
            w.i32(level);
        }
    }
}

impl Decode for ExEnchantSkillInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let skill_id = r.i32()?;
        let level = r.i32()?;
        let max_enchanted = r.i32()? == 0;
        let _enchanted = r.i32()?;
        let count = usize::try_from(r.i32()?).unwrap_or(0);
        let mut routes = Vec::new();
        for _ in 0..count {
            routes.push(r.i32()?);
        }
        Ok(Self {
            skill_id,
            level,
            max_enchanted,
            routes,
        })
    }
}

// Java: gameserver/network/serverpackets/ExEnchantSkillInfoDetail.java
/// The cost and chance of one step (`EX_ENCHANT_SKILL_INFO_DETAIL`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExEnchantSkillInfoDetail {
    /// 0 enchant, 1 safe enchant, 2 untrain, 3 change route.
    pub kind: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    /// The SP the step costs, with the cost multiplier of the kind already applied.
    pub sp: i32,
    /// The success chance in percent (Java comments it "exp").
    pub chance: i32,
    /// The adena the step costs, multiplier applied.
    pub adena: i32,
    /// The spellbook the step needs.
    pub book_id: i32,
    /// Whether the spellbook is really needed (the client shows it as a requirement).
    pub book_required: bool,
}

impl ExEnchantSkillInfoDetail {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExEnchantSkillInfoDetail;
}

impl Encode for ExEnchantSkillInfoDetail {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.kind)
            .i32(self.skill_id)
            .i32(self.skill_level)
            .i32(self.sp)
            .i32(self.chance);
        // "items count?": adena and the book.
        w.i32(2).i32(ADENA_ID).i32(self.adena);
        w.i32(self.book_id).i32(i32::from(self.book_required));
    }
}

impl Decode for ExEnchantSkillInfoDetail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        let skill_id = r.i32()?;
        let skill_level = r.i32()?;
        let sp = r.i32()?;
        let chance = r.i32()?;
        let _count = r.i32()?;
        let _adena_id = r.i32()?;
        let adena = r.i32()?;
        let book_id = r.i32()?;
        let book_required = r.i32()? != 0;
        Ok(Self {
            kind,
            skill_id,
            skill_level,
            sp,
            chance,
            adena,
            book_id,
            book_required,
        })
    }
}

// Java: gameserver/network/serverpackets/ExEnchantSkillList.java, mechanics/skill/enums/EnchantSkillType.java
/// A list of skills for one kind of enchant (`EX_ENCHANT_SKILL_LIST`). Java defines it and never
/// sends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExEnchantSkillList {
    /// `EnchantSkillType.ordinal()`: 0 NORMAL, 1 SAFE, 2 UNTRAIN, 3 CHANGE_ROUTE.
    pub kind: i32,
    /// `(skill id, skill level)`.
    pub skills: Vec<(i32, i32)>,
}

impl ExEnchantSkillList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExEnchantSkillList;
}

impl Encode for ExEnchantSkillList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.kind);
        w.i32(i32::try_from(self.skills.len()).unwrap_or(i32::MAX));
        for &(id, level) in &self.skills {
            w.i32(id).i32(level);
        }
    }
}

impl Decode for ExEnchantSkillList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        let count = usize::try_from(r.i32()?).unwrap_or(0);
        let mut skills = Vec::new();
        for _ in 0..count {
            skills.push((r.i32()?, r.i32()?));
        }
        Ok(Self { kind, skills })
    }
}

// Java: gameserver/network/serverpackets/ExEnchantSkillResult.java
/// Whether the skill changed (`EX_ENCHANT_SKILL_RESULT`): the client plays the success or the
/// failure effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExEnchantSkillResult {
    pub enchanted: bool,
}

impl ExEnchantSkillResult {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExEnchantSkillResult;
}

impl Encode for ExEnchantSkillResult {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.enchanted));
    }
}

impl Decode for ExEnchantSkillResult {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            enchanted: r.i32()? != 0,
        })
    }
}
