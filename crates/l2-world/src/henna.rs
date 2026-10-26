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
// slot (loaded at enter world, Java `restoreHenna`). The bonuses are not stored: the stat engine
// adds them from the slots (`FuncHenna`, `StatView::henna_stats`, the cap of 5 per stat), so a
// change of a slot is a change of the stats.
//
// # Database
// Java writes at once: `addHenna` is an `INSERT INTO character_hennas`, `removeHenna` a `DELETE`,
// both with the character's `class_index`. They go out as `PersistCommand::Henna` (D-15). A full
// character store does not touch the table, like Java.
//
// # The script
// D-05: the script API comes last, so `SymbolMaker` is built in like `ManorManager`
// (`manor/npc.rs`): the first click shows `symbol_maker.htm`, the `Script SymbolMaker <event>`
// bypass shows a page or opens the draw and remove lists.
//
// # Left out
// - `OnPlayerHennaAdd` / `OnPlayerHennaRemove` for scripts (no script listens yet).
// - Java reloads `_henna` when the sub class changes (`restoreHenna` in `Player.restore`, which the
//   sub class switch runs again): the switch is not in the world yet.

use l2_data::{ClassId, Henna};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::henna::{
    HennaEquipEntry, HennaEquipList, HennaItemInfo, HennaItemRemoveInfo, HennaPreview, HennaRemoveEntry,
    HennaRemoveList, HennaStatPreview,
};
use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::dialog::INTERACTION_DISTANCE;
use crate::flood::FloodKind;
use crate::target::distance_2d;
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

/// The NPCs of `SymbolMaker.NPCS`.
const SYMBOL_MAKERS: [i32; 11] = [
    31046, 31047, 31048, 31049, 31050, 31051, 31052, 31053, 31264, 31308, 31953,
];

/// The script folder (`Quest.getPath()`), below the datapack root.
const SCRIPT_PATH: &str = "data/scripts/ai/others/SymbolMaker";

/// The pages `SymbolMaker.onEvent` answers with the page itself.
const PAGES: [&str; 4] = [
    "symbol_maker.htm",
    "symbol_maker-1.htm",
    "symbol_maker-2.htm",
    "symbol_maker-3.htm",
];

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

    fn send_henna_equip_list(&mut self, id: ObjectId) {
        if let Some(packet) = self.henna_equip_list(id) {
            self.send(id, &packet);
        }
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

    // ------------------------------------------------------------------ SymbolMaker

    fn is_symbol_maker(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .is_some_and(|n| SYMBOL_MAKERS.contains(&n.npc_id))
    }

    /// `SymbolMaker.onFirstTalk` (`addFirstTalkId`, `addStartNpc`), then `Quest.showResult`.
    /// `false` when the NPC is not a symbol maker. `start_npc` is set for the click, which makes
    /// the NPC the quest NPC of the player (`ON_NPC_QUEST_START`).
    pub(crate) fn symbol_maker_first_talk(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        start_npc: bool,
    ) -> bool {
        if !self.is_symbol_maker(npc) {
            return false;
        }
        if start_npc && let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.0;
        }
        self.symbol_maker_show_page(player, npc, PAGES[0]);
        true
    }

    /// `Quest.showHtmlFile`: the page of the script, `%objectId%` and `%playername%` replaced, then
    /// `ActionFailed`.
    fn symbol_maker_show_page(&mut self, player: ObjectId, npc: ObjectId, page: &str) {
        let Some(content) = self.get_htm(player, &format!("{SCRIPT_PATH}/{page}")) else {
            debug!("{player}: page {page} of SymbolMaker not found.");
            return;
        };
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let html = content
            .replace("%objectId%", &npc.0.to_string())
            .replace("%playername%", &name);
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }

    /// Java `Player.processScriptEvent("SymbolMaker", event)`. `false` when the script is another one.
    pub(crate) fn symbol_maker_script_event(
        &mut self,
        player: ObjectId,
        script_name: &str,
        event: &str,
    ) -> bool {
        if script_name != "SymbolMaker" {
            return false;
        }
        if event.is_empty() {
            return true;
        }
        // The NPC the player talks to: the last folk, else the last quest NPC, in reach.
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return true;
        };
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |world: &Self, npc: ObjectId| match (world.position_of(player), world.position_of(npc))
        {
            (Some(me), Some(pos)) => distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        let npc = match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(self, target) => target,
            _ if quest_npc > 0 => {
                match self
                    .find_object(ObjectId(quest_npc))
                    .filter(|o| self.npcs.contains_key(o))
                {
                    Some(npc) if in_range(self, npc) => npc,
                    _ => return true,
                }
            }
            _ => return true,
        };
        // `SymbolMaker.onEvent`
        match event {
            e if PAGES.contains(&e) => self.symbol_maker_show_page(player, npc, e),
            "Draw" => self.send_henna_equip_list(player),
            "Remove" => {
                let packet = self.henna_remove_list(player);
                self.send(player, &packet);
            }
            _ => {}
        }
        true
    }
}
