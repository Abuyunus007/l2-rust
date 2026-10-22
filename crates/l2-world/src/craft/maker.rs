//! `RecipeManager.RecipeItemMaker`: making an item from a recipe, for oneself or for a customer
//! of a manufacture shop.
// Java: managers/RecipeManager.java (requestManufactureItem, requestMakeItem, RecipeItemMaker)
// Java: network/clientpackets/RequestRecipeItemMakeSelf.java
// Java: entity/item/recipe/{RecipeItemInfo,ManufactureItem}.java
//
// # The two modes
// Without `AltGameCreation` (the default) a craft is one call: the checks of the constructor,
// then `run()` takes the MP or HP, the payment and the materials, rolls the chance and gives the
// product. Nothing is kept afterwards.
//
// With `AltGameCreation` the maker is registered in `_activeMakers` and takes the materials in
// passes (`itemGrab` per pass, the crafting skill level times the `GIM` modifier), one pass per
// `AltGameCreationSpeed * mReuseRate` seconds, showing the skill animation and the gauge, and
// gives exp and SP at the end. The passes are timers on the wheel ([`CraftTimer`]); Java's
// `Thread.sleep(delay)` before the last step is a timer of `delay` as well.
//
// # Quirks kept
// - A successful craft sends `RecipeItemMakeInfo` twice (`rewardPlayer` and `finishCrafting`).
// - `finishCrafting` ignores the result of `calculateStatUse` in the normal mode, so a crafter
//   without the MP the recipe wants (it dropped since the constructor) still gets the item
//   after the "not enough MP" message.
// - `Rnd.get(100)` is drawn on every success while `CraftMasterwork` is on, also for a recipe
//   without a rare product (the rare item id of such a recipe is 0, not -1).
// - Only the alt mode checks `isProcessingTransaction` again between passes: it does not.

use l2_data::RecipeList;
use l2_data::recipe::StatType;
use l2_data::stats::Stat;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{
    ActionFailed, InventoryUpdate, MagicSkillUse, SetupGauge, SkillPosition, StatusUpdate, SystemMessage,
    SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::ManufactureItem;
use crate::enter::{item_list, quest_item_list};
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};
use l2_data::item::ADENA_ID;

/// Java `GameTimeTaskManager.TICKS_PER_SECOND * MILLIS_IN_TICK`: the factor of the pass delay.
const PASS_DELAY_FACTOR: f64 = 10.0 * 100.0;

/// The timers of the alt crafting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftTimer {
    /// `ThreadPool.schedule(maker, ..)`: `RecipeItemMaker.run()` of the maker with this serial.
    Run(u64),
    /// The end of `Thread.sleep(_delay)` before `finishCrafting()`.
    Finish(u64),
}

/// Java `RecipeItemInfo`: a material as it is counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecipeItemInfo {
    item_id: i32,
    quantity: i32,
    name: String,
}

/// Java `RecipeItemMaker`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ItemMaker {
    recipe: RecipeList,
    /// Java `_player`: the crafter.
    crafter: ObjectId,
    /// Java `_target`: the customer (the crafter itself when it crafts for itself).
    target: ObjectId,
    skill_id: i32,
    skill_level: i32,
    creation_passes: i32,
    item_grab: i32,
    exp: i32,
    sp: i32,
    price: i64,
    total_items: i32,
    delay: i32,
    /// Java `_items`: `None` until the materials were listed.
    items: Option<Vec<RecipeItemInfo>>,
    valid: bool,
    /// The key of a maker kept on the timer wheel (0 for one that is not).
    serial: u64,
    /// A timer for this maker was set during the current step.
    scheduled: bool,
}

impl World {
    // ------------------------------------------------------------------ the requests

    /// Java `RecipeData.getValidRecipeList(player, id)`.
    fn get_valid_recipe_list(&mut self, player: ObjectId, id: i32) -> Option<RecipeList> {
        let recipe = self
            .recipe_data()
            .and_then(|d| d.get(id).cloned())
            .filter(|r| !r.recipes.is_empty());
        if recipe.is_none() {
            self.send_message(player, &format!("RecipeData: No recipe for: {id}"));
            self.set_crafting(player, false);
        }
        recipe
    }

    /// `PunishmentManager.handleIllegalPlayerAction(player, "Warning!! Character .. sent a false
    /// recipe id.")`.
    fn false_recipe_id(&mut self, player: ObjectId) {
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let text = format!(
            "Warning!! Character {name} of account {} sent a false recipe id.",
            self.account_of(player)
        );
        self.store_illegal_action(player, &text);
    }

    /// Java `RecipeManager.requestManufactureItem(manufacturer, recipeListId, player)`.
    pub(crate) fn request_manufacture_item(
        &mut self,
        manufacturer: ObjectId,
        recipe_list_id: i32,
        player: ObjectId,
    ) {
        let Some(recipe) = self.get_valid_recipe_list(player, recipe_list_id) else {
            return;
        };
        if !self.has_recipe_list(manufacturer, recipe.id) {
            self.false_recipe_id(player);
            return;
        }
        // Check if manufacturer is under manufacturing store or private store.
        if self.settings.craft.alt_game_creation && self.craft.active.contains_key(&manufacturer) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::PleaseCloseTheSetupWindowForYourPrivateManufacturingStoreOrPrivateStoreAndTryAgain,
                ),
            );
            return;
        }
        self.start_maker(manufacturer, recipe, player);
    }

    /// Java `RecipeManager.requestMakeItem(player, recipeListId)`.
    pub(crate) fn request_make_item(&mut self, player: ObjectId, recipe_list_id: i32) {
        // Check if player is trying to operate a private store or private workshop while engaged
        // in combat.
        if (self.is_in_combat(player) && !self.settings.craft.crafting_in_combat) || self.is_in_duel(player) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::WhileYouAreEngagedInCombatYouCannotOperateAPrivateStoreOrPrivateWorkshop,
                ),
            );
            return;
        }
        let Some(recipe) = self.get_valid_recipe_list(player, recipe_list_id) else {
            return;
        };
        if !self.has_recipe_list(player, recipe.id) {
            self.false_recipe_id(player);
            return;
        }
        // Check if player is busy (possible if alt game creation is enabled).
        if self.settings.craft.alt_game_creation && self.craft.active.contains_key(&player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S2S1)
                    .with(self.recipe_product_name(recipe.item_id))
                    .with(SystemMessageParam::Text("You are busy creating.".to_string())),
            );
            return;
        }
        self.start_maker(player, recipe, player);
    }

    /// Java `RequestRecipeItemMakeSelf.runImpl`.
    // The flood protector `canManufacture` is not modelled.
    pub(crate) fn on_request_recipe_item_make_self(&mut self, id: ObjectId, recipe_id: i32) {
        if self.is_in_store_mode(id) {
            self.send_message(id, "You cannot create items while trading.");
            return;
        }
        if self.is_crafting(id) {
            self.send_message(id, "You are currently in Craft Mode.");
            return;
        }
        self.request_make_item(id, recipe_id);
    }

    /// The end of both request methods: `new RecipeItemMaker(..)`, and when it is valid either
    /// `_activeMakers.put` and a 100 ms timer (alt) or `maker.run()`.
    fn start_maker(&mut self, crafter: ObjectId, recipe: RecipeList, target: ObjectId) {
        let mut maker = self.new_item_maker(crafter, recipe, target);
        if !maker.valid {
            return;
        }
        if self.settings.craft.alt_game_creation {
            self.craft.next_serial += 1;
            maker.serial = self.craft.next_serial;
            self.craft.active.insert(crafter, maker.serial);
            self.schedule_maker(&mut maker, 100, false);
            self.craft.makers.insert(maker.serial, maker);
        } else {
            self.maker_run(&mut maker);
        }
    }

    /// The name of the product item as a message parameter.
    fn recipe_product_name(&self, item_id: i32) -> SystemMessageParam {
        item_name_param(self.item_templates().as_deref(), item_id)
    }

    // ------------------------------------------------------------------ the constructor

    /// Java `new RecipeItemMaker(player, recipeList, target)`.
    fn new_item_maker(&mut self, crafter: ObjectId, recipe: RecipeList, target: ObjectId) -> ItemMaker {
        let skill_id = if recipe.is_dwarven {
            super::CREATE_DWARVEN
        } else {
            super::CREATE_COMMON
        };
        let skill_level = self.skill_level_of(crafter, skill_id);
        let mut m = ItemMaker {
            recipe,
            crafter,
            target,
            skill_id,
            skill_level,
            creation_passes: 1,
            item_grab: 0,
            exp: -1,
            sp: -1,
            price: 0,
            total_items: 0,
            delay: 0,
            items: None,
            valid: false,
            serial: 0,
            scheduled: false,
        };
        self.set_crafting(crafter, true);

        if self.is_alike_dead(crafter) {
            self.send(crafter, &ActionFailed);
            self.maker_abort(&m);
            return m;
        }
        if self.is_alike_dead(target) {
            self.send(target, &ActionFailed);
            self.maker_abort(&m);
            return m;
        }
        if self.is_processing_transaction(target) {
            self.send(target, &ActionFailed);
            self.maker_abort(&m);
            return m;
        }
        if self.is_processing_transaction(crafter) {
            self.send(crafter, &ActionFailed);
            self.maker_abort(&m);
            return m;
        }

        // Validate recipe list.
        if m.recipe.recipes.is_empty() {
            self.send(crafter, &ActionFailed);
            self.maker_abort(&m);
            return m;
        }

        // Validate skill level.
        if m.recipe.level > m.skill_level {
            self.send(crafter, &ActionFailed);
            self.maker_abort(&m);
            return m;
        }

        // Check that customer can afford to pay for creation services.
        if crafter != target {
            let cost = self
                .players
                .get(&crafter)
                .and_then(|p| p.craft.manufacture.iter().find(|i| i.recipe_id == m.recipe.id))
                .map(|i: &ManufactureItem| i.cost);
            if let Some(cost) = cost {
                m.price = cost;
                if self.adena_of(target) < m.price {
                    self.send(
                        target,
                        &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                    );
                    self.maker_abort(&m);
                    return m;
                }
            }
        }

        // Make temporary items.
        let Some(items) = self.maker_list_items(&m, false) else {
            return m;
        };
        m.total_items = items.iter().fold(0_i32, |sum, i| sum.wrapping_add(i.quantity));
        m.items = Some(items);

        // Initial statUse checks.
        if !self.calculate_stat_use(&mut m, false, false) {
            self.maker_abort(&m);
            return m;
        }

        // Initial AltStatChange checks.
        if self.settings.craft.alt_game_creation {
            Self::calculate_alt_stat_change(&mut m);
        }

        self.maker_update_make_info(&m, true);
        self.maker_update_cur_mp(&m);
        self.send_cur_load(m.target);

        self.set_crafting(crafter, false);
        m.valid = true;
        m
    }

    // ------------------------------------------------------------------ run

    /// Java `RecipeItemMaker.run()`.
    fn maker_run(&mut self, m: &mut ItemMaker) {
        if !self.settings.craft.is_crafting_enabled {
            self.send_message(m.target, "Item creation is currently disabled.");
            self.maker_abort(m);
            return;
        }
        // `_player == null || _target == null`: Java keeps the objects of players who left; here
        // they are gone, so there is nothing to craft for.
        if !self.players.contains_key(&m.crafter) || !self.players.contains_key(&m.target) {
            warn!(
                "player or target == null (disconnected?), aborting {:?} {:?}",
                m.target, m.crafter
            );
            self.maker_abort(m);
            return;
        }

        let alt = self.settings.craft.alt_game_creation;
        if alt && !self.craft.active.contains_key(&m.crafter) {
            if m.target != m.crafter {
                self.send_message(m.target, "Manufacture aborted");
                self.send_message(m.crafter, "Manufacture aborted");
            } else {
                self.send_message(m.crafter, "Item creation aborted");
            }
            self.maker_abort(m);
            return;
        }

        if alt && m.items.as_ref().is_some_and(|i| !i.is_empty()) {
            if !self.calculate_stat_use(m, true, true) {
                return; // check stat use
            }
            self.maker_update_cur_mp(m); // Update craft window mp bar.
            self.grab_some_items(m); // Grab (equip) some more items with a nice msg to player.

            // If still not empty, schedule another pass.
            if m.items.as_ref().is_some_and(|i| !i.is_empty()) {
                m.delay = (self.settings.craft.alt_game_creation_speed
                    * self.m_reuse_rate(m.crafter, m.skill_id, m.skill_level)
                    * PASS_DELAY_FACTOR) as i32;
                // FIXME (Java): the packet should show a crafting animation.
                self.broadcast_craft_skill(m.crafter, m.skill_id, m.skill_level, m.delay);
                self.send(
                    m.crafter,
                    &SetupGauge::new(m.crafter.0, SetupGauge::BLUE, m.delay),
                );
                self.schedule_maker(m, 100 + i64::from(m.delay), false);
            } else {
                // For alt mode, sleep delay msec before finishing.
                self.send(
                    m.crafter,
                    &SetupGauge::new(m.crafter.0, SetupGauge::BLUE, m.delay),
                );
                self.schedule_maker(m, i64::from(m.delay), true);
            }
        } else {
            // For old craft mode just finish.
            self.maker_finish(m);
        }
    }

    /// The timers of the alt crafting.
    pub(crate) fn on_craft_timer(&mut self, timer: CraftTimer) {
        match timer {
            CraftTimer::Run(serial) => {
                let Some(mut maker) = self.craft.makers.remove(&serial) else {
                    return;
                };
                maker.scheduled = false;
                self.maker_run(&mut maker);
                if maker.scheduled {
                    self.craft.makers.insert(serial, maker);
                }
            }
            CraftTimer::Finish(serial) => {
                let Some(mut maker) = self.craft.makers.remove(&serial) else {
                    return;
                };
                maker.scheduled = false;
                self.maker_finish(&mut maker);
            }
        }
    }

    fn schedule_maker(&mut self, m: &mut ItemMaker, delay_ms: i64, finish: bool) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let timer = if finish {
            CraftTimer::Finish(m.serial)
        } else {
            CraftTimer::Run(m.serial)
        };
        self.timers.schedule_at(due, TimerKind::Craft(timer));
        m.scheduled = true;
    }

    /// Java `Creature.getMReuseRate(skill)`: `calcStat(MAGIC_REUSE_RATE, 1, null, skill)`.
    fn m_reuse_rate(&self, id: ObjectId, skill_id: i32, skill_level: i32) -> f64 {
        let skill = self
            .data
            .stat_data()
            .and_then(|d| d.skills.get_skill(skill_id, skill_level));
        let ctx = skill.map(crate::stats::skills::calc_skill);
        self.stats(id).map_or(1.0, |v| {
            v.calc_stat(Stat::MagicReuseRate, 1.0, None, ctx.as_ref())
        })
    }

    /// `broadcastSkillPacket(new MagicSkillUse(player, skillId, skillLevel, delay, 0), player)`.
    fn broadcast_craft_skill(&self, id: ObjectId, skill_id: i32, skill_level: i32, delay: i32) {
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let at = SkillPosition { x, y, z };
        let ground = self
            .cast
            .creatures
            .get(&id)
            .and_then(|s| s.current_skill_world_position)
            .map(|(x, y, z)| SkillPosition { x, y, z });
        self.broadcast_creature_packet(
            id,
            &MagicSkillUse {
                creature_id: id.0,
                target_id: id.0,
                skill_id,
                skill_level,
                hit_time: delay,
                reuse_delay: 0,
                creature: at,
                ground_location: ground,
                target: at,
            },
        );
    }

    // ------------------------------------------------------------------ finish

    /// Java `RecipeItemMaker.finishCrafting()`.
    fn maker_finish(&mut self, m: &mut ItemMaker) {
        if !self.settings.craft.alt_game_creation {
            // NOTE(java-quirk): the result is ignored (see the module notes).
            self.calculate_stat_use(m, false, true);
        }

        // First take adena for manufacture.
        // Customer must pay for services.
        if m.target != m.crafter && m.price > 0 && !self.craft_transfer_adena(m.target, m.crafter, m.price) {
            self.send(
                m.target,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
            );
            self.maker_abort(m);
            return;
        }

        // This line actually takes materials from inventory.
        m.items = self.maker_list_items(m, true);
        if m.items.is_some() {
            let success = self.rng().below(100) < m.recipe.success_rate;
            if success {
                self.reward_player(m);
            } else if m.target != m.crafter {
                let target_name = self.name_param(m.target);
                let crafter_name = self.name_param(m.crafter);
                self.send(
                    m.crafter,
                    &SystemMessage::new(
                        SystemMessageId::YourAttemptToCreateS2ForC1AtThePriceOfS3AdenaHasFailed,
                    )
                    .with(target_name)
                    .with(self.recipe_product_name(m.recipe.item_id))
                    .with(SystemMessageParam::LongNumber(m.price)),
                );
                self.send(
                    m.target,
                    &SystemMessage::new(SystemMessageId::C1HasFailedToCreateS2AtThePriceOfS3Adena)
                        .with(crafter_name)
                        .with(self.recipe_product_name(m.recipe.item_id))
                        .with(SystemMessageParam::LongNumber(m.price)),
                );
            } else {
                self.send(
                    m.target,
                    &SystemMessage::new(SystemMessageId::YouFailedAtMixingTheItem),
                );
            }

            // Craft window counts come from the client inventory, so it is sent directly,
            // sendItemList is delayed. The pending item added update goes first, otherwise it
            // arrives after the list and duplicates the item.
            self.flush_inventory_update(m.target);
            self.send_item_list_now(m.target);
            self.maker_update_make_info(m, success);
        }

        // Update load and mana bar of craft window.
        self.maker_update_cur_mp(m);
        self.send_cur_load(m.target);
        self.craft.active.remove(&m.crafter);
        self.set_crafting(m.crafter, false);
    }

    /// `player.sendPacket(new ItemList(player, false))`: the list now, not 250 ms later, and the
    /// quest items after it (`ItemList.runImpl`).
    fn send_item_list_now(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let now_ms = unix_ms_now();
        let mut list = item_list(&player.data, &*self.data, now_ms);
        list.show_window = false;
        list.block = self.inventory_block_packet(id);
        let mut quest = quest_item_list(&player.data, &*self.data, now_ms);
        quest.block = self.inventory_block_packet(id);
        self.send(id, &list);
        self.send(id, &quest);
    }

    /// `new SystemMessage(..).addString(player.getName())`.
    fn name_param(&self, id: ObjectId) -> SystemMessageParam {
        SystemMessageParam::Text(
            self.players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default(),
        )
    }

    /// Java `Player.transferItem(TRANSFER, adenaObjectId, price, crafter.getInventory(), crafter)`
    /// with the packets `Player.transferItem` sends to both sides. `false` where Java returns
    /// null.
    fn craft_transfer_adena(&mut self, from: ObjectId, to: ObjectId, amount: i64) -> bool {
        // `_target.getInventory().getAdenaInstance().getObjectId()`
        let Some(adena) = self
            .inventory_item_by_item_id(from, ADENA_ID)
            .map(|i| i.object_id)
        else {
            return false;
        };
        if self.check_item_manipulation(from, adena, amount).is_none() {
            return false;
        }
        let Some(moved) = self.transfer_item(from, adena, amount, to, from) else {
            return false;
        };
        // Send inventory update packet.
        let change = if moved.old.count > 0 && !moved.moved_whole {
            InventoryUpdate::MODIFIED
        } else {
            InventoryUpdate::REMOVED
        };
        let entry = self.update_entry(&moved.old, change);
        self.send_inventory_update(from, entry);
        // Update current load as well.
        self.send_cur_load(from);
        // Send target update packet.
        let change = if moved.new.count > amount {
            InventoryUpdate::MODIFIED
        } else {
            InventoryUpdate::ADDED
        };
        let entry = self.update_entry(&moved.new, change);
        self.send_inventory_update(to, entry);
        self.send_cur_load(to);
        true
    }

    /// Java `RecipeItemMaker.rewardPlayer()`.
    fn reward_player(&mut self, m: &mut ItemMaker) {
        let rare_prod_id = m.recipe.rare_item_id;
        let mut item_id = m.recipe.item_id;
        let mut item_count = m.recipe.count;
        let price = self
            .item_templates()
            .and_then(|t| t.get(item_id).map(|t| t.price))
            .unwrap_or(0);

        // Check that the current recipe has a rare production or not.
        // NOTE(java-quirk): a recipe without a rare product has rare item id 0, not -1, so this
        // holds and the dice are rolled (see the module notes).
        if rare_prod_id != -1
            && (rare_prod_id == item_id || self.settings.craft.craft_masterwork)
            && f64::from(self.rng().below(100))
                < f64::from(m.recipe.rarity) * self.settings.craft.craft_masterwork_chance_rate
        {
            item_id = rare_prod_id;
            item_count = m.recipe.rare_count;
        }

        self.inventory_add_item_by_id(m.target, item_id, i64::from(item_count));

        // Inform customer of earned item.
        if m.target != m.crafter {
            let target_name = self.name_param(m.target);
            let crafter_name = self.name_param(m.crafter);
            let product = self.recipe_product_name(item_id);
            // Inform manufacturer of earned profit.
            if item_count == 1 {
                self.send(
                    m.crafter,
                    &SystemMessage::new(
                        SystemMessageId::S2HasBeenCreatedForC1AfterThePaymentOfS3AdenaWasReceived,
                    )
                    .with(target_name)
                    .with(product.clone())
                    .with(SystemMessageParam::LongNumber(m.price)),
                );
                self.send(
                    m.target,
                    &SystemMessage::new(SystemMessageId::C1CreatedS2AfterReceivingS3Adena)
                        .with(crafter_name)
                        .with(product)
                        .with(SystemMessageParam::LongNumber(m.price)),
                );
            } else {
                self.send(
                    m.crafter,
                    &SystemMessage::new(SystemMessageId::S2S3HaveBeenCreatedForC1AtThePriceOfS4Adena)
                        .with(target_name)
                        .with(SystemMessageParam::IntNumber(item_count))
                        .with(product.clone())
                        .with(SystemMessageParam::LongNumber(m.price)),
                );
                self.send(
                    m.target,
                    &SystemMessage::new(SystemMessageId::C1CreatedS2S3AtThePriceOfS4Adena)
                        .with(crafter_name)
                        .with(SystemMessageParam::IntNumber(item_count))
                        .with(product)
                        .with(SystemMessageParam::LongNumber(m.price)),
                );
            }
        }

        let product = self.recipe_product_name(item_id);
        let earned = if item_count > 1 {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                .with(product)
                .with(SystemMessageParam::LongNumber(i64::from(item_count)))
        } else {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS1).with(product)
        };
        self.send(m.target, &earned);

        if self.settings.craft.alt_game_creation {
            self.alt_creation_exp_sp(m, price, item_id, item_count);
        }

        self.maker_update_make_info(m, true); // success
    }

    /// The `ALT_GAME_CREATION` end of `rewardPlayer`: exp and SP for the crafter.
    fn alt_creation_exp_sp(
        &mut self,
        m: &mut ItemMaker,
        reference_price: i32,
        item_id: i32,
        item_count: i32,
    ) {
        let settings = self.settings.craft.clone();
        let recipe_level = m.recipe.level;
        if m.exp < 0 {
            // `_exp = template.getReferencePrice() * itemCount; _exp /= recipeLevel;` (int math)
            m.exp = reference_price.wrapping_mul(item_count);
            // A recipe level of 0 throws in Java (an `ArithmeticException` in the maker's thread).
            m.exp = m.exp.checked_div(recipe_level).unwrap_or(0);
        }
        if m.sp < 0 {
            m.sp = m.exp / 10;
        }
        if item_id == m.recipe.rare_item_id {
            // `int *= double`: the product is cut back to an int.
            m.exp = (f64::from(m.exp) * settings.alt_game_creation_rare_xpsp_rate) as i32;
            m.sp = (f64::from(m.sp) * settings.alt_game_creation_rare_xpsp_rate) as i32;
        }
        if m.exp < 0 {
            m.exp = 0;
        }
        if m.sp < 0 {
            m.sp = 0;
        }
        let mut level = m.skill_level;
        while level > recipe_level {
            m.exp /= 4;
            m.sp /= 4;
            level -= 1;
        }

        // Added multiplication of Creation speed with XP/SP gain slower crafting -> more XP,
        // faster crafting -> less XP you can use ALT_GAME_CREATION_XP_RATE/SP to modify XP/SP
        // gained (default = 1).
        let exp_rate = |world: &World, init: f64| {
            world
                .stats(m.crafter)
                .map_or(init, |v| v.calc_stat(Stat::ExpspRate, init, None, None))
        };
        let exp = exp_rate(
            self,
            f64::from(m.exp) * settings.alt_game_creation_xp_rate * settings.alt_game_creation_speed,
        ) as i32;
        let sp = exp_rate(
            self,
            f64::from(m.sp) * settings.alt_game_creation_sp_rate * settings.alt_game_creation_speed,
        ) as i32;
        self.add_exp_and_sp(m.crafter, f64::from(exp), f64::from(sp), false);
    }

    // ------------------------------------------------------------------ the parts

    /// Java `RecipeItemMaker.abort()`.
    fn maker_abort(&mut self, m: &ItemMaker) {
        self.maker_update_make_info(m, false);
        self.set_crafting(m.crafter, false);
        self.craft.active.remove(&m.crafter);
    }

    /// Java `RecipeItemMaker.updateMakeInfo(success)`.
    fn maker_update_make_info(&self, m: &ItemMaker, success: bool) {
        if m.target == m.crafter {
            self.send_recipe_item_make_info(m.target, m.recipe.id, success);
        } else if let Some(info) = self.recipe_shop_item_info(m.crafter, m.recipe.id) {
            self.send(m.target, &info);
        }
    }

    /// Java `RecipeItemMaker.updateCurMp()`.
    fn maker_update_cur_mp(&self, m: &ItemMaker) {
        let Some(p) = self.players.get(&m.target) else {
            return;
        };
        let mut update = StatusUpdate::new(m.target.0);
        update.add_attribute(status::CUR_MP, p.core.cur_mp as i32);
        self.send(m.target, &update);
    }

    /// Java `RecipeItemMaker.calculateAltStatChange()`.
    fn calculate_alt_stat_change(m: &mut ItemMaker) {
        m.item_grab = m.skill_level;
        for change in &m.recipe.alt_stat_change {
            match change.kind {
                StatType::Xp => m.exp = change.value,
                StatType::Sp => m.sp = change.value,
                StatType::Gim => m.item_grab = m.item_grab.wrapping_mul(change.value),
                StatType::Hp | StatType::Mp => {}
            }
        }

        // Determine number of creation passes needed.
        // (`/ 0` throws in Java; a grab of 0 cannot craft anything, so it is one pass.)
        m.creation_passes = m
            .total_items
            .checked_div(m.item_grab)
            .map_or(1, |passes| passes + i32::from(m.total_items % m.item_grab != 0));
        if m.creation_passes < 1 {
            m.creation_passes = 1;
        }
    }

    /// Java `RecipeItemMaker.calculateStatUse(isWait, isReduce)`.
    fn calculate_stat_use(&mut self, m: &mut ItemMaker, is_wait: bool, is_reduce: bool) -> bool {
        let mut ret = true;
        let alt = self.settings.craft.alt_game_creation;
        let stat_use = m.recipe.stat_use.clone();
        for stat in stat_use {
            let modified_value = f64::from(stat.value.checked_div(m.creation_passes).unwrap_or(0));
            match stat.kind {
                StatType::Hp => {
                    // We do not want to kill the player, so its CurrentHP must be greater than the
                    // reduce value.
                    let cur_hp = self.players.get(&m.crafter).map_or(0.0, |p| p.core.cur_hp);
                    if cur_hp <= modified_value {
                        // Rest (wait for HP).
                        if alt && is_wait {
                            self.send(
                                m.crafter,
                                &SetupGauge::new(m.crafter.0, SetupGauge::BLUE, m.delay),
                            );
                            self.schedule_maker(m, 100 + i64::from(m.delay), false);
                        } else {
                            self.send(m.target, &SystemMessage::new(SystemMessageId::NotEnoughHp));
                            self.maker_abort(m);
                        }
                        ret = false;
                    } else if is_reduce {
                        self.reduce_current_hp(m.crafter, modified_value, Some(m.crafter));
                    }
                }
                StatType::Mp => {
                    let cur_mp = self.players.get(&m.crafter).map_or(0.0, |p| p.core.cur_mp);
                    if cur_mp < modified_value {
                        // Rest (wait for MP).
                        if alt && is_wait {
                            self.send(
                                m.crafter,
                                &SetupGauge::new(m.crafter.0, SetupGauge::BLUE, m.delay),
                            );
                            self.schedule_maker(m, 100 + i64::from(m.delay), false);
                        } else {
                            self.send(m.target, &SystemMessage::new(SystemMessageId::NotEnoughMp));
                            self.maker_abort(m);
                        }
                        ret = false;
                    } else if is_reduce {
                        self.reduce_mp(m.crafter, modified_value);
                    }
                }
                _ => {
                    // There is an unknown StatUse value.
                    self.send_message(m.target, "Recipe error!!!, please tell this to your GM.");
                    ret = false;
                    self.maker_abort(m);
                }
            }
        }
        ret
    }

    /// Java `RecipeItemMaker.listItems(remove)`: counts the materials in the customer's
    /// inventory (`None` and the abort when one is missing) and, with `remove`, takes them.
    fn maker_list_items(&mut self, m: &ItemMaker, remove: bool) -> Option<Vec<RecipeItemInfo>> {
        let templates = self.item_templates();
        let mut materials: Vec<RecipeItemInfo> = Vec::new();
        for holder in &m.recipe.recipes {
            if holder.quantity > 0 {
                let amount = self
                    .inventory_item_by_item_id(m.target, holder.item_id)
                    .map_or(0, |i| i.count);
                // Check materials.
                if amount < i64::from(holder.quantity) {
                    self.send(
                        m.target,
                        &SystemMessage::new(SystemMessageId::YouAreMissingS2S1RequiredToCreateThat)
                            .with(item_name_param(templates.as_deref(), holder.item_id))
                            .with(SystemMessageParam::LongNumber(
                                i64::from(holder.quantity) - amount,
                            )),
                    );
                    self.maker_abort(m);
                    return None;
                }
                // Make new temporary object, just for counting purposes.
                materials.push(RecipeItemInfo {
                    item_id: holder.item_id,
                    quantity: holder.quantity,
                    name: templates
                        .as_deref()
                        .and_then(|t| t.get(holder.item_id))
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                });
            }
        }

        if remove {
            for tmp in &materials {
                self.inventory_destroy_by_item_id(m.target, tmp.item_id, i64::from(tmp.quantity));
                let name = item_name_param(templates.as_deref(), tmp.item_id);
                let message = if tmp.quantity > 1 {
                    SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                        .with(name)
                        .with(SystemMessageParam::LongNumber(i64::from(tmp.quantity)))
                } else {
                    SystemMessage::new(SystemMessageId::S1HasDisappeared).with(name)
                };
                self.send(m.target, &message);
            }
        }
        Some(materials)
    }

    /// Java `PlayerInventory.destroyItemByItemId(process, itemId, count, actor, reference)`: the
    /// container operation, with no packets (the craft sends the whole item list at the end).
    fn inventory_destroy_by_item_id(&mut self, player: ObjectId, item_id: i32, count: i64) -> bool {
        let candidates: Vec<(ObjectId, bool)> = self
            .players
            .get(&player)
            .map(|p| {
                p.data
                    .items
                    .iter()
                    .filter(|i| i.item_id == item_id && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL))
                    .map(|i| (i.object_id, i.loc == LOC_PAPERDOLL))
                    .collect()
            })
            .unwrap_or_default();
        // Attempt to find non equipped items; with nothing but worn ones the last is taken.
        let Some(&(chosen, _)) = candidates
            .iter()
            .find(|(_, equipped)| !equipped)
            .or_else(|| candidates.last())
        else {
            return false;
        };
        // Support destroying multiple non stackable items.
        if !self.data.item_stackable(item_id) && count > 1 {
            let free: Vec<ObjectId> = candidates
                .iter()
                .filter(|(_, equipped)| !equipped)
                .map(|(object_id, _)| *object_id)
                .collect();
            if i64::try_from(free.len()).unwrap_or(i64::MAX) < count {
                return false;
            }
            for object_id in free.into_iter().take(usize::try_from(count).unwrap_or(0)) {
                self.destroy_inventory_item(player, object_id, 1);
            }
            return true;
        }
        // Single item or stackable.
        self.destroy_inventory_item(player, chosen, count).is_some()
    }

    /// Java `RecipeItemMaker.grabSomeItems()`.
    fn grab_some_items(&mut self, m: &mut ItemMaker) {
        let mut grab_items = m.item_grab;
        while grab_items > 0 && m.items.as_ref().is_some_and(|i| !i.is_empty()) {
            let Some(items) = m.items.as_mut() else { break };
            let item = &mut items[0];
            let count = if item.quantity >= grab_items {
                grab_items
            } else {
                item.quantity
            };
            item.quantity -= count;
            let (item_id, name) = (item.item_id, item.name.clone());
            if item.quantity <= 0 {
                items.remove(0);
            }

            grab_items -= count;
            if m.target == m.crafter {
                let templates = self.item_templates();
                self.send(
                    m.crafter,
                    &SystemMessage::new(SystemMessageId::EquippedS1S2)
                        .with(SystemMessageParam::LongNumber(i64::from(count)))
                        .with(item_name_param(templates.as_deref(), item_id)),
                );
            } else {
                let crafter_name = self
                    .players
                    .get(&m.crafter)
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default();
                self.send_message(
                    m.target,
                    &format!("Manufacturer {crafter_name} used {count} {name}"),
                );
            }
        }
    }
}
