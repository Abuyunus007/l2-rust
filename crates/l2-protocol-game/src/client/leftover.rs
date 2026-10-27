//! Client packets of L-02: macros, crystallize, the wear shop, item links in chat, the key
//! mapping, the inventory order, the mini map, rotating, the skill list and a skill on the ground.
//!
//! These only read the packet. What each one does is the world's job (`l2_world::leftover`).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

/// Java `RequestMakeMacro.MAX_MACRO_LENGTH`: the commands the packet keeps.
pub const MAX_MACRO_COMMANDS: usize = 12;
/// Java `RequestSaveInventoryOrder.LIMIT` ("client limit").
pub const MAX_INVENTORY_ORDER: usize = 125;
/// `RequestPreviewItem` returns on a longer list ("prevent too long lists").
pub const MAX_PREVIEW_ITEMS: i32 = 100;

// Java: gameserver/network/clientpackets/RequestMakeMacro.java (one command)
/// One command of a [`RequestMakeMacro`], as it came on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCommand {
    /// The running number (Java `entry`); Java never uses it.
    pub entry: u8,
    /// The raw type byte. Java maps a value outside 1 to 6 to type 0 (the world does).
    pub kind: u8,
    /// Java `d1`: the skill id of a skill command, the page of a shortcut.
    pub d1: i32,
    /// Java `d2`, one byte.
    pub d2: u8,
    /// Java `command`.
    pub command: String,
}

// Java: gameserver/network/clientpackets/RequestMakeMacro.java
/// The player saves a macro (`REQUEST_MAKE_MACRO`, 0xCD).
///
/// Java reads at most [`MAX_MACRO_COMMANDS`] commands and ignores the rest of the packet; a
/// longer list does not encode back the way it came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestMakeMacro {
    /// 0 for a new macro (the world gives it an id).
    pub id: i32,
    pub name: String,
    pub descr: String,
    pub acronym: String,
    /// Java `readByte()`, 0 to 255.
    pub icon: u8,
    pub commands: Vec<MacroCommand>,
}

impl RequestMakeMacro {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestMakeMacro;
}

impl Encode for RequestMakeMacro {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.id)
            .string(&self.name)
            .string(&self.descr)
            .string(&self.acronym)
            .u8(self.icon)
            .u8(self.commands.len() as u8);
        for c in &self.commands {
            w.u8(c.entry).u8(c.kind).i32(c.d1).u8(c.d2).string(&c.command);
        }
    }
}

impl Decode for RequestMakeMacro {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let id = r.i32()?;
        let name = r.string();
        let descr = r.string();
        let acronym = r.string();
        let icon = r.u8()?;
        let count = usize::from(r.u8()?).min(MAX_MACRO_COMMANDS);
        let mut commands = Vec::with_capacity(count);
        for _ in 0..count {
            commands.push(MacroCommand {
                entry: r.u8()?,
                kind: r.u8()?,
                d1: r.i32()?,
                d2: r.u8()?,
                command: r.string(),
            });
        }
        Ok(Self {
            id,
            name,
            descr,
            acronym,
            icon,
            commands,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestDeleteMacro.java
client_packet!(
    /// The player deletes a macro (`REQUEST_DELETE_MACRO`, 0xCE).
    RequestDeleteMacro,
    plain RequestDeleteMacro {
        /// The macro id.
        id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestCrystallizeItem.java
client_packet!(
    /// The player crystallizes an item (`REQUEST_CRYSTALLIZE_ITEM`, 0x2F).
    RequestCrystallizeItem,
    plain RequestCrystallizeItem {
        object_id: i32,
        count: i64,
    }
);

// Java: gameserver/network/clientpackets/RequestPreviewItem.java
/// The player tries items on in a wear shop (`REQUEST_PREVIEW_ITEM`, 0xC7).
///
/// Java reads a count, clamps a negative one to 0 and leaves the list null (`items` is `None`)
/// when the count is over [`MAX_PREVIEW_ITEMS`]; `runImpl` then does nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPreviewItem {
    /// Java `_unk`, never used.
    pub unknown: i32,
    /// The buy list id.
    pub list_id: i32,
    /// The item ids to try on.
    pub items: Option<Vec<i32>>,
}

impl RequestPreviewItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestPreviewItem;
}

impl Encode for RequestPreviewItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.unknown).i32(self.list_id);
        match &self.items {
            Some(items) => {
                w.i32(items.len() as i32);
                for item in items {
                    w.i32(*item);
                }
            }
            // A count Java refuses.
            None => {
                w.i32(MAX_PREVIEW_ITEMS + 1);
            }
        }
    }
}

impl Decode for RequestPreviewItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let unknown = r.i32()?;
        let list_id = r.i32()?;
        let count = r.i32()?.max(0);
        if count > MAX_PREVIEW_ITEMS {
            return Ok(Self {
                unknown,
                list_id,
                items: None,
            });
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(r.i32()?);
        }
        Ok(Self {
            unknown,
            list_id,
            items: Some(items),
        })
    }
}

// Java: gameserver/network/clientpackets/RequestExRqItemLink.java
client_packet!(
    /// The player clicks an item link in the chat (`0xD0` 0x001E, `REQUEST_EX_RQ_ITEM_LINK`).
    RequestExRqItemLink,
    ex RequestExRqItemLink {
        object_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestSaveKeyMapping.java
/// The client stores its key mapping (`0xD0` 0x0022, `REQUEST_SAVE_KEY_MAPPING`).
///
/// `None` when the size is not above 0 (Java then keeps no array and does nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSaveKeyMapping {
    pub ui_key_mapping: Option<Vec<u8>>,
}

impl RequestSaveKeyMapping {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestSaveKeyMapping;
}

impl Encode for RequestSaveKeyMapping {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        match &self.ui_key_mapping {
            Some(bytes) => {
                w.i32(bytes.len() as i32).bytes(bytes);
            }
            None => {
                w.i32(0);
            }
        }
    }
}

impl Decode for RequestSaveKeyMapping {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let size = r.i32()?;
        // `readBytes` throws when the packet is shorter than the size says.
        let ui_key_mapping = match usize::try_from(size) {
            Ok(len) if len > 0 => Some(r.bytes(len)?.to_vec()),
            _ => None,
        };
        Ok(Self { ui_key_mapping })
    }
}

// Java: gameserver/network/clientpackets/RequestSaveInventoryOrder.java
/// The client stores the order of its inventory window (`0xD0` 0x0024,
/// `REQUEST_SAVE_INVENTORY_ORDER`): pairs of object id and slot, at most [`MAX_INVENTORY_ORDER`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSaveInventoryOrder {
    /// `(object id, order)`.
    pub order: Vec<(i32, i32)>,
}

impl RequestSaveInventoryOrder {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestSaveInventoryOrder;
}

impl Encode for RequestSaveInventoryOrder {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.order.len() as i32);
        for (object_id, order) in &self.order {
            w.i32(*object_id).i32(*order);
        }
    }
}

impl Decode for RequestSaveInventoryOrder {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let size = r.i32()?;
        // `new ArrayList<>(sz)` throws for a negative size.
        let size = usize::try_from(size)
            .map_err(|_| PacketError::invalid("inventory order size", format!("negative: {size}")))?;
        let size = size.min(MAX_INVENTORY_ORDER);
        let mut order = Vec::with_capacity(size);
        for _ in 0..size {
            order.push((r.i32()?, r.i32()?));
        }
        Ok(Self { order })
    }
}

// Java: gameserver/network/clientpackets/RequestExMagicSkillUseGround.java
/// The player uses a skill on a point of the ground (`0xD0` 0x0044,
/// `REQUEST_EX_MAGIC_SKILL_USE_GROUND`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestExMagicSkillUseGround {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub skill_id: i32,
    /// Java `readInt() != 0`.
    pub ctrl_pressed: bool,
    /// Java `readByte() != 0`.
    pub shift_pressed: bool,
}

impl RequestExMagicSkillUseGround {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExMagicSkillUseGround;
}

impl Encode for RequestExMagicSkillUseGround {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.skill_id)
            .i32(i32::from(self.ctrl_pressed))
            .u8(u8::from(self.shift_pressed));
    }
}

impl Decode for RequestExMagicSkillUseGround {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            skill_id: r.i32()?,
            ctrl_pressed: r.i32()? != 0,
            shift_pressed: r.u8()? != 0,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestShowMiniMap.java
client_unit_packet!(
    /// The player opens the mini map (`REQUEST_SHOW_MINI_MAP`, 0x6C).
    RequestShowMiniMap,
    plain RequestShowMiniMap
);

// Java: gameserver/network/clientpackets/RequestRecordInfo.java
client_unit_packet!(
    /// The client asks for everything around it again (`REQUEST_RECORD_INFO`, 0x6E).
    RequestRecordInfo,
    plain RequestRecordInfo
);

// Java: gameserver/network/clientpackets/RequestSkillList.java
client_unit_packet!(
    /// The client asks for its skill list (`REQUEST_SKILL_LIST`, 0x50).
    RequestSkillList,
    plain RequestSkillList
);

// Java: gameserver/network/clientpackets/MoveWithDelta.java
client_packet!(
    /// A move by a difference (`MOVE_WITH_DELTA`, 0x52). Java reads it and does nothing
    /// ("TODO this").
    MoveWithDelta,
    plain MoveWithDelta {
        dx: i32,
        dy: i32,
        dz: i32,
    }
);

// Java: gameserver/network/clientpackets/StartRotating.java
client_packet!(
    /// The player starts to turn with the keyboard (`START_ROTATING`, 0x5B).
    StartRotating,
    plain StartRotating {
        degree: i32,
        side: i32,
    }
);

// Java: gameserver/network/clientpackets/FinishRotating.java
client_packet!(
    /// The player stops turning (`FINISH_ROTATING`, 0x5C).
    FinishRotating,
    plain FinishRotating {
        degree: i32,
        /// Java `readInt(); // Unknown.`
        unknown: i32,
    }
);
