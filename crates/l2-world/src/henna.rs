//! Hennas (dyes) and the symbol maker: the three slots, drawing and removing a dye, the lists and
//! previews the client asks for, and the `SymbolMaker` NPC script (L-01).
// Java: network/clientpackets/{RequestHennaItemList,RequestHennaItemInfo,RequestHennaEquip,
// Java: RequestHennaRemoveList,RequestHennaItemRemoveInfo,RequestHennaRemove}.java
// Java: network/serverpackets/{HennaEquipList,HennaRemoveList,HennaItemDrawInfo,
// Java: HennaItemRemoveInfo,HennaInfo}.java
// Java: entity/actor/Player.java (getHennaEmptySlots, addHenna, removeHenna, setPlayerClass)
// Java: dist/game/data/scripts/ai/others/SymbolMaker/SymbolMaker.java (and its pages)
//
// # Where a dye lives
// `PlayerProfile::hennas`: the dye id of slots 1 to 3 of the active class index, 0 for an empty
// slot (loaded at enter world and again by the subclass switch, Java `restoreHenna`). The bonuses are not stored: the stat engine
// adds them from the slots (`FuncHenna`, `StatView::henna_stats`, the cap of 5 per stat), so a
// change of a slot is a change of the stats.
//
// # Database
// Java writes at once: `addHenna` is an `INSERT INTO character_hennas`, `removeHenna` a `DELETE`,
// both with the character's `class_index`. They go out as `PersistCommand::Henna` (D-15). A full
// character store does not touch the table, like Java.
//
// # The script
// `SymbolMaker` is a script of the engine (S-API-02, `l2-scripts/src/ai/others/symbol_maker.rs`);
// it opens the lists with [`World::send_henna_equip_list`] and [`World::send_henna_remove_list`].
//
// # Left out
// - `OnPlayerHennaAdd` / `OnPlayerHennaRemove` for scripts (no script listens yet).

use l2_data::{ClassId, Henna};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::henna::{
    HennaEquipEntry, HennaEquipList, HennaItemInfo, HennaItemRemoveInfo, HennaPreview, HennaRemoveEntry,
    HennaRemoveList, HennaStatPreview,
};
use l2_protocol_game::server::{ActionFailed, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::flood::FloodKind;
use crate::world::World;

/// A client request of the symbol maker windows, with the raw wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HennaInput {
    /// Java `RequestHennaItemList`.
    ItemList,
    /// Java `RequestHennaItemInfo`.
    ItemInfo { symbol_id: i32 },
    /// Java `RequestHennaEquip`.
    Equip { symbol_id: i32 },
    /// Java `RequestHennaRemoveList`.
    RemoveList,
    /// Java `RequestHennaItemRemoveInfo`.
    ItemRemoveInfo { symbol_id: i32 },
    /// Java `RequestHennaRemove`.
    Remove { symbol_id: i32 },
}

/// A `character_hennas` row change with Java's statement (`Player.ADD_CHAR_HENNA`,
/// `DELETE_CHAR_HENNA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HennaWrite {
    /// `INSERT INTO character_hennas (charId,symbol_id,slot,class_index) VALUES (?,?,?,?)`.
    Add {
        char_id: i32,
        symbol_id: i32,
        slot: i32,
        class_index: i32,
    },
    /// `DELETE FROM character_hennas WHERE charId=? AND slot=? AND class_index=?`.
    Delete {
        char_id: i32,
        slot: i32,
        class_index: i32,
    },
}

/// `(byte) value`, what Java's `writeByte(int)` puts on the wire.
fn low_byte(value: i32) -> i8 {
    value as i8
}

impl World {
    // ------------------------------------------------------------------ the slots

    /// `HennaData.getHenna(id)`, cloned so the caller can use the world again.
    fn henna_data(&self, dye_id: i32) -> Option<Henna> {
        if dye_id == 0 {
            return None;
        }
        self.data.stat_data()?.hennas.get(dye_id).cloned()
    }

    /// Java `Player.getHenna(slot)` (1 to 3): a slot whose dye the datapack does not know is empty,
    /// like Java's `_henna[slot - 1] = HennaData.getHenna(symbolId)` gives null for it.
    fn henna_in_slot(&self, id: ObjectId, slot: usize) -> Option<Henna> {
        let dye_id = *self
            .players
            .get(&id)?
            .data
            .profile
            .hennas
            .get(slot.checked_sub(1)?)?;
        self.henna_data(dye_id)
    }

    fn class_of(&self, id: ObjectId) -> ClassId {
        let class_id = self.players.get(&id).map_or(0, |p| p.data.class_id);
        ClassId(u16::try_from(class_id).unwrap_or(0))
    }

    /// Java `Player.getHennaEmptySlots()`: 0 slots for a class of level 0, 2 for level 1, 3 above,
    /// less the dyes worn.
    pub(crate) fn henna_empty_slots(&self, id: ObjectId) -> i32 {
        let mut total = match self.class_of(id).class_level() {
            1 => 2,
            level if level > 1 => 3,
            _ => 0,
        };
        for slot in 1..=3 {
            if self.henna_in_slot(id, slot).is_some() {
                total -= 1;
            }
        }
        total.max(0)
    }

    /// Java `Player.addHenna(henna)`: the dye goes into the first empty slot, the row is written,
    /// the client gets `HennaInfo` and `UserInfo`. `false` when no slot is free.
    pub(crate) fn add_henna(&mut self, id: ObjectId, henna: &Henna) -> bool {
        let free = (1..=3).find(|slot| self.henna_in_slot(id, *slot).is_none());
        let (Some(slot), Some(player)) = (free, self.players.get_mut(&id)) else {
            return false;
        };
        player.data.profile.hennas[slot - 1] = henna.dye_id;
        let class_index = player.data.profile.class_index;
        self.outbox
            .persist(crate::outbox::PersistCommand::Henna(HennaWrite::Add {
                char_id: id.0,
                symbol_id: henna.dye_id,
                slot: slot as i32,
                class_index,
            }));
        let info = self.henna_info(id);
        self.send(id, &info);
        self.send_user_info(id);
        true
    }

    /// Java `Player.removeHenna(slot)`: the slot is emptied, the row deleted, the client told, the
    /// dyes given back and the fee taken. `false` for a slot out of 1 to 3 or an empty one.
    pub(crate) fn remove_henna(&mut self, id: ObjectId, slot: usize) -> bool {
        let Some(henna) = self.henna_in_slot(id, slot) else {
            return false;
        };
        let Some(player) = self.players.get_mut(&id) else {
            return false;
        };
        player.data.profile.hennas[slot - 1] = 0;
        let class_index = player.data.profile.class_index;
        self.outbox
            .persist(crate::outbox::PersistCommand::Henna(HennaWrite::Delete {
                char_id: id.0,
                slot: slot as i32,
                class_index,
            }));
        let info = self.henna_info(id);
        self.send(id, &info);
        self.send_user_info(id);

        // "Add the recovered dyes to the player's inventory and notify them."
        // `Inventory.addItem` of the inventory itself: no message of its own, no capacity check.
        if henna.cancel_count > 0 {
            let _ = self.inventory_add_item_by_id(id, henna.dye_item_id, i64::from(henna.cancel_count));
        }
        self.reduce_adena(id, i64::from(henna.cancel_fee), false);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                .with(SystemMessageParam::ItemName(henna.dye_item_id))
                .with(SystemMessageParam::LongNumber(i64::from(henna.cancel_count))),
        );
        self.send(id, &SystemMessage::new(SystemMessageId::TheSymbolHasBeenDeleted));
        true
    }

    /// The "Remove class permitted hennas" loop of `Player.setPlayerClass`: every dye the new class
    /// may not wear is removed, with its refund.
    pub(crate) fn remove_henna_not_allowed(&mut self, id: ObjectId) {
        let class = self.class_of(id);
        for slot in 1..=3 {
            if let Some(henna) = self.henna_in_slot(id, slot)
                && !henna.is_allowed_class(class)
            {
                self.remove_henna(id, slot);
            }
        }
    }

    // ------------------------------------------------------------------ the packets

    /// Java `new HennaEquipList(player)`.
    fn henna_equip_list(&self, id: ObjectId) -> Option<HennaEquipList> {
        let class = self.class_of(id);
        let hennas = self.data.stat_data()?.hennas.clone();
        let list = hennas.henna_list(class);
        // "Player must have at least one dye in inventory to be able to see the Henna that can be
        // applied with it."
        let entries = list
            .iter()
            .filter(|h| self.inventory_item_by_item_id(id, h.dye_item_id).is_some())
            .map(|h| HennaEquipEntry {
                dye_id: h.dye_id,
                dye_item_id: h.dye_item_id,
                wear_count: i64::from(h.wear_count),
                wear_fee: i64::from(h.wear_fee),
                allowed: h.is_allowed_class(class),
            })
            .collect();
        Some(HennaEquipList {
            adena: self.adena_of(id),
            total: list.len() as i32,
            entries,
        })
    }

    /// Java `new HennaRemoveList(player)`.
    fn henna_remove_list(&self, id: ObjectId) -> HennaRemoveList {
        let entries = (1..=3)
            .filter_map(|slot| self.henna_in_slot(id, slot))
            .map(|h| HennaRemoveEntry {
                dye_id: h.dye_id,
                dye_item_id: h.dye_item_id,
                cancel_count: h.cancel_count,
                cancel_fee: h.cancel_fee,
            })
            .collect();
        HennaRemoveList {
            adena: self.adena_of(id),
            count: 3 - self.henna_empty_slots(id),
            entries,
        }
    }

    /// The body of `HennaItemDrawInfo` (`sign` 1) and `HennaItemRemoveInfo` (`sign` -1).
    fn henna_preview(&self, id: ObjectId, henna: &Henna, sign: i32, wear: bool) -> HennaPreview {
        let stats = self.stats(id);
        let now = |value: fn(&crate::stats::StatView<'_>) -> i32| stats.as_ref().map_or(0, value);
        let stat = |current: i32, bonus: i32| HennaStatPreview {
            now: current,
            after: low_byte(current + sign * bonus),
        };
        HennaPreview {
            dye_id: henna.dye_id,
            dye_item_id: henna.dye_item_id,
            count: i64::from(if wear {
                henna.wear_count
            } else {
                henna.cancel_count
            }),
            fee: i64::from(if wear { henna.wear_fee } else { henna.cancel_fee }),
            allowed: henna.is_allowed_class(self.class_of(id)),
            adena: self.adena_of(id),
            int: stat(now(|s| s.int()), henna.stat_int),
            str: stat(now(|s| s.str()), henna.stat_str),
            con: stat(now(|s| s.con()), henna.stat_con),
            men: stat(now(|s| s.men()), henna.stat_men),
            dex: stat(now(|s| s.dex()), henna.stat_dex),
            wit: stat(now(|s| s.wit()), henna.stat_wit),
        }
    }

    // ------------------------------------------------------------------ the requests

    /// The henna client packets.
    pub(crate) fn on_henna_input(&mut self, id: ObjectId, input: HennaInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            HennaInput::ItemList => self.send_henna_equip_list(id),
            HennaInput::ItemInfo { symbol_id } => self.on_request_henna_item_info(id, symbol_id),
            HennaInput::Equip { symbol_id } => self.on_request_henna_equip(id, symbol_id),
            HennaInput::RemoveList => {
                let packet = self.henna_remove_list(id);
                self.send(id, &packet);
            }
            HennaInput::ItemRemoveInfo { symbol_id } => self.on_request_henna_item_remove_info(id, symbol_id),
            HennaInput::Remove { symbol_id } => self.on_request_henna_remove(id, symbol_id),
        }
    }

    /// Java `player.sendPacket(new HennaEquipList(player))` (the `SymbolMaker` script's `Draw`).
    pub fn send_henna_equip_list(&mut self, id: ObjectId) {
        if let Some(packet) = self.henna_equip_list(id) {
            self.send(id, &packet);
        }
    }

    /// Java `player.sendPacket(new HennaRemoveList(player))` (the `SymbolMaker` script's `Remove`).
    pub fn send_henna_remove_list(&mut self, id: ObjectId) {
        let packet = self.henna_remove_list(id);
        self.send(id, &packet);
    }

    /// Java `RequestHennaItemInfo.runImpl`.
    fn on_request_henna_item_info(&mut self, id: ObjectId, symbol_id: i32) {
        let Some(henna) = self.henna_data(symbol_id) else {
            if symbol_id != 0 {
                warn!(
                    "RequestHennaItemInfo: Invalid Henna Id: {symbol_id} from {}",
                    self.player_text(id)
                );
            }
            self.send(id, &ActionFailed);
            return;
        };
        let preview = self.henna_preview(id, &henna, 1, true);
        self.send(id, &HennaItemInfo(preview));
    }

    /// Java `RequestHennaItemRemoveInfo.runImpl`.
    fn on_request_henna_item_remove_info(&mut self, id: ObjectId, symbol_id: i32) {
        if symbol_id == 0 {
            return;
        }
        let Some(henna) = self.henna_data(symbol_id) else {
            warn!(
                "RequestHennaItemRemoveInfo: Invalid Henna Id: {symbol_id} from {}",
                self.player_text(id)
            );
            self.send(id, &ActionFailed);
            return;
        };
        let preview = self.henna_preview(id, &henna, -1, false);
        self.send(id, &HennaItemRemoveInfo(preview));
    }

    /// Java `RequestHennaEquip.runImpl`.
    fn on_request_henna_equip(&mut self, id: ObjectId, symbol_id: i32) {
        if !self.flood_allows(id, FloodKind::Transaction) {
            return;
        }
        if self.henna_empty_slots(id) == 0 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::NoSlotExistsToDrawTheSymbol),
            );
            self.send(id, &ActionFailed);
            return;
        }
        let Some(henna) = self.henna_data(symbol_id) else {
            warn!(
                "RequestHennaEquip: Invalid Henna Id: {symbol_id} from {}",
                self.player_text(id)
            );
            self.send(id, &ActionFailed);
            return;
        };
        let class = self.class_of(id);
        let count = self.inventory_item_count(id, henna.dye_item_id);
        // Java's `&&` chain: `addHenna` runs (and changes the slots) only when the rest holds.
        if henna.is_allowed_class(class)
            && count >= i64::from(henna.wear_count)
            && self.adena_of(id) >= i64::from(henna.wear_fee)
            && self.add_henna(id, &henna)
        {
            self.destroy_item_by_item_id(id, henna.dye_item_id, i64::from(henna.wear_count), true);
            // The fee leaves the adena stack; `reduce_adena` sends the inventory update of it, which
            // is the `InventoryUpdate` with the adena instance Java builds by hand.
            self.reduce_adena(id, i64::from(henna.wear_fee), false);
            self.send(id, &SystemMessage::new(SystemMessageId::TheSymbolHasBeenAdded));
        } else {
            self.send(id, &SystemMessage::new(SystemMessageId::TheSymbolCannotBeDrawn));
            if !self.is_gm(id) && !henna.is_allowed_class(class) {
                let message = format!(
                    "Exploit attempt: {} tryed to add a forbidden henna.",
                    self.player_text(id)
                );
                let punishment = self.default_punish();
                self.handle_illegal_player_action(id, &message, punishment);
            }
            self.send(id, &ActionFailed);
        }
    }

    /// Java `RequestHennaRemove.runImpl`.
    fn on_request_henna_remove(&mut self, id: ObjectId, symbol_id: i32) {
        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send(id, &ActionFailed);
            return;
        }
        for slot in 1..=3 {
            let Some(henna) = self.henna_in_slot(id, slot) else {
                continue;
            };
            if henna.dye_id != symbol_id {
                continue;
            }
            if self.adena_of(id) >= i64::from(henna.cancel_fee) {
                self.remove_henna(id, slot);
            } else {
                self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
                self.send(id, &ActionFailed);
            }
            return;
        }
        warn!(
            "RequestHennaRemove: {} requested Henna Draw remove without any henna.",
            self.player_text(id)
        );
        self.send(id, &ActionFailed);
    }
}
