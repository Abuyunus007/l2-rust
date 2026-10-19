//! Server packets of clicking and talking: target selection, action failure, NPC dialog html,
//! and the door and static object info the world sends.
//!
//! Same rules as [`super::enterworld`]: these carry values only. The caller looks up the objects
//! and passes the numbers in. Html is final: the caller does the replacements (`%objectId%`, ...)
//! before it builds an [`NpcHtmlMessage`].

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/MyTargetSelected.java
/// Sent to the player who selected a target: the target id and the colour of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyTargetSelected {
    pub object_id: i32,
    /// Player level minus target level for an auto-attackable target, else 0.
    pub color: i16,
}

impl MyTargetSelected {
    pub const OPCODE: ServerOpcode = ServerOpcode::MyTargetSelected;

    /// Java constructor: the colour is the level difference only for auto-attackable targets.
    ///
    /// NOTE(java-quirk): Java passes the int difference to `writeShort(int)`, which keeps the low
    /// 16 bits. `as i16` does the same, so a huge difference wraps like Java does.
    pub fn new(object_id: i32, player_level: i32, target_level: i32, auto_attackable: bool) -> Self {
        let color = if auto_attackable {
            (player_level - target_level) as i16
        } else {
            0
        };
        Self { object_id, color }
    }
}

impl Encode for MyTargetSelected {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: object id, colour, then a constant int 0.
        w.i32(self.object_id).i16(self.color).i32(0);
    }
}

impl Decode for MyTargetSelected {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let color = r.i16()?;
        r.skip(4)?; // constant 0
        Ok(Self { object_id, color })
    }
}

// Java: gameserver/network/serverpackets/TargetSelected.java
/// Tells the nearby players that `object_id` selected `target_id` at the given position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetSelected {
    pub object_id: i32,
    pub target_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl TargetSelected {
    pub const OPCODE: ServerOpcode = ServerOpcode::TargetSelected;
}

impl Encode for TargetSelected {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: the trailing int is written as a constant 0 (`// ?` in the source).
        w.i32(self.object_id)
            .i32(self.target_id)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z)
            .i32(0);
    }
}

impl Decode for TargetSelected {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let target_id = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        r.skip(4)?; // constant 0
        Ok(Self {
            object_id,
            target_id,
            x,
            y,
            z,
        })
    }
}

// Java: gameserver/network/serverpackets/TargetUnselected.java
/// Tells the nearby players that a target was dropped. Position is the target's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetUnselected {
    pub target_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl TargetUnselected {
    pub const OPCODE: ServerOpcode = ServerOpcode::TargetUnselected;
}

impl Encode for TargetUnselected {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: the trailing int is written as a constant 0 (`// ?` in the source).
        w.i32(self.target_id).i32(self.x).i32(self.y).i32(self.z).i32(0);
    }
}

impl Decode for TargetUnselected {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let target_id = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        r.skip(4)?; // constant 0
        Ok(Self { target_id, x, y, z })
    }
}

// Java: gameserver/network/serverpackets/ActionFailed.java
/// "The action did not happen": sent for every rejected click or bypass. No fields.
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

// Java: gameserver/network/serverpackets/AbstractHtmlPacket.java (setHtml)
/// Java `setHtml` limit, counted in UTF-16 units (Java `String.length()`).
const MAX_HTML_UNITS: usize = 17_200;

// Java: gameserver/network/serverpackets/NpcHtmlMessage.java
/// A dialog window with html. `html` is what goes on the wire; use [`NpcHtmlMessage::new`] or
/// [`NpcHtmlMessage::new_item`] to get Java's `setHtml` rules applied to the raw page.
///
/// `item_id` is 0 for an NPC dialog. Java uses a non-zero item id for the item html window (its
/// html action scope), so the world keeps the two apart when it caches the html links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcHtmlMessage {
    pub npc_obj_id: i32,
    pub html: String,
    pub item_id: i32,
}

impl NpcHtmlMessage {
    pub const OPCODE: ServerOpcode = ServerOpcode::NpcHtmlMessage;

    /// Java `NpcHtmlMessage(int npcObjId, String html)`. Returns `None` where Java throws
    /// `IllegalArgumentException` (negative object id).
    pub fn new(npc_obj_id: i32, html: &str) -> Option<Self> {
        Self::new_item(npc_obj_id, 0, html)
    }

    /// Java `NpcHtmlMessage(int npcObjId, int itemId, String html)`. Returns `None` where Java
    /// throws (negative object id or item id).
    pub fn new_item(npc_obj_id: i32, item_id: i32, html: &str) -> Option<Self> {
        if npc_obj_id < 0 || item_id < 0 {
            return None;
        }
        Some(Self {
            npc_obj_id,
            html: prepare_html(html),
            item_id,
        })
    }
}

/// Java `AbstractHtmlPacket.setHtml`: cut a page that is too long, wrap a bare fragment in
/// `<html><body>`, keep a full page or a `..\L2` file reference as it is.
///
/// NOTE(java-quirk): Java only warns about a page that is too long and cuts it without wrapping
/// it. Cutting may split a surrogate pair; Java then sends the lone half, which we cannot hold in
/// a `String`, so it becomes U+FFFD.
fn prepare_html(html: &str) -> String {
    if html.encode_utf16().count() > MAX_HTML_UNITS {
        let units: Vec<u16> = html.encode_utf16().take(MAX_HTML_UNITS).collect();
        return String::from_utf16_lossy(&units);
    }
    // NOTE(java-quirk): the check is case sensitive, `<HTML>` gets wrapped again.
    if !html.contains("<html") && !html.starts_with("..\\L2") {
        format!("<html><body>{html}</body></html>")
    } else {
        html.to_owned()
    }
}

impl Encode for NpcHtmlMessage {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.npc_obj_id).string(&self.html).i32(self.item_id);
    }
}

impl Decode for NpcHtmlMessage {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let npc_obj_id = r.i32()?;
        let html = r.string();
        let item_id = r.i32()?;
        Ok(Self {
            npc_obj_id,
            html,
            item_id,
        })
    }
}

// Java: gameserver/network/serverpackets/SocialAction.java
/// An object plays an animation (`SOCIAL_ACTION`): the greeting of a talked-to NPC, a player's
/// social emote. Java marks it droppable under load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocialAction {
    pub object_id: i32,
    pub action_id: i32,
}

impl SocialAction {
    pub const OPCODE: ServerOpcode = ServerOpcode::SocialAction;
}

impl Encode for SocialAction {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(self.action_id);
    }
}

impl Decode for SocialAction {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            action_id: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ShowTownMap.java
/// Opens the town map window (`SHOW_TOWN_MAP`); a clicked map sign sends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowTownMap {
    /// Java `"town_map." + texture`.
    pub texture: String,
    pub x: i32,
    pub y: i32,
}

impl ShowTownMap {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShowTownMap;
}

impl Encode for ShowTownMap {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.texture).i32(self.x).i32(self.y);
    }
}

impl Decode for ShowTownMap {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let texture = r.string();
        Ok(Self {
            texture,
            x: r.i32()?,
            y: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/DoorInfo.java
/// Tells a player that a door exists (sent when the door enters the player's view).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoorInfo {
    pub object_id: i32,
    /// The door template id (`Door.getId()`).
    pub door_id: i32,
}

impl DoorInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::DoorInfo;
}

impl Encode for DoorInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(self.door_id);
    }
}

impl Decode for DoorInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let door_id = r.i32()?;
        Ok(Self { object_id, door_id })
    }
}

// Java: gameserver/network/serverpackets/DoorStatusUpdate.java
/// A door's state and hp, sent after it opens, closes or takes damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoorStatusUpdate {
    pub object_id: i32,
    /// Java writes `!door.isOpen()`, so `true` means closed.
    pub closed: bool,
    pub damage: i32,
    pub is_enemy: bool,
    pub door_id: i32,
    /// Java casts the double hp to int: the caller passes the cast value.
    pub current_hp: i32,
    pub max_hp: i32,
}

impl DoorStatusUpdate {
    pub const OPCODE: ServerOpcode = ServerOpcode::DoorStatusUpdate;
}

impl Encode for DoorStatusUpdate {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(i32::from(self.closed))
            .i32(self.damage)
            .i32(i32::from(self.is_enemy))
            .i32(self.door_id)
            .i32(self.current_hp)
            .i32(self.max_hp);
    }
}

impl Decode for DoorStatusUpdate {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let closed = r.i32()? != 0;
        let damage = r.i32()?;
        let is_enemy = r.i32()? != 0;
        let door_id = r.i32()?;
        let current_hp = r.i32()?;
        let max_hp = r.i32()?;
        Ok(Self {
            object_id,
            closed,
            damage,
            is_enemy,
            door_id,
            current_hp,
            max_hp,
        })
    }
}

// Java: gameserver/network/serverpackets/StaticObjectInfo.java
/// Info about a static object (a signboard, a banner...) or a door, in the client's
/// "static object" form. Java sends doors with this packet too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticObjectInfo {
    /// The static object or door template id.
    pub static_object_id: i32,
    pub object_id: i32,
    /// [`Self::KIND_STATIC_OBJECT`] or [`Self::KIND_DOOR`] (Java `_type`).
    pub kind: i32,
    pub targetable: bool,
    pub mesh_index: i32,
    pub closed: bool,
    pub is_enemy: bool,
    pub current_hp: i32,
    pub max_hp: i32,
    pub show_hp: bool,
    pub damage_grade: i32,
}

impl StaticObjectInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::StaticObject;
    pub const KIND_STATIC_OBJECT: i32 = 0;
    pub const KIND_DOOR: i32 = 1;

    /// Java constructor `StaticObjectInfo(StaticObject)`: targetable, open, no hp, no hp bar.
    pub fn static_object(static_object_id: i32, object_id: i32, mesh_index: i32) -> Self {
        Self {
            static_object_id,
            object_id,
            kind: Self::KIND_STATIC_OBJECT,
            targetable: true,
            mesh_index,
            closed: false,
            is_enemy: false,
            current_hp: 0,
            max_hp: 0,
            show_hp: false,
            damage_grade: 0,
        }
    }
}

impl Encode for StaticObjectInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.static_object_id)
            .i32(self.object_id)
            .i32(self.kind)
            .i32(i32::from(self.targetable))
            .i32(self.mesh_index)
            .i32(i32::from(self.closed))
            .i32(i32::from(self.is_enemy))
            // Java writes current hp before max hp here.
            .i32(self.current_hp)
            .i32(self.max_hp)
            .i32(i32::from(self.show_hp))
            .i32(self.damage_grade);
    }
}

impl Decode for StaticObjectInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let static_object_id = r.i32()?;
        let object_id = r.i32()?;
        let kind = r.i32()?;
        let targetable = r.i32()? != 0;
        let mesh_index = r.i32()?;
        let closed = r.i32()? != 0;
        let is_enemy = r.i32()? != 0;
        let current_hp = r.i32()?;
        let max_hp = r.i32()?;
        let show_hp = r.i32()? != 0;
        let damage_grade = r.i32()?;
        Ok(Self {
            static_object_id,
            object_id,
            kind,
            targetable,
            mesh_index,
            closed,
            is_enemy,
            current_hp,
            max_hp,
            show_hp,
            damage_grade,
        })
    }
}
