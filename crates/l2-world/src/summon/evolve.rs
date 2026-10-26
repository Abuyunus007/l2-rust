//! The pet manager's work: the exchange of wolf collars, the evolution of a pet into the stronger
//! one and the restore of a pet from the collar of its evolved form (M6-FU-14).
// Java: entity/actor/instance/PetManager.java (onBypassFeedback, exchange, doEvolve, doRestore,
//       EvolveFeedWait, EvolveFinalizer)
// Java: entity/actor/instance/Pet.java (spawnPet, destroyControlItem(owner, evolve))
// Java: entity/actor/instance/ClanHallDoorman.java (the evolve commands, `clanhall/doorman.rs`)
//
// `doEvolve` and `doRestore` are static in Java and called by the pet managers and by the clan
// hall doorman of the halls with the evolve function: [`World::pet_do_evolve`] and
// [`World::pet_do_restore`] take the NPC the player talks to for the packets.
//
// # Left out
// - `PetManager.showChatWindow` (the `restore-unsummonpet` page of NPC 36478 and the rent pet
//   link of `AllowRentPet`) and the rest of `Merchant` (buy and sell lists) are the generic NPC's.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    InventoryUpdate, MagicSkillLaunched, MagicSkillUse, SkillPosition, SystemMessage,
};
use l2_util::ObjectId;
use tracing::warn;

use super::SummonTimer;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// The skill the client shows for the summoning (`MagicSkillUse(npc, 2046, 1, 1000, 600000)`).
const SUMMON_SKILL_ID: i32 = 2046;
/// `ThreadPool.schedule(new EvolveFinalizer(player, petSummon), 900)`.
const FINALIZER_DELAY_MS: u64 = 900;
/// `ThreadPool.schedule(new EvolveFeedWait(player, petSummon), 60000)`.
const FEED_WAIT_DELAY_MS: u64 = 60_000;

/// `onBypassFeedback`'s `exchange`: `(item taken, item given)`.
const EXCHANGES: [(i32, i32); 3] = [(7585, 6650), (7583, 6648), (7584, 6649)];
/// `onBypassFeedback`'s `evolve`: `(collar taken, collar given, level required)`.
const EVOLUTIONS: [(i32, i32, i32); 5] = [
    (2375, 9882, 55),
    (9882, 10426, 70),
    (6648, 10311, 55),
    (6650, 10313, 55),
    (6649, 10312, 55),
];
/// `onBypassFeedback`'s `restore`: `(collar taken, collar given, level)`.
const RESTORES: [(i32, i32, i32); 5] = [
    (10307, 9882, 55),
    (10611, 10426, 70),
    (10308, 4422, 55),
    (10309, 4423, 55),
    (10310, 4424, 55),
];

impl World {
    /// Java `PetManager.onBypassFeedback(player, command)`; anything else is the merchant's.
    pub(crate) fn pet_manager_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        let exchange = command.starts_with("exchange");
        let evolve = command.starts_with("evolve");
        let restore = command.starts_with("restore");
        if !(exchange || evolve || restore) {
            self.generic_npc_bypass_feedback(npc, player, command);
            return;
        }
        // `Integer.parseInt(command.split(" ")[1])` throws (and Java logs it) for anything else.
        let Some(value) = command.split(' ').nth(1).and_then(|t| t.parse::<i32>().ok()) else {
            warn!("Exception processing bypass from {player}: {command} (bad pet manager command)");
            return;
        };
        let index = usize::try_from(value).ok().and_then(|v| v.checked_sub(1));
        if exchange {
            if let Some(&(take, give)) = index.and_then(|i| EXCHANGES.get(i)) {
                self.pet_manager_exchange(npc, player, take, give);
            }
        } else if evolve {
            let ok = index
                .and_then(|i| EVOLUTIONS.get(i))
                .is_some_and(|&(take, give, level)| self.pet_do_evolve(player, npc, take, give, level));
            if !ok {
                self.pet_manager_page(npc, player, "data/html/petmanager/evolve_no.htm");
            }
        } else {
            let ok = index
                .and_then(|i| RESTORES.get(i))
                .is_some_and(|&(take, give, level)| self.pet_do_restore(player, npc, take, give, level));
            if !ok {
                self.pet_manager_page(npc, player, "data/html/petmanager/restore_no.htm");
            }
        }
    }

    /// `html.setFile(player, path); player.sendPacket(html)` of the pet manager.
    fn pet_manager_page(&mut self, npc: ObjectId, player: ObjectId, path: &str) {
        if let Some(html) = self.html_from_file(player, npc.0, path) {
            self.send_html(player, &html);
        }
    }

    /// Java `PetManager.exchange(player, itemIdtake, itemIdgive)`.
    fn pet_manager_exchange(&mut self, npc: ObjectId, player: ObjectId, take: i32, give: i32) {
        if self.destroy_item_by_item_id(player, take, 1, true) {
            self.add_item(player, give, 1, true);
            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
            self.pet_manager_page(npc, player, &format!("data/html/petmanager/{npc_id}.htm"));
        } else {
            self.pet_manager_page(npc, player, "data/html/petmanager/exchange_no.htm");
        }
    }

    /// Java `PetManager.doEvolve(player, npc, itemIdtake, itemIdgive, petminLevel)`: the living
    /// pet of the collar `take` (at least at level `petminLevel`) becomes the pet of the collar
    /// `give`, keeping its exp (never below the new pet's minimum), its name and its place.
    /// `false` is the "no" page.
    pub(crate) fn pet_do_evolve(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        take: i32,
        give: i32,
        pet_min_level: i32,
    ) -> bool {
        if take == 0 || give == 0 || pet_min_level == 0 {
            return false;
        }
        if !self.has_pet(player) {
            return false;
        }
        let Some(current) = self.summon_of(player) else {
            return false;
        };
        if self.is_alike_dead(current) {
            let text = format!("{} tried to use death pet exploit!", self.player_text(player));
            self.store_illegal_action(player, &text);
            return false;
        }
        let Some(table) = self.pet_table().cloned() else {
            return false;
        };
        let Some(old_npc_id) = table.pet_data_by_item_id(take).map(|d| d.npc_id) else {
            return false;
        };
        let current_level = self.pet_level_of(current);
        if current_level < pet_min_level || self.summon_npc_id(current) != Some(old_npc_id) {
            return false;
        }
        let Some(new_npc_id) = table.pet_data_by_item_id(give).map(|d| d.npc_id) else {
            return false;
        };
        if new_npc_id == 0 {
            return false;
        }
        let Some(control) = self.pet_control_object_id(current) else {
            return false;
        };
        let (mut pet_exp, old_name) = self
            .summon(current)
            .map_or((0, String::new()), |s| (s.exp, s.name.clone()));
        let Some((old_x, old_y, old_z)) = self.position_of(current) else {
            return false;
        };

        self.pet_unsummon(current, Some(player));
        // Deleting old pet item.
        self.pet_evolve_destroy_control_item(player, control);
        let Some(new_control) = self.pet_give_collar(player, give) else {
            return false;
        };

        // Summoning new pet.
        let Some(pet) = self.pet_spawn_pet(player, new_control) else {
            return false;
        };
        // Fix for non-linear baby pet exp.
        let minimum = self.pet_exp_for_level(pet, pet_min_level);
        if pet_exp < minimum {
            pet_exp = minimum;
        }
        self.pet_add_exp(pet, pet_exp);
        self.pet_fill_up(player, pet);
        if let Some(state) = self.summon_mut(pet) {
            // `petSummon.setName(oldname)`: an unnamed pet has no name (`null`).
            state.name = old_name.clone();
        }
        self.pet_sync_control_name_flag(player, new_control, !old_name.is_empty());
        self.summon_set_running(pet, true);
        self.pet_store_me(pet);
        self.set_pet(player, Some(pet));

        self.pet_summon_effects(player, npc);
        if !self.pet_spawn_for_evolve(player, pet, old_x, old_y, old_z) {
            return true;
        }
        let level = self.pet_level_of(pet);
        self.pet_set_control_enchant(player, new_control, level);
        self.pet_schedule_evolve_tasks(pet);
        true
    }

    /// Java `PetManager.doRestore(player, npc, itemIdtake, itemIdgive, petminLevel)`: the collar
    /// `take` of an evolved pet is turned back into the collar `give` of the lower form, which
    /// is summoned at the level the old collar showed (not below `petminLevel`).
    pub(crate) fn pet_do_restore(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        take: i32,
        give: i32,
        pet_min_level: i32,
    ) -> bool {
        if take == 0 || give == 0 || pet_min_level == 0 {
            return false;
        }
        let Some(item) = self.inventory_item_by_item_id(player, take).cloned() else {
            return false;
        };
        let Some(table) = self.pet_table().cloned() else {
            return false;
        };
        if table.pet_data_by_item_id(take).is_none() {
            return false;
        }
        let Some(new_npc_id) = table.pet_data_by_item_id(give).map(|d| d.npc_id) else {
            return false;
        };
        if new_npc_id == 0 {
            return false;
        }
        let old_level = if item.enchant_level < pet_min_level {
            pet_min_level
        } else {
            item.enchant_level
        };

        // Deleting old pet item.
        let templates = self.item_templates();
        let Some(removed) = self.destroy_inventory_item(player, item.object_id, item.count) else {
            return false;
        };
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S1HasDisappeared).with(crate::inventory::item_name_param(
                templates.as_deref(),
                removed.item_id,
            )),
        );
        // Give new pet item.
        let Some(new_control) = self.pet_give_collar(player, give) else {
            return false;
        };

        // Summoning new pet.
        let Some(pet) = self.pet_spawn_pet(player, new_control) else {
            return false;
        };
        let max_exp = self.pet_exp_for_level(pet, old_level);
        self.pet_add_exp(pet, max_exp);
        self.pet_fill_up(player, pet);
        self.summon_set_running(pet, true);
        self.pet_store_me(pet);
        self.set_pet(player, Some(pet));

        self.pet_summon_effects(player, npc);
        let Some((x, y, z)) = self.position_of(player) else {
            return false;
        };
        if !self.pet_spawn_for_evolve(player, pet, x, y, z) {
            return true;
        }
        let level = self.pet_level_of(pet);
        self.pet_set_control_enchant(player, new_control, level);

        // Inventory update.
        let entry = self.update_entry(&removed, InventoryUpdate::REMOVED);
        self.send_inventory_update(player, entry);
        self.send_cur_load(player);
        self.broadcast_user_info(player);
        self.pet_schedule_evolve_tasks(pet);

        // Pet control item no longer exists, delete the pet from the db.
        self.pet_row_deleted(removed.object_id);
        true
    }

    fn pet_level_of(&self, pet: ObjectId) -> i32 {
        self.npcs.get(&pet).map_or(1, |n| n.core.level)
    }

    /// `inventory.addItem(RESTORE, itemId, 1, player, npc)`: the new collar, written at once.
    fn pet_give_collar(&mut self, player: ObjectId, item_id: i32) -> Option<ObjectId> {
        let added = self.inventory_add_item_by_id(player, item_id, 1)?;
        let control = added.item.object_id;
        self.update_item_in_db(player, control);
        Some(control)
    }

    /// `setCurrentHp(getMaxHp())`, `setCurrentMp(getMaxMp())`, `setCurrentFed(getMaxFed())` and
    /// `setTitle(player.getName())`.
    fn pet_fill_up(&mut self, player: ObjectId, pet: ObjectId) {
        let (hp, mp) = self
            .stats(pet)
            .map_or((0.0, 0.0), |v| (f64::from(v.max_hp()), f64::from(v.max_mp())));
        self.set_current_hp(pet, hp, false);
        self.set_current_mp(pet, mp, false);
        let max_fed = self.pet_max_fed(pet);
        self.pet_set_current_fed(pet, max_fed);
        let title = self.players.get(&player).map(|p| p.data.name.clone());
        if let (Some(title), Some(state)) = (title, self.summon_mut(pet)) {
            state.title = title;
        }
    }

    /// `player.sendPacket(new MagicSkillUse(npc, 2046, 1, 1000, 600000))` and "Summoning your
    /// pet." (to the owner only).
    fn pet_summon_effects(&mut self, player: ObjectId, npc: ObjectId) {
        if let Some((x, y, z)) = self.position_of(npc) {
            let at = SkillPosition { x, y, z };
            self.send(
                player,
                &MagicSkillUse {
                    creature_id: npc.0,
                    target_id: npc.0,
                    skill_id: SUMMON_SKILL_ID,
                    skill_level: 1,
                    hit_time: 1000,
                    reuse_delay: 600_000,
                    creature: at,
                    ground_location: None,
                    target: at,
                },
            );
        }
        self.send(player, &SystemMessage::new(SystemMessageId::SummoningYourPet));
    }

    /// `petSummon.spawnMe(x, y, z)`, `startFeed()`: `false` when the pet could not enter the world.
    fn pet_spawn_for_evolve(&mut self, player: ObjectId, pet: ObjectId, x: i32, y: i32, z: i32) -> bool {
        self.spawn_at(pet, x, y, z);
        if !self.npcs.contains_key(&pet) {
            // Out of the world: `spawn` disposed of it.
            self.set_pet(player, None);
            self.summons.summons.remove(&pet);
            return false;
        }
        self.pet_start_feed(pet);
        true
    }

    /// `ThreadPool.schedule(new EvolveFinalizer(...), 900)` and the feed check.
    fn pet_schedule_evolve_tasks(&mut self, pet: ObjectId) {
        let due = self.tick + TimerWheel::ms_to_ticks(FINALIZER_DELAY_MS);
        self.timers
            .schedule_at(due, TimerKind::Summon(SummonTimer::EvolveFinalizer(pet)));
        if self.pet_state(pet).is_some_and(|p| p.cur_fed <= 0) {
            let due = self.tick + TimerWheel::ms_to_ticks(FEED_WAIT_DELAY_MS);
            self.timers
                .schedule_at(due, TimerKind::Summon(SummonTimer::EvolveFeedWait(pet)));
        } else {
            self.pet_start_feed(pet);
        }
    }

    /// `Pet.destroyControlItem(owner, true)`: like the destroy of a dead pet's collar, but the
    /// item is a fee (no "has disappeared" message).
    fn pet_evolve_destroy_control_item(&mut self, owner: ObjectId, control: ObjectId) {
        if self.inventory_item(owner, control).is_some() {
            if let Some(removed) = self.destroy_inventory_item(owner, control, 1) {
                let entry = self.update_entry(&removed, InventoryUpdate::REMOVED);
                self.send_inventory_update(owner, entry);
                self.send_cur_load(owner);
                self.broadcast_user_info(owner);
            }
        } else {
            warn!("Couldn't destroy pet control item for {owner} pet: {control} evolve: true");
        }
        // Pet control item no longer exists, delete the pet from the db.
        self.pet_row_deleted(control);
    }

    /// `EvolveFinalizer.run()`: the skill ends, the pet follows and its summon animation is over.
    pub(crate) fn on_pet_evolve_finalizer(&mut self, pet: ObjectId) {
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        self.send(
            owner,
            &MagicSkillLaunched {
                creature_id: owner.0,
                skill_id: SUMMON_SKILL_ID,
                skill_level: 1,
                target_ids: vec![owner.0],
            },
        );
        self.set_follow_status(pet, true);
        if let Some(state) = self.summon_mut(pet) {
            state.show_summon_animation = false;
        }
    }

    /// `EvolveFeedWait.run()`: a pet that is still starving a minute later goes away.
    pub(crate) fn on_pet_evolve_feed_wait(&mut self, pet: ObjectId) {
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        if self.pet_state(pet).is_some_and(|p| p.cur_fed <= 0) {
            self.pet_unsummon(pet, Some(owner));
        } else {
            self.pet_start_feed(pet);
        }
    }
}
