//! Small server packets that EnterWorld sends to a plain character.
//!
//! Same rules as [`super::charscreen`]: these packets carry values only. The caller reads the
//! player (skills, shortcuts, friends, ...) and passes the results in. The one exception with
//! logic is [`SkillCoolTime`], whose helpers do the arithmetic Java does while writing.
//! [`super::system_message`] holds `SystemMessage`, which EnterWorld also sends.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::npc_string_id::NpcStringId;
use crate::generated::opcodes::ServerOpcode;
use crate::generated::system_message_id::SystemMessageId;

/// Reads an `i32` count and checks that it is not negative.
fn read_count(r: &mut PacketReader, field: &'static str) -> Result<usize, PacketError> {
    let count = r.i32()?;
    usize::try_from(count).map_err(|_| PacketError::invalid(field, format!("negative: {count}")))
}

// Java: gameserver/network/serverpackets/SkillList.java
/// One skill of [`SkillList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillListEntry {
    pub id: i32,
    pub level: i32,
    pub passive: bool,
    pub disabled: bool,
    pub enchanted: bool,
}

// Java: gameserver/network/serverpackets/SkillList.java
/// The skills the character knows. `passive` is written as an int, `disabled` and `enchanted`
/// as bytes (Java `writeInt(boolean)` / `writeByte(boolean)`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillList {
    pub skills: Vec<SkillListEntry>,
}

impl SkillList {
    pub const OPCODE: ServerOpcode = ServerOpcode::SkillList;
}

impl Encode for SkillList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.skills.len() as i32);
        for skill in &self.skills {
            w.i32(i32::from(skill.passive)).i32(skill.level).i32(skill.id);
            w.u8(u8::from(skill.disabled)).u8(u8::from(skill.enchanted));
        }
    }
}

impl Decode for SkillList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "skill count")?;
        let mut skills = Vec::new();
        for _ in 0..count {
            let passive = r.i32()? != 0;
            let level = r.i32()?;
            let id = r.i32()?;
            let disabled = r.u8()? != 0;
            let enchanted = r.u8()? != 0;
            skills.push(SkillListEntry {
                id,
                level,
                passive,
                disabled,
                enchanted,
            });
        }
        Ok(Self { skills })
    }
}

// Java: gameserver/network/serverpackets/ShortcutInit.java
/// One shortcut of [`ShortcutInit`]. `slot` is the wire slot: Java writes
/// `slot + page * 12`, so the caller passes that sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutEntry {
    /// `ShortcutType.NONE`: only the type and the slot go on the wire.
    Empty { slot: i32 },
    /// `ShortcutType.ITEM`: the item id and its shared reuse group.
    Item {
        slot: i32,
        id: i32,
        shared_reuse_group: i32,
    },
    /// `ShortcutType.SKILL`: the skill id and level.
    Skill { slot: i32, id: i32, level: i32 },
    /// Action, macro, recipe or bookmark: the id only.
    Other { kind: OtherShortcut, slot: i32, id: i32 },
}

/// The shortcut kinds that carry only an id. The values are the Java `ShortcutType` ordinals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtherShortcut {
    Action = 3,
    Macro = 4,
    Recipe = 5,
    Bookmark = 6,
}

impl ShortcutEntry {
    const TYPE_NONE: i32 = 0;
    const TYPE_ITEM: i32 = 1;
    const TYPE_SKILL: i32 = 2;

    fn write(&self, w: &mut PacketWriter) {
        match *self {
            Self::Empty { slot } => {
                w.i32(Self::TYPE_NONE).i32(slot);
            }
            Self::Item {
                slot,
                id,
                shared_reuse_group,
            } => {
                w.i32(Self::TYPE_ITEM).i32(slot);
                // Java: id, constant 1, shared reuse group, two zero ints, two zero shorts.
                w.i32(id).i32(1).i32(shared_reuse_group).i32(0).i32(0);
                w.u16(0).u16(0);
            }
            Self::Skill { slot, id, level } => {
                w.i32(Self::TYPE_SKILL).i32(slot);
                w.i32(id).i32(level).u8(0).i32(1); // Java: "C5" zero byte, "C6" constant 1
            }
            Self::Other { kind, slot, id } => {
                w.i32(kind as i32).i32(slot).i32(id).i32(1); // Java: "C6" constant 1
            }
        }
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        let slot = r.i32()?;
        match kind {
            Self::TYPE_NONE => Ok(Self::Empty { slot }),
            Self::TYPE_ITEM => {
                let id = r.i32()?;
                r.skip(4)?; // constant 1
                let shared_reuse_group = r.i32()?;
                r.skip(4 + 4 + 2 + 2)?;
                Ok(Self::Item {
                    slot,
                    id,
                    shared_reuse_group,
                })
            }
            Self::TYPE_SKILL => {
                let id = r.i32()?;
                let level = r.i32()?;
                r.skip(1 + 4)?;
                Ok(Self::Skill { slot, id, level })
            }
            3..=6 => {
                let other = match kind {
                    3 => OtherShortcut::Action,
                    4 => OtherShortcut::Macro,
                    5 => OtherShortcut::Recipe,
                    _ => OtherShortcut::Bookmark,
                };
                let id = r.i32()?;
                r.skip(4)?;
                Ok(Self::Other {
                    kind: other,
                    slot,
                    id,
                })
            }
            other => Err(PacketError::invalid("shortcut type", format!("unknown: {other}"))),
        }
    }
}

/// The shortcut bar of the character.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShortcutInit {
    pub shortcuts: Vec<ShortcutEntry>,
}

impl ShortcutInit {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShortCutInit;
}

impl Encode for ShortcutInit {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.shortcuts.len() as i32);
        for shortcut in &self.shortcuts {
            shortcut.write(w);
        }
    }
}

impl Decode for ShortcutInit {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "shortcut count")?;
        let mut shortcuts = Vec::new();
        for _ in 0..count {
            shortcuts.push(ShortcutEntry::read(r)?);
        }
        Ok(Self { shortcuts })
    }
}

// Java: gameserver/network/serverpackets/QuestList.java
/// One quest of [`QuestList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestListEntry {
    pub quest_id: i32,
    /// The Java quest state word (`__compltdStateFlags`, or the condition when that is 0). The
    /// bit rules are in the Java comment on `QuestList.writeImpl`; the caller computes it.
    pub status: i32,
}

// Java: gameserver/network/serverpackets/QuestList.java
/// The quest journal. Java ends the packet with 128 zero bytes, which this packet writes too.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestList {
    pub quests: Vec<QuestListEntry>,
}

impl QuestList {
    pub const OPCODE: ServerOpcode = ServerOpcode::QuestList;
    /// Trailing zero bytes after the quest entries (Java `new byte[128]`).
    const TRAILER_LEN: usize = 128;
}

impl Encode for QuestList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java `writeShort(size)`: the count wraps like a short.
        w.u16(self.quests.len() as u16);
        for quest in &self.quests {
            w.i32(quest.quest_id).i32(quest.status);
        }
        w.bytes(&[0; Self::TRAILER_LEN]);
    }
}

impl Decode for QuestList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.u16()?;
        let mut quests = Vec::new();
        for _ in 0..count {
            let quest_id = r.i32()?;
            let status = r.i32()?;
            quests.push(QuestListEntry { quest_id, status });
        }
        r.skip(Self::TRAILER_LEN)?;
        Ok(Self { quests })
    }
}

// Java: gameserver/network/serverpackets/HennaInfo.java
/// The dye (henna) summary of the character.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HennaInfo {
    /// Stat bonus of the equipped dyes, in Java's wire order INT, STR, CON, MEN, DEX, WIT.
    /// Java `writeByte(int)`: the caller passes the value that fits one byte.
    pub int: i8,
    pub str: i8,
    pub con: i8,
    pub men: i8,
    pub dex: i8,
    pub wit: i8,
    /// Dye item ids of the slots in use (Java `getDyeId()`, empty slots left out).
    pub dye_ids: Vec<i32>,
}

impl HennaInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::HennaInfo;
    /// Java writes the constant 3 slots.
    const SLOTS: i32 = 3;
}

impl Encode for HennaInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i8(self.int)
            .i8(self.str)
            .i8(self.con)
            .i8(self.men)
            .i8(self.dex)
            .i8(self.wit);
        w.i32(Self::SLOTS);
        w.i32(self.dye_ids.len() as i32);
        for dye_id in &self.dye_ids {
            w.i32(*dye_id).i32(1); // Java: constant 1 after each dye id
        }
    }
}

impl Decode for HennaInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let (int, str, con, men, dex, wit) = (r.i8()?, r.i8()?, r.i8()?, r.i8()?, r.i8()?, r.i8()?);
        r.skip(4)?; // slots
        let count = read_count(r, "henna count")?;
        let mut dye_ids = Vec::new();
        for _ in 0..count {
            dye_ids.push(r.i32()?);
            r.skip(4)?; // constant 1
        }
        Ok(Self {
            int,
            str,
            con,
            men,
            dex,
            wit,
            dye_ids,
        })
    }
}

// Java: gameserver/network/serverpackets/FriendList.java (one `FriendInfo`)
/// One friend of [`FriendList`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendListEntry {
    pub object_id: i32,
    pub name: String,
    pub online: bool,
}

// Java: gameserver/network/serverpackets/FriendList.java
/// The friend list. Java sends it as `L2_FRIEND_LIST` (`0x75`), not `FRIEND_LIST` (`0x58`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FriendList {
    pub friends: Vec<FriendListEntry>,
}

impl FriendList {
    pub const OPCODE: ServerOpcode = ServerOpcode::L2FriendList;
}

impl Encode for FriendList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.friends.len() as i32);
        for friend in &self.friends {
            // The last int is the object id when online, else 0.
            let online_object_id = if friend.online { friend.object_id } else { 0 };
            w.i32(friend.object_id)
                .string(&friend.name)
                .i32(i32::from(friend.online))
                .i32(online_object_id);
        }
    }
}

impl Decode for FriendList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "friend count")?;
        let mut friends = Vec::new();
        for _ in 0..count {
            let object_id = r.i32()?;
            let name = r.string();
            let online = r.i32()? != 0;
            r.skip(4)?; // object id when online, derived again on encode
            friends.push(FriendListEntry {
                object_id,
                name,
                online,
            });
        }
        Ok(Self { friends })
    }
}

// Java: gameserver/network/serverpackets/EtcStatusUpdate.java
/// The status icons of the character (weight, dangers, penalties, charges).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EtcStatusUpdate {
    /// Force charges, 1 to 7 (the level of the force).
    pub charges: i32,
    /// Weight penalty level, 0 to 4.
    pub weight_penalty: i32,
    /// Java: refusing messages, chat banned or silenced.
    pub chat_blocked: bool,
    /// The player stands in a danger area.
    pub danger_area: bool,
    pub expertise_weapon_penalty: i32,
    pub expertise_armor_penalty: i32,
    /// Charm of courage: resurrection on the siege battlefield.
    pub charm_of_courage: bool,
    /// Death penalty level, 0 to 15.
    pub death_penalty_level: i32,
    pub charged_souls: i32,
}

impl EtcStatusUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::EtcStatusUpdate;
}

impl Encode for EtcStatusUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.charges)
            .i32(self.weight_penalty)
            .i32(i32::from(self.chat_blocked))
            .i32(i32::from(self.danger_area))
            .i32(self.expertise_weapon_penalty)
            .i32(self.expertise_armor_penalty)
            .i32(i32::from(self.charm_of_courage))
            .i32(self.death_penalty_level)
            .i32(self.charged_souls);
    }
}

impl Decode for EtcStatusUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            charges: r.i32()?,
            weight_penalty: r.i32()?,
            chat_blocked: r.i32()? != 0,
            danger_area: r.i32()? != 0,
            expertise_weapon_penalty: r.i32()?,
            expertise_armor_penalty: r.i32()?,
            charm_of_courage: r.i32()? != 0,
            death_penalty_level: r.i32()?,
            charged_souls: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExStorageMaxCount.java
/// The slot limits of the storages (`EX_STORAGE_MAX_COUNT`). The values come from the player
/// and [`PlayerConfig`] in Java; the caller passes them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExStorageMaxCount {
    pub inventory: i32,
    pub warehouse: i32,
    pub clan: i32,
    pub private_sell: i32,
    pub private_buy: i32,
    pub dwarven_recipe: i32,
    pub common_recipe: i32,
    /// Extra inventory slots from the belt (`INV_LIM` stat).
    pub inventory_extra_slots: i32,
    pub inventory_quest_items: i32,
}

impl ExStorageMaxCount {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExStorageMaxCount;
}

impl Encode for ExStorageMaxCount {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.inventory)
            .i32(self.warehouse)
            .i32(self.clan)
            .i32(self.private_sell)
            .i32(self.private_buy)
            .i32(self.dwarven_recipe)
            .i32(self.common_recipe)
            .i32(self.inventory_extra_slots)
            .i32(self.inventory_quest_items);
    }
}

impl Decode for ExStorageMaxCount {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            inventory: r.i32()?,
            warehouse: r.i32()?,
            clan: r.i32()?,
            private_sell: r.i32()?,
            private_buy: r.i32()?,
            dwarven_recipe: r.i32()?,
            common_recipe: r.i32()?,
            inventory_extra_slots: r.i32()?,
            inventory_quest_items: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExBasicActionList.java
/// The action bar actions (`EX_BASIC_ACTION_LIST`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExBasicActionList {
    pub action_ids: Vec<i32>,
}

impl ExBasicActionList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBasicActionList;

    /// Java `ExBasicActionList.STATIC_PACKET`: ids `0..74`, then `1000..1099`, then `5000..5016`.
    pub fn default_list() -> Self {
        Self {
            action_ids: (0..74).chain(1000..1099).chain(5000..5016).collect(),
        }
    }
}

impl Encode for ExBasicActionList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.action_ids.len() as i32);
        for action_id in &self.action_ids {
            w.i32(*action_id);
        }
    }
}

impl Decode for ExBasicActionList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "action count")?;
        let mut action_ids = Vec::new();
        for _ in 0..count {
            action_ids.push(r.i32()?);
        }
        Ok(Self { action_ids })
    }
}

// Java: gameserver/network/serverpackets/ExVoteSystemInfo.java
/// Recommendations and the Nevit hourglass bonus (`EX_VOTE_SYSTEM_INFO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExVoteSystemInfo {
    pub recom_left: i32,
    pub recom_have: i32,
    pub bonus_time: i32,
    pub bonus_value: i32,
    pub bonus_type: i32,
}

impl ExVoteSystemInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExVoteSystemInfo;
}

impl Encode for ExVoteSystemInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.recom_left)
            .i32(self.recom_have)
            .i32(self.bonus_time)
            .i32(self.bonus_value)
            .i32(self.bonus_type);
    }
}

impl Decode for ExVoteSystemInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            recom_left: r.i32()?,
            recom_have: r.i32()?,
            bonus_time: r.i32()?,
            bonus_value: r.i32()?,
            bonus_type: r.i32()?,
        })
    }
}

/// One teleport bookmark of [`ExGetBookMarkInfoPacket`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TeleportBookmark {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub name: String,
    pub icon: i32,
    pub tag: String,
}

// Java: gameserver/network/serverpackets/ExGetBookMarkInfoPacket.java
/// The teleport bookmark list (`EX_GET_BOOKMARK_INFO`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExGetBookMarkInfoPacket {
    /// Java `getBookmarkSlot()`: how many bookmarks the character may have.
    pub slot: i32,
    pub bookmarks: Vec<TeleportBookmark>,
}

impl ExGetBookMarkInfoPacket {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExGetBookmarkInfo;
}

impl Encode for ExGetBookMarkInfoPacket {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(0); // Java: "Dummy"
        w.i32(self.slot).i32(self.bookmarks.len() as i32);
        for bookmark in &self.bookmarks {
            w.i32(bookmark.id)
                .i32(bookmark.x)
                .i32(bookmark.y)
                .i32(bookmark.z)
                .string(&bookmark.name)
                .i32(bookmark.icon)
                .string(&bookmark.tag);
        }
    }
}

impl Decode for ExGetBookMarkInfoPacket {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?; // dummy
        let slot = r.i32()?;
        let count = read_count(r, "bookmark count")?;
        let mut bookmarks = Vec::new();
        for _ in 0..count {
            bookmarks.push(TeleportBookmark {
                id: r.i32()?,
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
                name: r.string(),
                icon: r.i32()?,
                tag: r.string(),
            });
        }
        Ok(Self { slot, bookmarks })
    }
}

// Java: gameserver/network/serverpackets/ValidateLocation.java
/// Tells the client where the server thinks the character stands (`VALIDATE_LOCATION`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ValidateLocation {
    pub object_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl ValidateLocation {
    pub const OPCODE: ServerOpcode = ServerOpcode::ValidateLocation;
}

impl Encode for ValidateLocation {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(self.heading);
    }
}

impl Decode for ValidateLocation {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            z: r.i32()?,
            heading: r.i32()?,
        })
    }
}

/// One skill of [`SkillCoolTime`]: the times are already in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillCoolTimeEntry {
    pub skill_id: i32,
    pub level: i32,
    /// See [`reuse_seconds`].
    pub reuse_seconds: i32,
    /// See [`remaining_seconds`].
    pub remaining_seconds: i32,
}

/// The "reuse" seconds Java writes for a skill (`SkillCoolTime.writeImpl`): the full reuse delay
/// when it is set, else the time left. Java casts the long to `int` before dividing by 1000.
pub fn reuse_seconds(reuse_ms: i64, remaining_ms: i64) -> i32 {
    let ms = if reuse_ms > 0 { reuse_ms } else { remaining_ms };
    (ms as i32) / 1000
}

/// The "remaining" seconds Java writes for a skill: at least 1, so the client keeps showing the
/// cool down until it really ended.
pub fn remaining_seconds(remaining_ms: i64) -> i32 {
    ((remaining_ms as i32) / 1000).max(1)
}

// Java: gameserver/network/serverpackets/SkillCoolTime.java
/// The skills still in their reuse delay (`SKILL_COOL_TIME`). The caller leaves out the ones
/// that already passed (Java `hasNotPassed()`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillCoolTime {
    pub entries: Vec<SkillCoolTimeEntry>,
}

impl SkillCoolTime {
    pub const OPCODE: ServerOpcode = ServerOpcode::SkillCoolTime;
}

impl Encode for SkillCoolTime {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.entries.len() as i32);
        for entry in &self.entries {
            w.i32(entry.skill_id)
                .i32(entry.level)
                .i32(entry.reuse_seconds)
                .i32(entry.remaining_seconds);
        }
    }
}

impl Decode for SkillCoolTime {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "skill cool time count")?;
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(SkillCoolTimeEntry {
                skill_id: r.i32()?,
                level: r.i32()?,
                reuse_seconds: r.i32()?,
                remaining_seconds: r.i32()?,
            });
        }
        Ok(Self { entries })
    }
}

// Java: gameserver/network/serverpackets/ExUISetting.java
/// The stored UI key mapping (`EX_UI_SETTING`). Java reads it from a player variable; empty
/// bytes mean "nothing stored" (Java writes a length of 0 then).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExUiSetting {
    pub ui_key_mapping: Vec<u8>,
}

impl ExUiSetting {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExUiSetting;
}

impl Encode for ExUiSetting {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.ui_key_mapping.len() as i32);
        w.bytes(&self.ui_key_mapping);
    }
}

impl Decode for ExUiSetting {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let len = read_count(r, "ui key mapping length")?;
        Ok(Self {
            ui_key_mapping: r.bytes(len)?.to_vec(),
        })
    }
}

/// Screen position: top left.
pub const TOP_LEFT: i32 = 1;
/// Screen position: top centre.
pub const TOP_CENTER: i32 = 2;
/// Screen position: top right.
pub const TOP_RIGHT: i32 = 3;
/// Screen position: middle left.
pub const MIDDLE_LEFT: i32 = 4;
/// Screen position: middle centre.
pub const MIDDLE_CENTER: i32 = 5;
/// Screen position: middle right.
pub const MIDDLE_RIGHT: i32 = 6;
/// Screen position: bottom centre.
pub const BOTTOM_CENTER: i32 = 7;
/// Screen position: bottom right.
pub const BOTTOM_RIGHT: i32 = 8;

// Java: gameserver/network/serverpackets/ExShowScreenMessage.java
/// A message in the middle of the screen (`EX_SHOW_SCREEN_MESSAGE`).
///
/// The constructors cover the Java constructors EnterWorld uses. `npc_string_id` `-1` and
/// `sys_message_id` `-1` mean "none".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowScreenMessage {
    /// Java `_type`: 1 for a styled text, 2 for a system message or NPC string.
    pub kind: i32,
    /// [`SystemMessageId`] of the message, or -1.
    pub sys_message_id: i32,
    /// One of the `TOP_*` / `MIDDLE_*` / `BOTTOM_*` constants.
    pub position: i32,
    pub unknown1: i32,
    /// Font size: 0 normal, 1 small.
    pub size: i32,
    pub unknown2: i32,
    pub unknown3: i32,
    pub show_effect: bool,
    /// Display time in milliseconds.
    pub time: i32,
    pub fade: bool,
    /// [`NpcStringId`] of the message, or -1.
    pub npc_string_id: i32,
    /// Written when there is no NPC string. An empty string is what Java writes for `null`.
    pub text: String,
    /// Written instead of `text` when there is an NPC string.
    pub params: Vec<String>,
}

impl ExShowScreenMessage {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowScreenMessage;

    /// The defaults every constructor starts from.
    fn plain(kind: i32, position: i32, time: i32) -> Self {
        Self {
            kind,
            sys_message_id: -1,
            position,
            unknown1: 0,
            size: 0,
            unknown2: 0,
            unknown3: 0,
            show_effect: false,
            time,
            fade: false,
            npc_string_id: -1,
            text: String::new(),
            params: Vec::new(),
        }
    }

    /// Java `ExShowScreenMessage(String text, int time)`: type 2, top centre.
    pub fn text(text: &str, time: i32) -> Self {
        Self {
            text: text.to_string(),
            ..Self::plain(2, TOP_CENTER, time)
        }
    }

    /// Java `ExShowScreenMessage(String text, int position, int time, int size, boolean fade,
    /// boolean showEffect)`: type 1.
    pub fn styled_text(
        text: &str,
        position: i32,
        time: i32,
        size: i32,
        fade: bool,
        show_effect: bool,
    ) -> Self {
        Self {
            size,
            fade,
            show_effect,
            text: text.to_string(),
            ..Self::plain(1, position, time)
        }
    }

    /// Java `ExShowScreenMessage(NpcStringId, int position, int time, String... params)`.
    pub fn npc_string(npc: NpcStringId, position: i32, time: i32, params: &[&str]) -> Self {
        Self {
            npc_string_id: npc.id(),
            params: params.iter().map(|p| (*p).to_string()).collect(),
            ..Self::plain(2, position, time)
        }
    }

    /// Java `ExShowScreenMessage(SystemMessageId, int position, int time, String... params)`.
    ///
    /// NOTE(java-quirk): Java keeps the parameters but its `writeImpl` only writes them when
    /// there is an NPC string, so they never reach the client. They are kept here as well.
    pub fn system_message(message: SystemMessageId, position: i32, time: i32, params: &[&str]) -> Self {
        Self {
            sys_message_id: message.id(),
            params: params.iter().map(|p| (*p).to_string()).collect(),
            ..Self::plain(2, position, time)
        }
    }
}

impl Encode for ExShowScreenMessage {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.kind)
            .i32(self.sys_message_id)
            .i32(self.position)
            .i32(self.unknown1)
            .i32(self.size)
            .i32(self.unknown2)
            .i32(self.unknown3)
            .i32(i32::from(self.show_effect))
            .i32(self.time)
            .i32(i32::from(self.fade))
            .i32(self.npc_string_id);
        if self.npc_string_id == -1 {
            w.string(&self.text);
        } else {
            for param in &self.params {
                w.string(param);
            }
        }
    }
}

impl Decode for ExShowScreenMessage {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let kind = r.i32()?;
        let sys_message_id = r.i32()?;
        let position = r.i32()?;
        let unknown1 = r.i32()?;
        let size = r.i32()?;
        let unknown2 = r.i32()?;
        let unknown3 = r.i32()?;
        let show_effect = r.i32()? != 0;
        let time = r.i32()?;
        let fade = r.i32()? != 0;
        let npc_string_id = r.i32()?;
        let (text, params) = if npc_string_id == -1 {
            (r.string(), Vec::new())
        } else {
            // The parameters run to the end of the packet; a lone trailing byte cannot hold a
            // string terminator, so it is left alone.
            let mut params = Vec::new();
            while r.remaining() >= 2 {
                params.push(r.string());
            }
            (String::new(), params)
        };
        Ok(Self {
            kind,
            sys_message_id,
            position,
            unknown1,
            size,
            unknown2,
            unknown3,
            show_effect,
            time,
            fade,
            npc_string_id,
            text,
            params,
        })
    }
}
