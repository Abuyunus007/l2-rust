//! More server packets of the enter-world sequence: the macro list, the quest item list, the
//! rotation, the contact list, `ActionFailed`, `ExBrExtraUserInfo` and the two Nevit packets.
//!
//! Same rules as [`super::enterworld`]: values only, the caller prepares them.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{
    InventoryBlock, ItemInfo, read_inventory_block, read_item, write_inventory_block, write_item,
};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// Reads an `i32` count and checks that it is not negative.
fn read_count(r: &mut PacketReader, field: &'static str) -> Result<usize, PacketError> {
    let count = r.i32()?;
    usize::try_from(count).map_err(|_| PacketError::invalid(field, format!("negative: {count}")))
}

// Java: gameserver/network/serverpackets/SendMacroList.java (one `MacroCmd`)
/// One command of a [`MacroEntry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCommandEntry {
    /// Java `MacroType` ordinal (0 none, 1 skill, 2 action, 3 text, 4 shortcut, 5 item, 6 delay).
    pub kind: u8,
    /// Java `getD1()`: the skill id for a skill command.
    pub d1: i32,
    /// Java `getD2()`, written as one byte.
    pub d2: u8,
    /// Java `getCmd()`.
    pub cmd: String,
}

// Java: gameserver/network/serverpackets/SendMacroList.java (one `Macro`)
/// One macro of [`MacroList`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroEntry {
    pub id: i32,
    pub name: String,
    pub descr: String,
    pub acronym: String,
    /// Written as one byte.
    pub icon: u8,
    pub commands: Vec<MacroCommandEntry>,
}

// Java: gameserver/network/serverpackets/SendMacroList.java
/// One macro of the list (`MACRO_LIST`). Java sends one packet per macro, or a single packet
/// without a macro when the list is empty (`MacroList.sendUpdate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroList {
    /// Macro change revision.
    pub revision: i32,
    /// How many macros the whole list has (Java `writeByte(count)`).
    pub count: u8,
    pub macro_entry: Option<MacroEntry>,
}

impl MacroList {
    pub const OPCODE: ServerOpcode = ServerOpcode::MacroList;
}

impl Encode for MacroList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.revision);
        w.u8(0); // Java: "unknown"
        w.u8(self.count);
        w.u8(u8::from(self.macro_entry.is_some()));
        if let Some(m) = &self.macro_entry {
            w.i32(m.id).string(&m.name).string(&m.descr).string(&m.acronym);
            w.u8(m.icon);
            w.u8(m.commands.len() as u8);
            for (i, cmd) in m.commands.iter().enumerate() {
                // Java numbers the commands from 1.
                w.u8((i + 1) as u8)
                    .u8(cmd.kind)
                    .i32(cmd.d1)
                    .u8(cmd.d2)
                    .string(&cmd.cmd);
            }
        }
    }
}

impl Decode for MacroList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let revision = r.i32()?;
        r.skip(1)?;
        let count = r.u8()?;
        let macro_entry = if r.u8()? != 0 {
            let id = r.i32()?;
            let name = r.string();
            let descr = r.string();
            let acronym = r.string();
            let icon = r.u8()?;
            let commands = r.u8()?;
            let mut list = Vec::new();
            for _ in 0..commands {
                r.skip(1)?; // running number
                list.push(MacroCommandEntry {
                    kind: r.u8()?,
                    d1: r.i32()?,
                    d2: r.u8()?,
                    cmd: r.string(),
                });
            }
            Some(MacroEntry {
                id,
                name,
                descr,
                acronym,
                icon,
                commands: list,
            })
        } else {
            None
        };
        Ok(Self {
            revision,
            count,
            macro_entry,
        })
    }
}

// Java: gameserver/network/serverpackets/ExQuestItemList.java
/// The quest items of the inventory (`EX_QUEST_ITEM_LIST`). Java sends it right after
/// [`ItemList`](super::ItemList) (from `ItemList.runImpl`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExQuestItemList {
    pub items: Vec<ItemInfo>,
    pub block: Option<InventoryBlock>,
}

impl ExQuestItemList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExQuestItemList;
}

impl Encode for ExQuestItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u16(self.items.len() as u16);
        for item in &self.items {
            write_item(w, item);
        }
        write_inventory_block(w, self.block.as_ref());
    }
}

impl Decode for ExQuestItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
        }
        let block = read_inventory_block(r)?;
        Ok(Self { items, block })
    }
}

// Java: gameserver/network/serverpackets/ExRotation.java
/// Turns a character to a heading (`EX_ROTATION`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExRotation {
    pub char_id: i32,
    pub heading: i32,
}

impl ExRotation {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExRotation;
}

impl Encode for ExRotation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.char_id).i32(self.heading);
    }
}

impl Decode for ExRotation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            char_id: r.i32()?,
            heading: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExShowContactList.java
/// The mail contact list (opcode name `EX_RECEIVE_SHOW_POST_FRIEND`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExShowContactList {
    pub contacts: Vec<String>,
}

impl ExShowContactList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReceiveShowPostFriend;
}

impl Encode for ExShowContactList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.contacts.len() as i32);
        for name in &self.contacts {
            w.string(name);
        }
    }
}

impl Decode for ExShowContactList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "contact count")?;
        let mut contacts = Vec::new();
        for _ in 0..count {
            contacts.push(r.string());
        }
        Ok(Self { contacts })
    }
}

// Java: gameserver/network/serverpackets/ActionFailed.java
/// "Nothing happened": unlocks the client after a refused action (`ACTION_FAIL`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionFailed;

impl ActionFailed {
    pub const OPCODE: ServerOpcode = ServerOpcode::ActionFail;
}

impl Encode for ActionFailed {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ActionFailed {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExBrExtraUserInfo.java
/// Event effects and the lecture mark of a player (`EX_BR_EXTRA_USER_INFO`), sent after every
/// `UserInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExBrExtraUserInfo {
    pub object_id: i32,
    /// `getAbnormalVisualEffectEvent()`.
    pub abnormal_visual_effects_event: i32,
    /// Java: constant 1 ("TODO: Implement").
    pub lecture_mark: u8,
}

impl ExBrExtraUserInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBrExtraUserInfo;
}

impl Encode for ExBrExtraUserInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.abnormal_visual_effects_event)
            .u8(self.lecture_mark);
    }
}

impl Decode for ExBrExtraUserInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            abnormal_visual_effects_event: r.i32()?,
            lecture_mark: r.u8()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExNevitAdventPointInfoPacket.java
/// Nevit's blessing points (`EX_NAVIT_ADVENT_POINT_INFO`): 72 is 1%, 7200 is 100%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExNevitAdventPointInfo {
    pub points: i32,
}

impl ExNevitAdventPointInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExNavitAdventPointInfo;
}

impl Encode for ExNevitAdventPointInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.points);
    }
}

impl Decode for ExNevitAdventPointInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { points: r.i32()? })
    }
}

// Java: gameserver/network/serverpackets/ExNevitAdventTimeChange.java
/// The Nevit advent time left (`EX_NAVIT_ADVENT_TIME_CHANGE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExNevitAdventTimeChange {
    /// Java `writeByte(!paused)`.
    pub started: bool,
    pub time: i32,
}

impl ExNevitAdventTimeChange {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExNavitAdventTimeChange;
    /// Java's constructor caps the time at 14400.
    pub const MAX_TIME: i32 = 14_400;

    /// Java `new ExNevitAdventTimeChange(time, paused)`.
    pub fn new(time: i32, paused: bool) -> Self {
        Self {
            started: !paused,
            time: time.min(Self::MAX_TIME),
        }
    }
}

impl Encode for ExNevitAdventTimeChange {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u8(u8::from(self.started)).i32(self.time);
    }
}

impl Decode for ExNevitAdventTimeChange {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            started: r.u8()? != 0,
            time: r.i32()?,
        })
    }
}
