//! The shortcut bar: `RequestShortcutReg`, `RequestShortcutDel`, and the deletions other code
//! causes (a destroyed item, a removed skill).
// Java: network/clientpackets/{RequestShortcutReg,RequestShortcutDel}.java
// Java: network/serverpackets/{ShortcutRegister,ShortcutInit}.java
// Java: entity/actor/holders/player/{Shortcuts,Shortcut}.java
// Java: entity/actor/Player.java (registerShortcut, deleteShortcut, removeSkill)
//
// # Where a shortcut lives
// `PlayerData.shortcuts` (one `ShortcutData` per `character_shortcuts` row of the active class).
// Java keeps them in a map keyed by `slot + page * 12`; here a key is "the shortcut whose
// `slot + page * 12` is equal", so the same collisions happen (a slot of -5 on page 1 and slot 7 on
// page 0 are one place).
//
// # Database
// Java writes at once: `registerShortcutInDb` is `REPLACE INTO character_shortcuts` (after a
// `DELETE` of the shortcut it replaces) and `deleteShortcutFromDb` is a `DELETE`, both with the
// character's `class_index`. Here the rows follow with the next store, which compares the
// shortcut list with what it stored last (`persist.rs`, the same path as items and skills). The
// result in the table is the same rows.
//
// # What is not Java's
// - There is no `ShortCutDelete` server packet. Opcode 0x46 has no class in this Java version:
//   `deleteShortcut` resends the whole `ShortcutInit` (and the automatic shots) instead.
// - The shared reuse group is not kept per shortcut (see [`World::register_shortcut`]).
// - `Player.removeRecipeBook`'s `deleteShortcut` is not ported: there are no recipe books yet.

use l2_data::item::types::EtcItemType;
use l2_protocol_game::server::ShortcutRegister;
use l2_protocol_game::server::enterworld::OtherShortcut;
use l2_util::ObjectId;

use crate::enter::shortcut_init;
use crate::inventory::etc_type;
use crate::player::ShortcutData;
use crate::world::World;

/// Java `MAX_SHORTCUTS_PER_BAR`: the wire slot is `slot + page * 12`.
pub const SHORTCUTS_PER_PAGE: i32 = 12;
/// Java `ShortcutType` ordinals (NONE, ITEM, SKILL, ACTION, MACRO, RECIPE, BOOKMARK).
pub const TYPE_NONE: i32 = 0;
pub const TYPE_ITEM: i32 = 1;
pub const TYPE_SKILL: i32 = 2;

/// The `ShortcutRegister` packet of a shortcut. `group` is the shared reuse group an item
/// shortcut shows (`Shortcut.getSharedReuseGroup`, -1 until set).
///
/// A type outside the ordinals Java knows cannot come from a client: `RequestShortcutReg` maps
/// those to NONE. A stored row with such a type is written as NONE here.
pub fn shortcut_register_packet(sc: &ShortcutData, character_type: i32, group: i32) -> ShortcutRegister {
    let slot = sc.slot + sc.page * SHORTCUTS_PER_PAGE;
    let id = sc.shortcut_id;
    let other = |kind| ShortcutRegister::Other {
        kind,
        slot,
        id,
        character_type,
    };
    match sc.kind {
        TYPE_ITEM => ShortcutRegister::Item {
            slot,
            id,
            character_type,
            shared_reuse_group: group,
        },
        TYPE_SKILL => ShortcutRegister::Skill {
            slot,
            id,
            level: sc.level,
            character_type,
        },
        3 => other(OtherShortcut::Action),
        4 => other(OtherShortcut::Macro),
        5 => other(OtherShortcut::Recipe),
        6 => other(OtherShortcut::Bookmark),
        _ => ShortcutRegister::Empty { slot },
    }
}

impl World {
    /// Java `RequestShortcutReg.runImpl`: the shortcut goes on the bar and the client is told.
    pub(crate) fn on_request_shortcut_reg(
        &mut self,
        id: ObjectId,
        type_id: i32,
        wire_slot: i32,
        shortcut_id: i32,
        level: i32,
        character_type: i32,
    ) {
        if !self.players.contains_key(&id) {
            return;
        }
        // `ShortcutType.values()[(typeId < 1) || (typeId > 6) ? 0 : typeId]`
        let kind = if (1..=6).contains(&type_id) {
            type_id
        } else {
            TYPE_NONE
        };
        // Java's `%` and `/` truncate toward zero, and so do Rust's: -5 is slot -5 on page 0.
        let slot = wire_slot % SHORTCUTS_PER_PAGE;
        let page = wire_slot / SHORTCUTS_PER_PAGE;
        if !(0..=10).contains(&page) {
            return;
        }
        let shortcut = ShortcutData {
            slot,
            page,
            kind,
            shortcut_id,
            level,
        };
        let group = self.register_shortcut(id, shortcut).unwrap_or(-1);
        // NOTE(java-quirk): the packet goes out even when `registerShortcut` refused an item that
        // is not in the inventory; the client then shows an item shortcut with group -1.
        self.send(id, &shortcut_register_packet(&shortcut, character_type, group));
    }

    /// Java `RequestShortcutDel.runImpl`: the shortcut leaves the bar. Java sends nothing of its
    /// own ("client needs no confirmation"); `deleteShortcut` resends the list.
    pub(crate) fn on_request_shortcut_del(&mut self, id: ObjectId, wire_slot: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        let slot = wire_slot % SHORTCUTS_PER_PAGE;
        let page = wire_slot / SHORTCUTS_PER_PAGE;
        if !(0..=10).contains(&page) {
            return;
        }
        self.delete_shortcut(id, slot, page);
    }

    /// Java `Shortcuts.registerShortcut`. Returns the shared reuse group an item shortcut got, or
    /// `None` when nothing was registered (an item that is not in the inventory) or the shortcut
    /// is not an item.
    ///
    /// The group is only used for the packet: Java also keeps it in the `Shortcut`, and a later
    /// `ShortcutInit` writes it, but `ShortcutData` has no field for it, so the list recomputes it
    /// the way `restoreMe` does (the template's group for etc items, -1 for the rest).
    // NOTE(java-quirk): `Shortcuts.registerShortcut` takes the group of any item from its template
    // (0 for most weapons and armor), `restoreMe` only of etc items (-1 for the rest). The client
    // does not seem to care.
    pub(crate) fn register_shortcut(&mut self, id: ObjectId, shortcut: ShortcutData) -> Option<i32> {
        let group = if shortcut.kind == TYPE_ITEM {
            let object_id = ObjectId(shortcut.shortcut_id);
            // Verify shortcut: the item has to be in the inventory (or worn).
            let item_id = self.inventory_item(id, object_id)?.item_id;
            let from_template = self
                .item_templates()
                .as_deref()
                .and_then(|t| t.get(item_id))
                .map(|t| t.shared_reuse_group);
            Some(
                from_template.unwrap_or_else(|| self.data.item(item_id).map_or(-1, |i| i.shared_reuse_group)),
            )
        } else {
            None
        };
        let player = self.players.get_mut(&id)?;
        let key = shortcut.slot + shortcut.page * SHORTCUTS_PER_PAGE;
        // `_shortcuts.put(key, shortcut)` keeps the place of a shortcut it replaces.
        match player
            .data
            .shortcuts
            .iter_mut()
            .find(|s| s.slot + s.page * SHORTCUTS_PER_PAGE == key)
        {
            Some(old) => *old = shortcut,
            None => player.data.shortcuts.push(shortcut),
        }
        group
    }

    /// Java `Shortcuts.deleteShortcut(slot, page)`: the shortcut goes; one to a shot item also ends
    /// the automatic use of the shot; the client gets the whole list again and then the automatic
    /// shots. Nothing happens for an empty place.
    pub(crate) fn delete_shortcut(&mut self, id: ObjectId, slot: i32, page: i32) {
        let key = slot + page * SHORTCUTS_PER_PAGE;
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let Some(index) = player
            .data
            .shortcuts
            .iter()
            .position(|s| s.slot + s.page * SHORTCUTS_PER_PAGE == key)
        else {
            return;
        };
        let old = player.data.shortcuts.remove(index);
        if old.kind == TYPE_ITEM {
            let item = player
                .data
                .items
                .iter()
                .find(|i| i.object_id.0 == old.shortcut_id)
                .map(|i| i.item_id);
            if let Some(item_id) = item {
                let is_shot = self
                    .item_templates()
                    .as_deref()
                    .and_then(|t| t.get(item_id))
                    .is_some_and(|t| etc_type(t) == Some(EtcItemType::Shot));
                self.shortcut_removed_for_item(id, item_id, is_shot);
            }
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let list = shortcut_init(&player.data, &*self.data);
        self.send(id, &list);
        // The automatic shots are announced again after the list.
        self.resend_auto_soul_shots(id);
    }

    /// The end of Java `Player.removeSkill(skill)`: every skill shortcut of the skill goes, one
    /// `deleteShortcut` (and one list) each. The skills 3080 to 3259 keep theirs.
    // Java returns before this when `getTransformationId() > 0` or a cursed weapon is equipped
    // (cursed weapons are M8).
    pub fn delete_skill_shortcuts(&mut self, id: ObjectId, skill_id: i32) {
        if self.transformation_id(id) > 0 {
            return;
        }
        if (3080..=3259).contains(&skill_id) {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let places: Vec<(i32, i32)> = player
            .data
            .shortcuts
            .iter()
            .filter(|s| s.kind == TYPE_SKILL && s.shortcut_id == skill_id)
            .map(|s| (s.slot, s.page))
            .collect();
        for (slot, page) in places {
            self.delete_shortcut(id, slot, page);
        }
    }

    /// Java `Shortcuts.deleteShortcutByObjectId`: the first shortcut that points to the item goes.
    pub(crate) fn delete_shortcut_by_object_id(&mut self, id: ObjectId, object_id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let place = player
            .data
            .shortcuts
            .iter()
            .find(|s| s.kind == TYPE_ITEM && s.shortcut_id == object_id.0)
            .map(|s| (s.slot, s.page));
        if let Some((slot, page)) = place {
            self.delete_shortcut(id, slot, page);
        }
    }
}
