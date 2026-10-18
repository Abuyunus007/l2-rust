//! `SystemMessage`: a message id from the generated table plus its typed parameters.
//!
//! Java: `gameserver/network/serverpackets/SystemMessage.java`. The text itself is never sent,
//! the client looks it up by id and fills the `$s1`, `$c1`, ... slots with the parameters.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;
use crate::generated::system_message_id::SystemMessageId;

/// Java `SystemMessage.TYPE_*`: the type int written before each parameter value.
mod param_type {
    pub const TEXT: i32 = 0;
    pub const INT_NUMBER: i32 = 1;
    pub const NPC_NAME: i32 = 2;
    pub const ITEM_NAME: i32 = 3;
    pub const SKILL_NAME: i32 = 4;
    pub const CASTLE_NAME: i32 = 5;
    pub const LONG_NUMBER: i32 = 6;
    pub const ZONE_NAME: i32 = 7;
    pub const ELEMENT_NAME: i32 = 9;
    pub const INSTANCE_NAME: i32 = 10;
    pub const DOOR_NAME: i32 = 11;
    pub const PLAYER_NAME: i32 = 12;
    pub const SYSTEM_STRING: i32 = 13;
}

/// Java `addNpcName(int id)` stores `1000000 + id` on the wire.
const NPC_NAME_OFFSET: i32 = 1_000_000;

/// One parameter of a [`SystemMessage`]. The variant is the Java `addXxx` method used.
///
/// Java's `addItemName` and `addSkillName` send plain text instead when the display id differs
/// from the id. That lookup needs game data, so the caller picks [`Self::Text`] in that case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessageParam {
    /// `addString`: a string.
    Text(String),
    /// `addPcName`: a player name (also a string on the wire).
    PlayerName(String),
    /// `addCastleId`: castle or fortress id.
    CastleName(i32),
    /// `addDoorName`: door id.
    DoorName(i32),
    /// `addNpcName(int)`: NPC template id. The wire value is `1000000 + id`.
    NpcName(i32),
    /// `addItemName(int)`: item template id.
    ItemName(i32),
    /// `addSkillName(int, int)`: skill id and level.
    SkillName { id: i32, level: i32 },
    /// `addElemental`: element type.
    ElementName(i32),
    /// `addSystemString`: system string id.
    SystemString(i32),
    /// `addInstanceName`: instance type id.
    InstanceName(i32),
    /// `addInt`: an int.
    IntNumber(i32),
    /// `addLong`: a long.
    LongNumber(i64),
    /// `addZoneName(int, int, int)`: a position.
    ZoneName { x: i32, y: i32, z: i32 },
}

impl SystemMessageParam {
    fn write(&self, w: &mut PacketWriter) {
        match self {
            Self::Text(text) => {
                w.i32(param_type::TEXT).string(text);
            }
            Self::PlayerName(name) => {
                w.i32(param_type::PLAYER_NAME).string(name);
            }
            Self::CastleName(id) => {
                w.i32(param_type::CASTLE_NAME).i32(*id);
            }
            Self::DoorName(id) => {
                w.i32(param_type::DOOR_NAME).i32(*id);
            }
            Self::NpcName(id) => {
                w.i32(param_type::NPC_NAME).i32(id.wrapping_add(NPC_NAME_OFFSET));
            }
            Self::ItemName(id) => {
                w.i32(param_type::ITEM_NAME).i32(*id);
            }
            Self::SkillName { id, level } => {
                w.i32(param_type::SKILL_NAME).i32(*id).i32(*level);
            }
            Self::ElementName(element) => {
                w.i32(param_type::ELEMENT_NAME).i32(*element);
            }
            Self::SystemString(id) => {
                w.i32(param_type::SYSTEM_STRING).i32(*id);
            }
            Self::InstanceName(id) => {
                w.i32(param_type::INSTANCE_NAME).i32(*id);
            }
            Self::IntNumber(value) => {
                w.i32(param_type::INT_NUMBER).i32(*value);
            }
            Self::LongNumber(value) => {
                w.i32(param_type::LONG_NUMBER).i64(*value);
            }
            Self::ZoneName { x, y, z } => {
                w.i32(param_type::ZONE_NAME).i32(*x).i32(*y).i32(*z);
            }
        }
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        Ok(match kind {
            param_type::TEXT => Self::Text(r.string()),
            param_type::PLAYER_NAME => Self::PlayerName(r.string()),
            param_type::CASTLE_NAME => Self::CastleName(r.i32()?),
            param_type::DOOR_NAME => Self::DoorName(r.i32()?),
            param_type::NPC_NAME => Self::NpcName(r.i32()?.wrapping_sub(NPC_NAME_OFFSET)),
            param_type::ITEM_NAME => Self::ItemName(r.i32()?),
            param_type::SKILL_NAME => Self::SkillName {
                id: r.i32()?,
                level: r.i32()?,
            },
            param_type::ELEMENT_NAME => Self::ElementName(r.i32()?),
            param_type::SYSTEM_STRING => Self::SystemString(r.i32()?),
            param_type::INSTANCE_NAME => Self::InstanceName(r.i32()?),
            param_type::INT_NUMBER => Self::IntNumber(r.i32()?),
            param_type::LONG_NUMBER => Self::LongNumber(r.i64()?),
            param_type::ZONE_NAME => Self::ZoneName {
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            },
            other => {
                return Err(PacketError::invalid(
                    "system message parameter type",
                    format!("unknown: {other}"),
                ));
            }
        })
    }
}

// Java: gameserver/network/serverpackets/SystemMessage.java
/// A system message: the id and the parameters in the order Java adds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemMessage {
    pub id: SystemMessageId,
    pub params: Vec<SystemMessageParam>,
}

impl SystemMessage {
    pub const OPCODE: ServerOpcode = ServerOpcode::SystemMessage;

    /// A message with no parameters yet. Add them with [`Self::with`].
    pub fn new(id: SystemMessageId) -> Self {
        Self {
            id,
            params: Vec::new(),
        }
    }

    /// Java `SystemMessage(String text)`: id `S1_2` (`$s1`) with the text as its only parameter.
    pub fn text(text: &str) -> Self {
        Self {
            id: SystemMessageId::S12,
            params: vec![SystemMessageParam::Text(text.to_string())],
        }
    }

    /// Appends one parameter, like the Java `addXxx` chain.
    #[must_use]
    pub fn with(mut self, param: SystemMessageParam) -> Self {
        self.params.push(param);
        self
    }
}

impl Encode for SystemMessage {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java writes `_params.length`: the declared count of the id, or more when the caller
        // added more parameters than declared. Parameters that were never added are not written,
        // which is the Java behaviour too.
        let count = self.params.len().max(usize::from(self.id.param_count()));
        w.i32(self.id.id()).i32(count as i32);
        for param in &self.params {
            param.write(w);
        }
    }
}

impl Decode for SystemMessage {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let raw_id = r.i32()?;
        let id = SystemMessageId::from_id(raw_id)
            .ok_or_else(|| PacketError::invalid("system message id", format!("unknown: {raw_id}")))?;
        let count = r.i32()?;
        let count = usize::try_from(count).map_err(|_| {
            PacketError::invalid("system message parameter count", format!("negative: {count}"))
        })?;
        let mut params = Vec::new();
        for _ in 0..count {
            params.push(SystemMessageParam::read(r)?);
        }
        Ok(Self { id, params })
    }
}
