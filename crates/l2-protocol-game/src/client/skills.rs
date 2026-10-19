//! Client packets of skills and items: casting a skill, the auto soulshot switch, the skill
//! learning requests and the dispel of a buff.
//!
//! These only read the packet. What a cast, a learn or a dispel does is the world's job (the
//! handlers of M5-CAST-01, M5-SHOT-01, M5-LEARN-01 and the effect engine).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};
use crate::server::skills::AcquireSkillType;

// Java: gameserver/network/clientpackets/RequestMagicSkillUse.java
/// The player uses a skill (`REQUEST_MAGIC_SKILL_USE`). The id is the skill the player knows, or
/// the display id of a transformed or custom skill; the world decides which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestMagicSkillUse {
    pub magic_id: i32,
    /// Ctrl held: a force attack (Java `readInt() != 0`).
    pub ctrl_pressed: bool,
    /// Shift held (Java `readByte() != 0`).
    pub shift_pressed: bool,
}

impl RequestMagicSkillUse {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestMagicSkillUse;
}

impl Encode for RequestMagicSkillUse {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.magic_id)
            .i32(i32::from(self.ctrl_pressed))
            .u8(u8::from(self.shift_pressed));
    }
}

impl Decode for RequestMagicSkillUse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let magic_id = r.i32()?;
        // Java: any non-zero value counts as pressed.
        let ctrl_pressed = r.i32()? != 0;
        let shift_pressed = r.u8()? != 0;
        Ok(Self {
            magic_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestAutoSoulShot.java
/// The player switches the automatic use of a soulshot or spiritshot on or off. Ex packet
/// `0xD0 0x000D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAutoSoulShot {
    pub item_id: i32,
    /// [`Self::ON`] or [`Self::OFF`]. Java acts on those two values only; any other value does
    /// nothing, so it is kept as sent.
    pub shot_type: i32,
}

impl RequestAutoSoulShot {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestAutoSoulshot;
    /// Java `_type == 1`: turn the automatic use on.
    pub const ON: i32 = 1;
    /// Java `_type == 0`: turn the automatic use off.
    pub const OFF: i32 = 0;
}

impl Encode for RequestAutoSoulShot {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.item_id).i32(self.shot_type);
    }
}

impl Decode for RequestAutoSoulShot {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            item_id: r.i32()?,
            shot_type: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestAcquireSkill.java
/// The player asks to learn a skill from a trainer (`REQUEST_ACQUIRE_SKILL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAcquireSkill {
    pub id: i32,
    pub level: i32,
    pub skill_type: AcquireSkillType,
    /// The squad (Java `_subType`). Only read and written for [`AcquireSkillType::Subpledge`];
    /// it is 0 for every other type.
    pub sub_type: i32,
}

impl RequestAcquireSkill {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestAcquireSkill;
}

impl Encode for RequestAcquireSkill {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.id).i32(self.level).i32(self.skill_type.id());
        if self.skill_type == AcquireSkillType::Subpledge {
            w.i32(self.sub_type);
        }
    }
}

impl Decode for RequestAcquireSkill {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let id = r.i32()?;
        let level = r.i32()?;
        let skill_type = read_skill_type(r)?;
        let sub_type = if skill_type == AcquireSkillType::Subpledge {
            r.i32()?
        } else {
            0
        };
        Ok(Self {
            id,
            level,
            skill_type,
            sub_type,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestAcquireSkillInfo.java
/// The player asks for the details of a skill to learn (`REQUEST_ACQUIRE_SKILL_INFO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAcquireSkillInfo {
    pub id: i32,
    pub level: i32,
    pub skill_type: AcquireSkillType,
}

impl RequestAcquireSkillInfo {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestAcquireSkillInfo;
}

impl Encode for RequestAcquireSkillInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.id).i32(self.level).i32(self.skill_type.id());
    }
}

impl Decode for RequestAcquireSkillInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            id: r.i32()?,
            level: r.i32()?,
            skill_type: read_skill_type(r)?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestDispel.java
/// The player removes a buff from itself or from its summon (`REQUEST_DISPEL`). Ex packet
/// `0xD0 0x004B`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestDispel {
    /// The player's own object id, or the summon's.
    pub object_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
}

impl RequestDispel {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestDispel;
}

impl Encode for RequestDispel {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.object_id).i32(self.skill_id).i32(self.skill_level);
    }
}

impl Decode for RequestDispel {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            skill_id: r.i32()?,
            skill_level: r.i32()?,
        })
    }
}

/// Java `AcquireSkillType.getAcquireSkillType`: `values()[id]` throws on an unknown id, which
/// drops the packet. Here it is a decode error.
fn read_skill_type(r: &mut PacketReader) -> Result<AcquireSkillType, PacketError> {
    let id = r.i32()?;
    AcquireSkillType::from_id(id)
        .ok_or_else(|| PacketError::invalid("acquire skill type", format!("unknown type {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_packet_writes_its_opcode_and_fields() {
        let skill = RequestMagicSkillUse {
            magic_id: 0x499,
            ctrl_pressed: true,
            shift_pressed: false,
        };
        assert_eq!(skill.to_bytes(), [0x39, 0x99, 0x04, 0, 0, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn ex_packets_write_0xd0_and_the_ex_id() {
        let shot = RequestAutoSoulShot {
            item_id: 1,
            shot_type: RequestAutoSoulShot::ON,
        };
        assert_eq!(shot.to_bytes(), [0xD0, 0x0D, 0x00, 1, 0, 0, 0, 1, 0, 0, 0]);
        let dispel = RequestDispel {
            object_id: 2,
            skill_id: 3,
            skill_level: 4,
        };
        assert_eq!(dispel.to_bytes()[..3], [0xD0, 0x4B, 0x00]);
    }
}
