//! [`ScriptCtx`]: the world a script hook runs with, and the helpers of Java's `Quest`.
// Java: mechanics/script/Quest.java (getQuestState, giveItems, takeItems, rewardItems,
//       giveAdena, getQuestItemsCount, hasQuestItems, giveItemRandomly, addExpAndSp, playSound,
//       showHtmlFile, getHtm, showResult, getNoQuestMsg, getAlreadyCompletedMsg,
//       showOnScreenMsg, addSpawn, teleportPlayer, addAttackDesire, castSkill, addRadar,
//       getRandomPartyMember*, executeForEachPlayer, openDoor, closeDoor, playMovie, the
//       one-time flags and the NR memo helpers)
// Java: entity/actor/holders/player/Radar.java
//
// The names follow Java (`give_items` = `giveItems`). Overloads become one function with the
// extra arguments spelled out, or a `_with` variant; the doc of each says which Java call it is.

use std::any::Any;

use l2_data::item::ADENA_ID;
use l2_data::stats::Stat;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{
    ActionFailed, ChatType, ExShowScreenMessage, NpcHtmlMessage, NpcSay, RadarControl, StatusUpdate,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use tracing::warn;

use super::api::ScriptNpc;
use super::qs::Qs;
use super::setup::ScriptId;
use super::sound::{QuestSound, sound_packet};
use super::state::{COMPLETED, CREATED, QuestState, STARTED};
use crate::effects::SkillKey;
use crate::enter::current_load;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::world::World;

/// Java `Quest.DEFAULT_NO_QUEST_MSG`.
pub const DEFAULT_NO_QUEST_MSG: &str = "<html><body>You are either not on a quest that involves this NPC, or you don't meet this NPC's minimum quest requirements.</body></html>";
/// Java `Quest.DEFAULT_ALREADY_COMPLETED_MSG`.
pub const DEFAULT_ALREADY_COMPLETED_MSG: &str =
    "<html><body>This quest has already been completed.</body></html>";

/// The world while one script's hook runs. Every helper a Java script inherits from `Quest` is
/// a method here; anything else of the world is reached through [`ScriptCtx::world_mut`].
pub struct ScriptCtx<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) script: ScriptId,
}

impl<'w> ScriptCtx<'w> {
    pub(crate) fn new(world: &'w mut World, script: ScriptId) -> Self {
        Self { world, script }
    }

    /// The whole world, read only.
    pub fn world(&self) -> &World {
        self.world
    }

    /// The whole world, for the calls the helpers do not cover.
    pub fn world_mut(&mut self) -> &mut World {
        self.world
    }

    /// Java `getName()` of the running script.
    pub fn script_name(&self) -> &'static str {
        self.world.scripts.name_of(self.script)
    }

    /// Java `getId()` of the running script.
    pub fn quest_id(&self) -> i32 {
        self.world.scripts.quest_id_of(self.script)
    }

    /// What the running script keeps between calls (a Java script's fields). Created with
    /// `T::default()` the first time. Scripts store one type each.
    pub fn state<T: Any + Send + Default>(&mut self) -> &mut T {
        let name = self.world.scripts.name_of(self.script);
        let slot = self
            .world
            .scripts
            .script_state
            .entry(self.script)
            .or_insert_with(|| Box::new(T::default()));
        if !slot.is::<T>() {
            warn!("{name}: script state replaced by a value of another type.");
            *slot = Box::new(T::default());
        }
        match slot.downcast_mut::<T>() {
            Some(value) => value,
            // Unreachable: the slot holds a `T` now.
            None => unreachable!("script state of the wrong type"),
        }
    }

    /// Milliseconds since the epoch (Java `System.currentTimeMillis()`).
    pub fn now_ms(&self) -> i64 {
        crate::world::unix_ms_now()
    }

    // ------------------------------------------------------------------ random (D-13)

    /// Java `getRandom(max)`: `[0, max)`.
    pub fn get_random(&mut self, max: i32) -> i32 {
        self.world.rng().below(max)
    }

    /// Java `getRandom(min, max)`: `[min, max]`.
    pub fn get_random_range(&mut self, min: i32, max: i32) -> i32 {
        self.world.rng().get(min, max)
    }

    /// Java `getRandomBoolean()`.
    pub fn get_random_boolean(&mut self) -> bool {
        self.world.rng().next_bool()
    }

    /// Java `getRandomEntry(array)`: `None` for an empty slice (Java returns null / 0).
    pub fn get_random_entry<T: Copy>(&mut self, entries: &[T]) -> Option<T> {
        if entries.is_empty() {
            return None;
        }
        let index = self.world.rng().below(entries.len() as i32);
        entries.get(index as usize).copied()
    }

    // ------------------------------------------------------------------ objects

    /// The [`ScriptNpc`] of an object that is an NPC.
    pub fn script_npc(&self, object: ObjectId) -> Option<ScriptNpc> {
        self.world.npcs.get(&object).map(|n| ScriptNpc {
            object_id: object,
            id: n.npc_id,
        })
    }

    /// Java `player.getLevel()`.
    pub fn player_level(&self, player: ObjectId) -> i32 {
        self.world.players.get(&player).map_or(0, |p| p.core.level)
    }

    /// Java `player.getName()`.
    pub fn player_name(&self, player: ObjectId) -> String {
        self.world
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `player.isGM()`.
    pub fn is_gm(&self, player: ObjectId) -> bool {
        self.world.is_gm(player)
    }

    // ------------------------------------------------------------------ quest states

    /// Java `getQuestState(player, initIfNone)` of the running script.
    pub fn get_quest_state(&mut self, player: ObjectId, init_if_none: bool) -> Option<Qs> {
        quest_state(self.world, self.script, player, init_if_none)
    }

    /// Java `ScriptManager.getScript(name).getQuestState(player, initIfNone)`: another script's
    /// state. `None` when that script is not installed.
    pub fn quest_state_of(&mut self, script_name: &str, player: ObjectId, init_if_none: bool) -> Option<Qs> {
        let script = self.world.scripts.by_name(script_name)?;
        quest_state(self.world, script, player, init_if_none)
    }

    /// Java `player.getQuestState(name)`: the state of any quest, read only.
    pub fn player_quest_state(&self, player: ObjectId, quest_name: &str) -> Option<&QuestState> {
        self.world.scripts.players.get(&player)?.get(quest_name)
    }

    /// Java `player.hasQuestState(getName())`.
    pub fn has_quest_state(&self, player: ObjectId) -> bool {
        self.world
            .scripts
            .players
            .get(&player)
            .is_some_and(|q| q.contains(self.script_name()))
    }

    /// Java `haveMemo(talker, questId)`: the player has a state of that quest.
    pub fn have_memo(&self, player: ObjectId, quest_id: i32) -> bool {
        self.world.scripts.by_quest_id(quest_id).is_some_and(|q| {
            let name = self.world.scripts.name_of(q);
            self.world
                .scripts
                .players
                .get(&player)
                .is_some_and(|p| p.contains(name))
        })
    }

    /// Java `getOneTimeQuestFlag(talker, questId)`: 1 when that quest is completed.
    pub fn get_one_time_quest_flag(&mut self, player: ObjectId, quest_id: i32) -> i32 {
        let Some(quest) = self.world.scripts.by_quest_id(quest_id) else {
            return 0;
        };
        match quest_state(self.world, quest, player, true) {
            Some(qs) if qs.is_completed(self) => 1,
            _ => 0,
        }
    }

    /// Java `setOneTimeQuestFlag(talker, questId, flag)`.
    pub fn set_one_time_quest_flag(&mut self, player: ObjectId, quest_id: i32, flag: i32) {
        let Some(quest) = self.world.scripts.by_quest_id(quest_id) else {
            return;
        };
        if let Some(qs) = quest_state(self.world, quest, player, true) {
            qs.set_state(self, if flag == 1 { COMPLETED } else { STARTED });
        }
    }

    /// Java `removeNRMemo(qs, value)`.
    pub fn remove_nr_memo(&mut self, qs: Qs, _value: i32) {
        qs.unset(self, "NRmemo");
    }

    /// Java `setNRMemoStateEx(qs, slot, unknown, value)`.
    pub fn set_nr_memo_state_ex(&mut self, qs: Qs, slot: i32, _unknown: i32, value: i32) {
        qs.set_int(self, &format!("NRmemoStateEx{slot}"), value);
    }

    /// Java `getNRMemoStateEx(qs, slot, unknown)`.
    pub fn get_nr_memo_state_ex(&self, qs: Qs, slot: i32, _unknown: i32) -> i32 {
        qs.get_int(self, &format!("NRmemoStateEx{slot}"))
    }

    /// Java `setNRFlagJournal(qs, questId, flagId)`.
    pub fn set_nr_flag_journal(&mut self, qs: Qs, _quest_id: i32, flag_id: i32) {
        qs.set_int(self, "NRFlagJournal", flag_id);
    }

    /// Java `setFlagJournal(qs, flagId)`.
    pub fn set_flag_journal(&mut self, qs: Qs, flag_id: i32) {
        qs.set_int(self, "FlagJournal", flag_id);
    }

    /// Java `setNRMemo(qs, value)`.
    pub fn set_nr_memo(&mut self, qs: Qs, value: i32) {
        qs.set_int(self, "NRmemo", value);
    }

    /// Java `setNRMemoState(qs, slot, value)`.
    pub fn set_nr_memo_state(&mut self, qs: Qs, slot: i32, value: i32) {
        qs.set_int(self, &format!("NRmemoState{slot}"), value);
    }

    /// Java `getNRMemoState(qs, slot)`.
    pub fn get_nr_memo_state(&self, qs: Qs, slot: i32) -> i32 {
        qs.get_int(self, &format!("NRmemoState{slot}"))
    }

    /// Java `haveNRMemo(qs, slot)`.
    pub fn have_nr_memo(&self, qs: Qs, slot: i32) -> bool {
        qs.get_int(self, "NRmemo") == slot
    }

    // ------------------------------------------------------------------ items

    /// Java `getQuestItemsCount(player, itemId)`: `getInventoryItemCount(itemId, -1)`.
    pub fn get_quest_items_count(&self, player: ObjectId, item_id: i32) -> i64 {
        self.world.inventory_item_count(player, item_id)
    }

    /// Java `getQuestItemsCount(player, itemIds...)` with more than one id: the counts of every
    /// item of those ids.
    pub fn get_quest_items_count_of(&self, player: ObjectId, item_ids: &[i32]) -> i64 {
        let Some(p) = self.world.players.get(&player) else {
            return 0;
        };
        let mut count: i64 = 0;
        // Java walks `getInventory().getItems()`: the inventory and the worn items.
        for item in p.data.items.iter().filter(|i| in_inventory(&i.loc)) {
            for &item_id in item_ids {
                if item.item_id == item_id {
                    // NOTE(java-quirk): Java's `count + n > Long.MAX_VALUE` can never be true, the
                    // sum wraps instead.
                    count = count.wrapping_add(item.count);
                }
            }
        }
        count
    }

    /// Java `hasQuestItems(player, itemId)`.
    pub fn has_quest_items(&self, player: ObjectId, item_id: i32) -> bool {
        self.world.inventory_item_by_item_id(player, item_id).is_some()
    }

    /// Java `hasQuestItems(player, itemIds...)`: every one of them; `false` for none.
    pub fn has_all_quest_items(&self, player: ObjectId, item_ids: &[i32]) -> bool {
        !item_ids.is_empty() && item_ids.iter().all(|&id| self.has_quest_items(player, id))
    }

    /// Java `hasAtLeastOneQuestItem(player, itemIds...)`.
    pub fn has_at_least_one_quest_item(&self, player: ObjectId, item_ids: &[i32]) -> bool {
        item_ids.iter().any(|&id| self.has_quest_items(player, id))
    }

    /// Java `getEnchantLevel(player, itemId)`.
    pub fn get_enchant_level(&self, player: ObjectId, item_id: i32) -> i32 {
        self.world
            .inventory_item_by_item_id(player, item_id)
            .map_or(0, |i| i.enchant_level)
    }

    /// Java `giveItems(player, itemId, count)`.
    pub fn give_items(&mut self, player: ObjectId, item_id: i32, count: i64) {
        self.give_items_enchanted(player, item_id, count, 0);
    }

    /// Java `giveItems(player, itemId, count, enchantlevel)`: quest items count times
    /// `QuestItemDropAmountMultiplier`; "You have earned ..." and the load.
    pub fn give_items_enchanted(&mut self, player: ObjectId, item_id: i32, count: i64, enchant_level: i32) {
        if count <= 0 {
            return;
        }
        let Some(templates) = self.world.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            return;
        };
        // Apply quest item drop amount multiplier configuration (`long * float` is a float).
        let mut final_count = count;
        if template.quest_item {
            final_count =
                (count as f32 * self.world.scripts.settings.quest_item_drop_amount_multiplier) as i64;
        }
        // Add items to player's inventory (`PlayerInventory.addItem`: no "obtained" message).
        let Some(added) = self.world.inventory_add_item_by_id(player, item_id, final_count) else {
            return;
        };
        // Set enchant level for item if that item is not adena.
        if enchant_level > 0 && item_id != ADENA_ID {
            let object_id = added.item.object_id;
            if let Some(item) = self
                .world
                .players
                .get_mut(&player)
                .and_then(|p| p.data.items.iter_mut().find(|i| i.object_id == object_id))
            {
                item.enchant_level = enchant_level;
            }
        }
        self.send_item_get_message(player, item_id, final_count);
    }

    /// Java `rewardItems(player, itemId, count)`: the count times the quest reward rate of the
    /// item's kind.
    pub fn reward_items(&mut self, player: ObjectId, item_id: i32, count: i64) {
        if count <= 0 {
            return;
        }
        let Some(templates) = self.world.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            return;
        };
        let rates = &self.world.scripts.settings;
        // Java `count *= rate` on a long: the product is a float, cast back.
        let rate = if item_id == ADENA_ID {
            Some(rates.rate_quest_reward_adena)
        } else if rates.rate_quest_reward_use_multipliers {
            // Only etc items get a multiplier; the others keep their count.
            crate::inventory::etc_type(template).map(|etc| {
                use l2_data::item::EtcItemType;
                match etc {
                    EtcItemType::Potion => rates.rate_quest_reward_potion,
                    EtcItemType::ScrlEnchantWp | EtcItemType::ScrlEnchantAm | EtcItemType::Scroll => {
                        rates.rate_quest_reward_scroll
                    }
                    EtcItemType::Recipe => rates.rate_quest_reward_recipe,
                    EtcItemType::Material => rates.rate_quest_reward_material,
                    _ => rates.rate_quest_reward,
                }
            })
        } else {
            Some(rates.rate_quest_reward)
        };
        let count = match rate {
            Some(rate) => (count as f32 * rate) as i64,
            None => count,
        };
        if self
            .world
            .inventory_add_item_by_id(player, item_id, count)
            .is_none()
        {
            return;
        }
        self.send_item_get_message(player, item_id, count);
    }

    /// Java `giveAdena(player, count, applyRates)`.
    pub fn give_adena(&mut self, player: ObjectId, count: i64, apply_rates: bool) {
        if apply_rates {
            self.reward_items(player, ADENA_ID, count);
        } else {
            self.give_items(player, ADENA_ID, count);
        }
    }

    /// Java `Quest.sendItemGetMessage(player, item, count)`.
    fn send_item_get_message(&mut self, player: ObjectId, item_id: i32, count: i64) {
        let templates = self.world.item_templates();
        let message = if item_id == ADENA_ID {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS1Adena)
                .with(SystemMessageParam::LongNumber(count))
        } else if count > 1 {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                .with(item_name_param(templates.as_deref(), item_id))
                .with(SystemMessageParam::LongNumber(count))
        } else {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS1)
                .with(item_name_param(templates.as_deref(), item_id))
        };
        self.world.send(player, &message);
        let Some(p) = self.world.players.get(&player) else {
            return;
        };
        let mut update = StatusUpdate::new(player.0);
        update.add_attribute(status::CUR_LOAD, current_load(&p.data, &*self.world.data));
        self.world.send(player, &update);
    }

    /// Java `giveItemRandomly(player, npc, itemId, minAmount, maxAmount, limit, dropChance,
    /// playSound)`. `true` when the item was given and the limit is reached (or there is no
    /// limit). Champions are not rolled in this server, so their bonus never applies.
    #[allow(clippy::too_many_arguments)]
    pub fn give_item_randomly(
        &mut self,
        player: ObjectId,
        _npc: Option<ScriptNpc>,
        item_id: i32,
        min_amount: i64,
        max_amount: i64,
        limit: i64,
        drop_chance: f64,
        play_sound: bool,
    ) -> bool {
        let current = self.get_quest_items_count(player, item_id);
        if limit > 0 && current >= limit {
            return true;
        }
        let multiplier = self.world.scripts.settings.quest_item_drop_amount_multiplier;
        let min_with_bonus = (min_amount as f32 * multiplier) as i64;
        let max_with_bonus = (max_amount as f32 * multiplier) as i64;
        let mut amount = if min_with_bonus == max_with_bonus {
            min_with_bonus
        } else {
            self.world.rng().get_i64(min_with_bonus, max_with_bonus)
        };
        let random = self.world.rng().next_f64();
        // Inventory slot check (almost useless for non-stacking items).
        if drop_chance >= random && amount > 0 && self.world.validate_capacity_by_item_id(player, item_id) {
            if limit > 0 && current + amount > limit {
                amount = limit - current;
            }
            // Give the item to player (`player.addItem(..., true)`: "You have obtained ...").
            if self.world.add_item(player, item_id, amount, true).is_some() {
                // Limit reached (if there is no limit, this block doesn't execute).
                if current + amount == limit {
                    if play_sound {
                        self.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    }
                    return true;
                }
                if play_sound {
                    self.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
                // If there is no limit, return true every time an item is given.
                if limit <= 0 {
                    return true;
                }
            }
        }
        false
    }

    /// Java `takeItems(player, itemId, amount)`: `amount < 0` takes all. Each item goes
    /// through `destroyItemByItemId` (with its "has disappeared" message); a worn item is
    /// taken off first.
    pub fn take_items(&mut self, player: ObjectId, item_id: i32, amount: i64) -> bool {
        let items: Vec<(ObjectId, i64)> = self.world.players.get(&player).map_or_else(Vec::new, |p| {
            p.data
                .items
                .iter()
                .filter(|i| i.item_id == item_id && in_inventory(&i.loc))
                .map(|i| (i.object_id, i.count))
                .collect()
        });
        if amount < 0 {
            for (object_id, count) in items {
                self.take_item(player, object_id, item_id, count);
            }
        } else {
            let mut current: i64 = 0;
            for (object_id, count) in items {
                let mut to_delete = count;
                if current + to_delete > amount {
                    to_delete = amount - current;
                }
                if to_delete > 0 {
                    self.take_item(player, object_id, item_id, to_delete);
                }
                current += to_delete;
            }
        }
        true
    }

    /// Java `takeItems(player, amount, itemIds...)`.
    pub fn take_items_many(&mut self, player: ObjectId, amount: i64, item_ids: &[i32]) -> bool {
        let mut check = true;
        for &item_id in item_ids {
            check &= self.take_items(player, item_id, amount);
        }
        check
    }

    /// Java `Quest.takeItem(player, item, toDelete)`.
    fn take_item(&mut self, player: ObjectId, object_id: ObjectId, item_id: i32, to_delete: i64) -> bool {
        let equipped = self.world.players.get(&player).and_then(|p| {
            p.data
                .items
                .iter()
                .find(|i| i.object_id == object_id && i.loc == LOC_PAPERDOLL)
                .cloned()
        });
        if let Some(item) = equipped
            && let Some(templates) = self.world.item_templates()
            && let Some(template) = templates.get(item.item_id)
            && let Some(part) = crate::inventory::body_part_of_worn_item(&item, template)
        {
            let changed = self.world.unequip_item_in_body_slot_and_record(player, part);
            let entries = changed
                .iter()
                .filter_map(|id| self.world.modified_entry(player, *id))
                .collect();
            self.world.send_inventory_update_now(player, entries);
            self.world.broadcast_user_info(player);
        }
        // NOTE(java-quirk): `destroyItemByItemId` destroys from the first item of the id, not
        // necessarily the one this loop is at.
        self.world
            .destroy_item_by_item_id(player, item_id, to_delete, true)
    }

    // ------------------------------------------------------------------ exp

    /// Java `addExpAndSp(player, exp, sp)`: times the quest reward rates and the player's
    /// `EXPSP_RATE`; the PC cafe points of the exp. Premium rates are M9.
    pub fn add_exp_and_sp(&mut self, player: ObjectId, exp: i64, sp: i32) {
        let rates = &self.world.scripts.settings;
        // `addExp * RATE_QUEST_REWARD_XP`: a long times a float is a float.
        let exp_rated = exp as f32 * rates.rate_quest_reward_xp;
        let sp_rated = sp as f32 * rates.rate_quest_reward_sp;
        let calc = |world: &World, value: f64| {
            world
                .stats(player)
                .map_or(value, |v| v.calc_stat(Stat::ExpspRate, value, None, None))
        };
        let add_exp = calc(self.world, f64::from(exp_rated)) as i64;
        let add_sp = calc(self.world, f64::from(sp_rated)) as i32;
        self.world
            .add_exp_and_sp(player, add_exp as f64, f64::from(add_sp), false);
        self.world.give_pc_cafe_point(player, exp_rated as i64 as f64);
    }

    // ------------------------------------------------------------------ sounds, messages

    /// Java `playSound(player, QuestSound)`.
    pub fn play_sound(&mut self, player: ObjectId, sound: QuestSound) {
        self.world.send(player, &sound.packet());
    }

    /// Java `playSound(player, soundName)`.
    pub fn play_sound_named(&mut self, player: ObjectId, sound: &str) {
        self.world.send(player, &sound_packet(sound));
    }

    /// Java `showOnScreenMsg(player, text, time)`.
    pub fn show_on_screen_msg(&mut self, player: ObjectId, text: &str, time: i32) {
        self.world.send(player, &ExShowScreenMessage::text(text, time));
    }

    /// Java `showOnScreenMsg(player, npcString, position, time, params...)`.
    pub fn show_on_screen_npc_string(
        &mut self,
        player: ObjectId,
        npc_string: NpcStringId,
        position: i32,
        time: i32,
        params: &[&str],
    ) {
        self.world.send(
            player,
            &ExShowScreenMessage::npc_string(npc_string, position, time, params),
        );
    }

    /// Java `showOnScreenMsg(player, systemMsg, position, time, params...)`.
    pub fn show_on_screen_system_message(
        &mut self,
        player: ObjectId,
        message: SystemMessageId,
        position: i32,
        time: i32,
        params: &[&str],
    ) {
        self.world.send(
            player,
            &ExShowScreenMessage::system_message(message, position, time, params),
        );
    }

    /// Java `npc.broadcastSay(chatType, npcStringId, params...)`: `NpcSay` to the players who
    /// see the NPC.
    pub fn broadcast_npc_say(
        &mut self,
        npc: ScriptNpc,
        chat: ChatType,
        npc_string: NpcStringId,
        params: &[&str],
    ) {
        let packet = NpcSay {
            object_id: npc.object_id.0,
            text_type: chat.client_id(),
            npc_id: self.npc_display_id(npc),
            npc_string: npc_string.id(),
            text: String::new(),
            parameters: params.iter().map(|p| (*p).to_owned()).collect(),
        };
        self.world.broadcast_packet(npc.object_id, &packet);
    }

    /// Java `npc.broadcastSay(chatType, text)`.
    pub fn broadcast_npc_say_text(&mut self, npc: ScriptNpc, chat: ChatType, text: &str) {
        let packet = NpcSay {
            object_id: npc.object_id.0,
            text_type: chat.client_id(),
            npc_id: self.npc_display_id(npc),
            npc_string: -1,
            text: text.to_owned(),
            parameters: Vec::new(),
        };
        self.world.broadcast_packet(npc.object_id, &packet);
    }

    /// `npc.getTemplate().getDisplayId()`.
    fn npc_display_id(&self, npc: ScriptNpc) -> i32 {
        self.world
            .npc_world
            .template(npc.id)
            .map_or(npc.id, |t| t.display_id)
    }

    // ------------------------------------------------------------------ html

    /// Java `getHtm(player, fileName)`: the script's folder, then `data/scripts/quests/<name>`.
    pub fn get_htm(&self, player: ObjectId, file_name: &str) -> Option<String> {
        get_htm(self.world, self.script, player, file_name)
    }

    /// Java `showHtmlFile(player, fileName, npc)`: the page goes out (an `.htm` of a quest as
    /// the quest window); returns its text.
    pub fn show_html_file(
        &mut self,
        player: ObjectId,
        file_name: &str,
        npc: Option<ScriptNpc>,
    ) -> Option<String> {
        show_html_file(self.world, self.script, player, file_name, npc)
    }

    /// Java `getNoQuestMsg(player)`.
    pub fn get_no_quest_msg(&self, player: ObjectId) -> String {
        no_quest_msg(self.world, player)
    }

    /// Java `getAlreadyCompletedMsg(player)`.
    pub fn get_already_completed_msg(&self, player: ObjectId) -> String {
        self.world
            .get_htm(player, "data/html/alreadycompleted.htm")
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| DEFAULT_ALREADY_COMPLETED_MSG.to_owned())
    }

    /// Java `showResult(player, res, npc)`.
    pub fn show_result(&mut self, player: ObjectId, res: Option<&str>, npc: Option<ScriptNpc>) {
        show_result(self.world, self.script, player, res, npc);
    }

    // ------------------------------------------------------------------ timers

    /// Java `startQuestTimer(name, time, npc, player, repeating)`: `on_event(name, npc, player)`
    /// after `time_ms` (and every `time_ms` when repeating). A timer of the same name, NPC and
    /// player that is running already is kept and the new one dropped.
    pub fn start_quest_timer(
        &mut self,
        name: &str,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
        repeating: bool,
    ) {
        self.world
            .start_quest_timer(self.script, name, time_ms, npc, player, repeating);
    }

    /// Java `ScriptManager.getScript(name).startQuestTimer(...)`: the timer of another script (its
    /// `on_event` runs). Nothing when the script is not installed.
    pub fn start_quest_timer_of(
        &mut self,
        script_name: &str,
        name: &str,
        time_ms: i64,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
        repeating: bool,
    ) {
        if let Some(script) = self.world.scripts.by_name(script_name) {
            self.world
                .start_quest_timer(script, name, time_ms, npc, player, repeating);
        }
    }

    /// Java `getQuestTimer(name, npc, player) != null`.
    pub fn has_quest_timer(&self, name: &str, npc: Option<ScriptNpc>, player: Option<ObjectId>) -> bool {
        self.world
            .scripts
            .timers
            .find(self.script, name, npc.map(|n| n.object_id), player)
            .is_some()
    }

    /// Java `cancelQuestTimer(name, npc, player)`.
    pub fn cancel_quest_timer(&mut self, name: &str, npc: Option<ScriptNpc>, player: Option<ObjectId>) {
        self.world
            .cancel_quest_timer(self.script, name, npc.map(|n| n.object_id), player);
    }

    /// Java `cancelQuestTimers(name)`.
    pub fn cancel_quest_timers(&mut self, name: &str) {
        self.world.cancel_quest_timers(self.script, name);
    }

    // ------------------------------------------------------------------ party

    /// Java `getRandomPartyMember(player)`: a random member in the player's instance, or the
    /// player without a party.
    pub fn get_random_party_member(&mut self, player: ObjectId) -> Option<ObjectId> {
        if !self.world.players.contains_key(&player) {
            return None;
        }
        let Some(members) = self.party_members(player) else {
            return Some(player);
        };
        let instance = self.world.instance_id_of(player);
        if !members.iter().any(|m| self.world.instance_id_of(*m) == instance) {
            // NOTE(java-quirk): Java draws until a member is in the player's instance; the
            // player is always one, so this never happens there.
            return Some(player);
        }
        loop {
            let index = self.world.rng().below(members.len() as i32) as usize;
            let member = members[index];
            if self.world.instance_id_of(member) == instance {
                return Some(member);
            }
        }
    }

    /// Java `getRandomPartyMember(player, cond)`.
    pub fn get_random_party_member_cond(&mut self, player: ObjectId, cond: i32) -> Option<ObjectId> {
        self.get_random_party_member_var(player, Some("cond"), &cond.to_string())
    }

    /// Java `getRandomPartyMember(player, var, value)`: a random member (in range of the
    /// player's target, in the same instance) whose state of this quest has `var` equal to
    /// `value`, ignoring case. `var` `None` is any member.
    pub fn get_random_party_member_var(
        &mut self,
        player: ObjectId,
        var: Option<&str>,
        value: &str,
    ) -> Option<ObjectId> {
        if !self.world.players.contains_key(&player) {
            return None;
        }
        let Some(var) = var else {
            return self.get_random_party_member(player);
        };
        let matches = |ctx: &Self, member: ObjectId| {
            ctx.player_quest_state(member, ctx.script_name())
                .and_then(|qs| qs.get(var))
                .is_some_and(|v| v.eq_ignore_ascii_case(value))
        };
        self.random_party_candidate(player, matches)
    }

    /// Java `getRandomPartyMemberState(player, state)`.
    pub fn get_random_party_member_state(&mut self, player: ObjectId, state: u8) -> Option<ObjectId> {
        if !self.world.players.contains_key(&player) {
            return None;
        }
        let matches = |ctx: &Self, member: ObjectId| {
            ctx.player_quest_state(member, ctx.script_name())
                .is_some_and(|qs| qs.state() == state)
        };
        self.random_party_candidate(player, matches)
    }

    /// The common part of `getRandomPartyMember(player, var, value)` and
    /// `getRandomPartyMemberState(player, state)`.
    fn random_party_candidate(
        &mut self,
        player: ObjectId,
        matches: impl Fn(&Self, ObjectId) -> bool,
    ) -> Option<ObjectId> {
        let Some(members) = self.party_members(player) else {
            return matches(self, player).then_some(player);
        };
        // Get the target for enforcing distance limitations.
        let target = self.world.target_of(player).unwrap_or(player);
        let range = f64::from(self.world.settings.party.alt_party_range);
        let instance = self.world.instance_id_of(player);
        let target_pos = self.world.position_of(target);
        let candidates: Vec<ObjectId> = members
            .into_iter()
            .filter(|&m| {
                matches(self, m)
                    && match (self.world.position_of(m), target_pos) {
                        (Some(a), Some(b)) => crate::target::distance_3d(a, b) <= range,
                        _ => false,
                    }
                    && self.world.instance_id_of(m) == instance
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let index = self.world.rng().below(candidates.len() as i32) as usize;
        candidates.get(index).copied()
    }

    /// Java `getRandomPartyMember(player, npc)`: the member with the highest roll of 1000 that
    /// passes [`super::Script::check_party_member`], in range of the NPC.
    pub fn random_party_member_for_npc(
        &mut self,
        player: ObjectId,
        npc: Option<ScriptNpc>,
    ) -> Option<ObjectId> {
        if !self.world.players.contains_key(&player) || !self.check_distance_to_target(player, npc) {
            return None;
        }
        let script = self.world.scripts.script(self.script)?;
        let mut lucky = None;
        if let Some(members) = self.party_members(player) {
            let mut highest_roll = 0;
            for member in members {
                let roll = self.get_random(1000);
                if roll > highest_roll && script.check_party_member(self, member, npc) {
                    highest_roll = roll;
                    lucky = Some(member);
                }
            }
        } else if script.check_party_member(self, player, npc) {
            lucky = Some(player);
        }
        lucky.filter(|&l| self.check_distance_to_target(l, npc))
    }

    /// Java `getRandomPartyMemberState(player, condition, playerChance, target)`: the player
    /// counts `player_chance` times, every other member once; `condition` -1 is any started
    /// state.
    pub fn random_party_member_state(
        &mut self,
        player: ObjectId,
        condition: i32,
        player_chance: i32,
        target: Option<ScriptNpc>,
    ) -> Option<Qs> {
        if !self.world.players.contains_key(&player) || player_chance < 1 {
            return None;
        }
        let script = self.world.scripts.script(self.script)?;
        let qs = quest_state(self.world, self.script, player, false);
        let Some(members) = self.party_members(player) else {
            let ok = self.check_party_member_conditions(&*script, qs, condition, target)
                && self.check_distance_to_target(player, target);
            return if ok { qs } else { None };
        };
        let mut candidates = Vec::new();
        if self.check_party_member_conditions(&*script, qs, condition, target)
            && let Some(qs) = qs
        {
            for _ in 0..player_chance {
                candidates.push(qs);
            }
        }
        for member in members {
            if member == player {
                continue;
            }
            let qs = quest_state(self.world, self.script, member, false);
            if self.check_party_member_conditions(&*script, qs, condition, target)
                && let Some(qs) = qs
            {
                candidates.push(qs);
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let index = self.get_random(candidates.len() as i32) as usize;
        let qs = *candidates.get(index)?;
        self.check_distance_to_target(qs.player, target).then_some(qs)
    }

    /// Java `player.getParty().getMembers()`; `None` without a party.
    fn party_members(&self, player: ObjectId) -> Option<Vec<ObjectId>> {
        self.world
            .party_of(player)
            .map(|party| self.world.party_members(party))
    }

    fn check_party_member_conditions(
        &mut self,
        script: &dyn super::Script,
        qs: Option<Qs>,
        condition: i32,
        npc: Option<ScriptNpc>,
    ) -> bool {
        let Some(qs) = qs else {
            return false;
        };
        let state_ok = if condition == -1 {
            qs.is_started(self)
        } else {
            qs.is_cond(self, condition)
        };
        state_ok && script.check_party_member_state(self, qs, npc)
    }

    /// Java `checkDistanceToTarget(player, target)`: `checkIfInRange(ALT_PARTY_RANGE, player,
    /// target, true)`.
    fn check_distance_to_target(&self, player: ObjectId, target: Option<ScriptNpc>) -> bool {
        match target {
            None => true,
            Some(npc) => self.world.check_if_in_range(
                self.world.settings.party.alt_party_range,
                player,
                npc.object_id,
                true,
            ),
        }
    }

    /// Java `executeForEachPlayer(player, npc, isSummon, includeParty, includeCommandChannel)`.
    pub fn execute_for_each_player(
        &mut self,
        player: ObjectId,
        npc: ScriptNpc,
        is_summon: bool,
        include_party: bool,
        include_command_channel: bool,
    ) {
        let Some(script) = self.world.scripts.script(self.script) else {
            return;
        };
        let in_party = self.world.party_of(player).is_some();
        if (include_party || include_command_channel) && in_party {
            let channel = self.world.command_channel_of(player);
            let members = if include_command_channel && let Some(channel) = channel {
                self.world.command_channel_members(channel)
            } else if include_party {
                self.world.party_members_of(player)
            } else {
                Vec::new()
            };
            for member in members {
                script.action_for_each_player(self, member, npc, is_summon);
            }
        } else {
            script.action_for_each_player(self, player, npc, is_summon);
        }
    }

    // ------------------------------------------------------------------ npcs and players

    /// Java `addSpawn(npcId, x, y, z, heading, randomOffset, despawnDelay, isSummonSpawn,
    /// instanceId)`: a spawn that does not respawn. `None` for (0, 0) coordinates or an NPC
    /// that cannot be made.
    #[allow(clippy::too_many_arguments)]
    pub fn add_spawn(
        &mut self,
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
        random_offset: bool,
        despawn_delay_ms: i64,
        instance_id: i32,
    ) -> Option<ScriptNpc> {
        self.world
            .script_add_spawn(
                npc_id,
                (x, y, z),
                heading,
                random_offset,
                despawn_delay_ms,
                instance_id,
            )
            .map(|object_id| ScriptNpc {
                object_id,
                id: npc_id,
            })
    }

    /// Java `addSpawn(npcId, npc, randomOffset, despawnDelay)`: at an object's place, heading
    /// and instance.
    pub fn add_spawn_at(
        &mut self,
        npc_id: i32,
        at: ObjectId,
        random_offset: bool,
        despawn_delay_ms: i64,
    ) -> Option<ScriptNpc> {
        let (x, y, z) = self.world.position_of(at)?;
        let heading = self.world.heading_of(at);
        let instance = self.world.instance_id_of(at);
        self.add_spawn(
            npc_id,
            x,
            y,
            z,
            heading,
            random_offset,
            despawn_delay_ms,
            instance,
        )
    }

    /// Java `teleportPlayer(player, loc, instanceId, allowRandomOffset)`.
    pub fn teleport_player(
        &mut self,
        player: ObjectId,
        x: i32,
        y: i32,
        z: i32,
        instance_id: i32,
        allow_random_offset: bool,
    ) {
        let offset = if allow_random_offset {
            self.world.settings.teleport.max_offset_on_teleport
        } else {
            0
        };
        self.world.tele_to_location(
            player,
            crate::teleport::TeleportTarget::at(x, y, z)
                .with_instance(instance_id)
                .with_random_offset(offset),
        );
    }

    /// Java `addAttackDesire(npc, target, desire)` (999 in `addAttackDesire(npc, target)`).
    pub fn add_attack_desire(&mut self, npc: ScriptNpc, target: ObjectId, desire: i64) {
        if self.world.is_attackable_npc(npc.object_id) {
            self.world.add_damage_hate(npc.object_id, target, 0, desire);
        }
        self.world.set_running(npc.object_id, true);
        self.world.set_intention_attack(npc.object_id, target);
    }

    /// Java `castSkill(npc, target, skill)`: the NPC targets and casts.
    pub fn cast_skill(&mut self, npc: ScriptNpc, target: ObjectId, skill: SkillKey) {
        self.world.set_npc_target(npc.object_id, Some(target));
        self.world.do_cast(npc.object_id, skill);
    }

    /// Java `playMovie(player, movie)`: a `MovieHolder`; the player holds the movie until the
    /// client answers with `EndScenePlayer` (S-API-03).
    pub fn play_movie(&mut self, player: ObjectId, movie_id: i32) {
        self.world.script_play_movie(&[player], movie_id);
    }

    /// Java `playMovie(players, movie)`: a `MovieHolder` for several players.
    pub fn play_movie_for(&mut self, players: &[ObjectId], movie_id: i32) {
        self.world.script_play_movie(players, movie_id);
    }

    /// Java `openDoor(doorId, instanceId)`.
    pub fn open_door(&mut self, door_id: i32, instance_id: i32) {
        match self.door_object(door_id, instance_id) {
            Some(door) => {
                if self.world.door(door).is_some_and(|d| !d.open) {
                    self.world.open_door(door);
                }
            }
            None => warn!(
                "{}: called openDoor({door_id}, {instance_id}); but door was not found!",
                self.script_name()
            ),
        }
    }

    /// Java `closeDoor(doorId, instanceId)`.
    pub fn close_door(&mut self, door_id: i32, instance_id: i32) {
        match self.door_object(door_id, instance_id) {
            Some(door) => {
                if self.world.door(door).is_some_and(|d| d.open) {
                    self.world.close_door(door);
                }
            }
            None => warn!(
                "{}: called closeDoor({door_id}, {instance_id}); but door was not found!",
                self.script_name()
            ),
        }
    }

    /// Java `getDoor(doorId, instanceId)`.
    fn door_object(&self, door_id: i32, instance_id: i32) -> Option<ObjectId> {
        if instance_id != 0 && self.world.instance(instance_id).is_some() {
            return self.world.instance_door(instance_id, door_id);
        }
        self.world.door_by_door_id(door_id).map(|d| d.object_id)
    }

    // ------------------------------------------------------------------ radar

    /// Java `addRadar(player, x, y, z)`.
    pub fn add_radar(&mut self, player: ObjectId, x: i32, y: i32, z: i32) {
        let markers = self.world.scripts.radar.entry(player).or_default();
        if !markers.contains(&(x, y, z)) {
            markers.push((x, y, z));
        }
        self.world.send(player, &radar(2, 2, x, y, z));
        self.world.send(player, &radar(0, 1, x, y, z));
    }

    /// Java `removeRadar(player, x, y, z)`.
    pub fn remove_radar(&mut self, player: ObjectId, x: i32, y: i32, z: i32) {
        if let Some(markers) = self.world.scripts.radar.get_mut(&player) {
            markers.retain(|m| *m != (x, y, z));
        }
        self.world.send(player, &radar(1, 1, x, y, z));
    }

    /// Java `clearRadar(player)`.
    pub fn clear_radar(&mut self, player: ObjectId) {
        // NOTE(java-quirk): Java walks a hash set; the markers go in the order they came.
        let markers = self.world.scripts.radar.remove(&player).unwrap_or_default();
        for (x, y, z) in markers {
            self.world.send(player, &radar(2, 2, x, y, z));
        }
    }

    // ------------------------------------------------------------------ other scripts

    /// Java `getRegisteredIds(type)`: the ids the running script registered for an event.
    pub fn registered_ids(&self, kind: super::EventKind) -> Vec<i32> {
        self.world.scripts.listeners.registered_ids(self.script, kind)
    }

    /// Java `ScriptManager.getScript(name).notifyEvent(event, npc, player)`.
    pub fn notify_event_of(
        &mut self,
        script_name: &str,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        if let Some(script) = self.world.scripts.by_name(script_name) {
            self.world.script_notify_event(script, event, npc, player);
        }
    }

    /// Java `notifyEvent(event, npc, player)` of the running script.
    pub fn notify_event(&mut self, event: &str, npc: Option<ScriptNpc>, player: Option<ObjectId>) {
        self.world.script_notify_event(self.script, event, npc, player);
    }

    /// Java `GlobalVariablesManager.getInstance()`, read only.
    pub fn global_variables(&self) -> &super::GlobalVariables {
        &self.world.scripts.globals
    }

    /// Java `GlobalVariablesManager.getInstance().set(name, value)`.
    pub fn set_global_variable(&mut self, name: &str, value: &str) {
        self.world.scripts.globals.set(name, value);
    }

    /// Java `GlobalVariablesManager.getInstance().remove(name)`.
    pub fn remove_global_variable(&mut self, name: &str) {
        self.world.scripts.globals.remove(name);
    }
}

fn radar(show_radar: i32, radar_type: i32, x: i32, y: i32, z: i32) -> RadarControl {
    RadarControl {
        show_radar,
        radar_type,
        x,
        y,
        z,
    }
}

fn in_inventory(loc: &str) -> bool {
    loc == LOC_INVENTORY || loc == LOC_PAPERDOLL
}

/// Java `Quest.getQuestState(player, initIfNone)` for any script: `newQuestState(player)` makes
/// a `CREATED` state and registers it with the player (no database row yet).
pub(crate) fn quest_state(
    world: &mut World,
    script: ScriptId,
    player: ObjectId,
    init_if_none: bool,
) -> Option<Qs> {
    if !world.players.contains_key(&player) {
        return None;
    }
    let name = world.scripts.name_of(script);
    let quests = world.scripts.players.entry(player).or_default();
    if quests.contains(name) {
        return Some(Qs { player, script });
    }
    if !init_if_none {
        return None;
    }
    quests.insert(QuestState::new(name, CREATED));
    Some(Qs { player, script })
}

/// Java `Quest.getNoQuestMsg(player)`.
pub(crate) fn no_quest_msg(world: &World, player: ObjectId) -> String {
    world
        .get_htm(player, "data/html/noquest.htm")
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| DEFAULT_NO_QUEST_MSG.to_owned())
}

/// Java `Quest.getHtm(player, fileName)`.
pub(crate) fn get_htm(world: &World, script: ScriptId, player: ObjectId, file_name: &str) -> Option<String> {
    let s = world.scripts.script(script)?;
    let in_folder = format!("data/scripts/{}/{file_name}", s.path());
    let first = if file_name.starts_with("data/") {
        file_name.to_owned()
    } else {
        in_folder.clone()
    };
    world
        .get_htm(player, &first)
        .or_else(|| world.get_htm(player, &in_folder))
        .or_else(|| world.get_htm(player, &format!("data/scripts/quests/{}/{file_name}", s.name())))
}

/// Java `Quest.showHtmlFile(player, filename, npc)`.
pub(crate) fn show_html_file(
    world: &mut World,
    script: ScriptId,
    player: ObjectId,
    file_name: &str,
    npc: Option<ScriptNpc>,
) -> Option<String> {
    let quest_window = !file_name.ends_with(".html");
    let mut content = get_htm(world, script, player, file_name)?;
    if let Some(npc) = npc {
        content = content.replace("%objectId%", &npc.object_id.0.to_string());
    }
    let quest_id = world.scripts.quest_id_of(script);
    let npc_obj_id = npc.map_or(0, |n| n.object_id.0);
    let name = world
        .players
        .get(&player)
        .map(|p| p.data.name.clone())
        .unwrap_or_default();
    if quest_window && quest_id > 0 && quest_id < 20000 && quest_id != 999 {
        if let Some(mut message) =
            l2_protocol_game::server::NpcQuestHtmlMessage::new(npc_obj_id, quest_id, &content)
        {
            message.html = message.html.replace("%playername%", &name);
            world.send_quest_html(player, &message);
        }
    } else if let Some(mut message) = NpcHtmlMessage::new(npc_obj_id, &content) {
        message.html = message.html.replace("%playername%", &name);
        world.send_html(player, &message);
    }
    world.send(player, &ActionFailed);
    Some(content)
}

/// Java `Quest.showResult(player, res, npc)`.
pub(crate) fn show_result(
    world: &mut World,
    script: ScriptId,
    player: ObjectId,
    res: Option<&str>,
    npc: Option<ScriptNpc>,
) {
    let Some(res) = res.filter(|r| !r.is_empty()) else {
        return;
    };
    if !world.players.contains_key(&player) {
        return;
    }
    if res.ends_with(".htm") || res.ends_with(".html") {
        show_html_file(world, script, player, res, npc);
    } else if res.starts_with("<html") {
        let name = world
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if let Some(mut message) = NpcHtmlMessage::new(npc.map_or(0, |n| n.object_id.0), res) {
            message.html = message.html.replace("%playername%", &name);
            world.send_html(player, &message);
        }
        world.send(player, &ActionFailed);
    } else {
        world.send_message(player, res);
    }
}
