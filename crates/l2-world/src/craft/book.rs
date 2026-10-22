//! The recipe books of a player: registering and forgetting recipes, the book window, the
//! `Recipes` item handler and the effects that open the book.
// Java: entity/actor/Player.java (registerCommonRecipeList, registerDwarvenRecipeList,
//       hasRecipeList, unregisterRecipeList, insertNewRecipeData, deleteRecipeData,
//       restoreRecipeBook, hasDwarvenCraft, getDwarfRecipeLimit, ...)
// Java: managers/RecipeManager.java (requestBookOpen)
// Java: network/clientpackets/{RequestRecipeBookOpen,RequestRecipeBookDestroy,
//       RequestRecipeItemMakeInfo}.java
// Java: dist/game/data/scripts/handlers/items/Recipes.java
// Java: dist/game/data/scripts/handlers/skill/effects/{OpenDwarfRecipeBook,OpenCommonRecipeBook}.java

use l2_data::stats::Stat;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{RecipeBookItemList, RecipeItemMakeInfo, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{CREATE_COMMON, CREATE_DWARVEN, RecipeWrite};
use crate::inventory::{item_name_param, java_concurrent_map_order};
use crate::outbox::PersistCommand;
use crate::player::ItemData;
use crate::world::World;

/// Java `ShortcutType.RECIPE.ordinal()` (NONE, ITEM, SKILL, ACTION, MACRO, RECIPE, BOOKMARK).
const SHORTCUT_TYPE_RECIPE: i32 = 5;

impl World {
    // ------------------------------------------------------------------ the books

    /// Java `Player.getDwarvenRecipeBook()`: the recipe list ids in the order the map iterates.
    pub fn dwarven_recipe_book(&self, id: ObjectId) -> Vec<i32> {
        self.players
            .get(&id)
            .map(|p| java_concurrent_map_order(&p.craft.dwarven))
            .unwrap_or_default()
    }

    /// Java `Player.getCommonRecipeBook()`.
    pub fn common_recipe_book(&self, id: ObjectId) -> Vec<i32> {
        self.players
            .get(&id)
            .map(|p| java_concurrent_map_order(&p.craft.common))
            .unwrap_or_default()
    }

    /// Java `Player.hasRecipeList(recipeId)`.
    pub fn has_recipe_list(&self, id: ObjectId, recipe_id: i32) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.craft.dwarven.contains(&recipe_id) || p.craft.common.contains(&recipe_id))
    }

    /// Java `Player.getSkillLevel(skillId)`: 0 when the skill is not known.
    pub(super) fn skill_level_of(&self, id: ObjectId, skill_id: i32) -> i32 {
        self.known_skill(id, skill_id).map_or(0, |key| key.level)
    }

    /// Java `Player.getDwarvenCraft()`.
    pub(crate) fn dwarven_craft(&self, id: ObjectId) -> i32 {
        self.skill_level_of(id, CREATE_DWARVEN)
    }

    /// Java `Player.getCommonCraft()`.
    pub(crate) fn common_craft(&self, id: ObjectId) -> i32 {
        self.skill_level_of(id, CREATE_COMMON)
    }

    /// Java `Player.hasDwarvenCraft()`.
    pub(crate) fn has_dwarven_craft(&self, id: ObjectId) -> bool {
        self.dwarven_craft(id) >= 1
    }

    /// Java `Player.hasCommonCraft()`.
    pub(crate) fn has_common_craft(&self, id: ObjectId) -> bool {
        self.common_craft(id) >= 1
    }

    /// Java `Player.getDwarfRecipeLimit()`: `DwarfRecipeLimit` plus `REC_D_LIM`.
    pub(crate) fn dwarf_recipe_limit(&self, id: ObjectId) -> i32 {
        let bonus = self.stats(id).map_or(0.0, |v| v.calc(Stat::RecDLim, 0.0));
        self.settings.dwarf_recipe_limit.wrapping_add(bonus as i32)
    }

    /// Java `Player.getCommonRecipeLimit()`: `CommonRecipeLimit` plus `REC_C_LIM`.
    pub(crate) fn common_recipe_limit(&self, id: ObjectId) -> i32 {
        let bonus = self.stats(id).map_or(0.0, |v| v.calc(Stat::RecCLim, 0.0));
        self.settings.common_recipe_limit.wrapping_add(bonus as i32)
    }

    fn recipe_class_index(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.data.profile.class_index)
    }

    /// Java `Player.registerDwarvenRecipeList(recipe, saveToDb)`.
    pub(crate) fn register_dwarven_recipe_list(&mut self, id: ObjectId, recipe_id: i32, save_to_db: bool) {
        let class_index = self.recipe_class_index(id);
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        // `Map.put`: a recipe that is there stays where it is.
        if !p.craft.dwarven.contains(&recipe_id) {
            p.craft.dwarven.push(recipe_id);
        }
        if save_to_db {
            self.outbox.persist(PersistCommand::Recipe(RecipeWrite::Insert {
                char_id: id.0,
                recipe_id,
                class_index,
                dwarven: true,
            }));
        }
    }

    /// Java `Player.registerCommonRecipeList(recipe, saveToDb)`.
    pub(crate) fn register_common_recipe_list(&mut self, id: ObjectId, recipe_id: i32, save_to_db: bool) {
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        if !p.craft.common.contains(&recipe_id) {
            p.craft.common.push(recipe_id);
        }
        if save_to_db {
            self.outbox.persist(PersistCommand::Recipe(RecipeWrite::Insert {
                char_id: id.0,
                recipe_id,
                class_index: 0,
                dwarven: false,
            }));
        }
    }

    /// Java `Player.unregisterRecipeList(recipeId)`.
    pub(crate) fn unregister_recipe_list(&mut self, id: ObjectId, recipe_id: i32) {
        let class_index = self.recipe_class_index(id);
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        if let Some(index) = p.craft.dwarven.iter().position(|r| *r == recipe_id) {
            p.craft.dwarven.remove(index);
            self.outbox.persist(PersistCommand::Recipe(RecipeWrite::Delete {
                char_id: id.0,
                recipe_id,
                class_index,
            }));
        } else if let Some(index) = p.craft.common.iter().position(|r| *r == recipe_id) {
            p.craft.common.remove(index);
            self.outbox.persist(PersistCommand::Recipe(RecipeWrite::Delete {
                char_id: id.0,
                recipe_id,
                class_index: 0,
            }));
        } else {
            warn!("Attempted to remove unknown RecipeList: {recipe_id}");
        }
        // The recipe shortcuts of the recipe go, one `deleteShortcut` (and one list) each.
        let places: Vec<(i32, i32)> = self
            .players
            .get(&id)
            .map(|p| {
                p.data
                    .shortcuts
                    .iter()
                    .filter(|s| s.kind == SHORTCUT_TYPE_RECIPE && s.shortcut_id == recipe_id)
                    .map(|s| (s.slot, s.page))
                    .collect()
            })
            .unwrap_or_default();
        for (slot, page) in places {
            self.delete_shortcut(id, slot, page);
        }
    }

    /// Java `Player.restoreRecipeBook(true)` and `restoreRecipeShopList()`, once the player is in
    /// the world's tables: the rows the session read become the books and the shop.
    pub(crate) fn restore_craft(&mut self, id: ObjectId) {
        let Some(load) = self.players.get_mut(&id).and_then(|p| p.craft.pending.take()) else {
            return;
        };
        let class_index = self.recipe_class_index(id);
        let Some(data) = self.recipe_data() else {
            if !load.book.is_empty() {
                warn!("Could not restore recipe book data: no recipes are loaded.");
            }
            return;
        };
        for row in &load.book {
            // NOTE(java-quirk): `getRecipeList(id)` gives null for an unknown recipe and
            // `registerDwarvenRecipeList(null)` throws; the `catch` ends the whole read, so the
            // recipes after it never join the book.
            if data.get(row.recipe_id).is_none() {
                warn!(
                    "Could not restore recipe book data: unknown recipe {}.",
                    row.recipe_id
                );
                break;
            }
            if row.dwarven {
                if row.class_index == class_index {
                    self.register_dwarven_recipe_list(id, row.recipe_id, false);
                }
            } else {
                self.register_common_recipe_list(id, row.recipe_id, false);
            }
        }
        // `if (PlayerConfig.STORE_RECIPE_SHOPLIST) player.restoreRecipeShopList()`.
        if !self.settings.craft.store_recipe_shoplist {
            return;
        }
        for (recipe_id, cost) in load.shop {
            // `new ManufactureItem` reads the recipe: an unknown one throws and ends the read.
            if data.get(recipe_id).is_none() {
                warn!("Could not restore recipe shop list data for playerId: {}", id.0);
                break;
            }
            self.put_manufacture_item(id, recipe_id, cost);
        }
    }

    // ------------------------------------------------------------------ the book window

    /// Java `RecipeManager.requestBookOpen(player, isDwarvenCraft)`.
    pub(crate) fn request_book_open(&mut self, id: ObjectId, is_dwarven_craft: bool) {
        // Check if player is trying to alter recipe book while engaged in manufacturing.
        if !self.craft.active.contains_key(&id) {
            let max_mp = self.players.get(&id).map_or(0, |p| p.core.max_mp as i32);
            let recipes = if is_dwarven_craft {
                self.dwarven_recipe_book(id)
            } else {
                self.common_recipe_book(id)
            };
            self.send(
                id,
                &RecipeBookItemList {
                    is_common: !is_dwarven_craft,
                    max_mp,
                    recipes,
                },
            );
            return;
        }
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouMayNotAlterYourRecipeBookWhileEngagedInManufacturing),
        );
    }

    /// Java `RequestRecipeBookOpen.runImpl`.
    pub(crate) fn on_request_recipe_book_open(&mut self, id: ObjectId, recipe_type: i32) {
        if self.is_casting_now(id) || self.is_casting_simultaneously_now(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourRecipeBookMayNotBeAccessedWhileUsingASkill),
            );
            return;
        }
        if self.active_requester(id).is_some() {
            self.send_message(id, "You may not alter your recipe book while trading.");
            return;
        }
        self.request_book_open(id, recipe_type == 0);
    }

    /// Java `RequestRecipeBookDestroy.runImpl`.
    // The flood protector `canPerformTransaction` is not modelled.
    pub(crate) fn on_request_recipe_book_destroy(&mut self, id: ObjectId, recipe_id: i32) {
        let Some(recipe) = self.recipe_data().and_then(|d| d.get(recipe_id).cloned()) else {
            return;
        };
        self.unregister_recipe_list(id, recipe_id);
        let max_mp = self.players.get(&id).map_or(0, |p| p.core.max_mp as i32);
        let recipes = if recipe.is_dwarven {
            self.dwarven_recipe_book(id)
        } else {
            self.common_recipe_book(id)
        };
        self.send(
            id,
            &RecipeBookItemList {
                is_common: !recipe.is_dwarven,
                max_mp,
                recipes,
            },
        );
    }

    /// Java `RequestRecipeItemMakeInfo.runImpl`: `new RecipeItemMakeInfo(id, player)`.
    pub(crate) fn on_request_recipe_item_make_info(&mut self, id: ObjectId, recipe_id: i32) {
        self.send_recipe_item_make_info(id, recipe_id, true);
    }

    /// `player.sendPacket(new RecipeItemMakeInfo(recipeId, player, success))`. Java writes no
    /// packet at all for a recipe it does not know (it logs it).
    pub(crate) fn send_recipe_item_make_info(&self, id: ObjectId, recipe_id: i32, success: bool) {
        let Some(recipe) = self.recipe_data().and_then(|d| d.get(recipe_id).cloned()) else {
            tracing::info!(
                "Character: {}: Requested unexisting recipe with id = {recipe_id}",
                self.player_text(id)
            );
            return;
        };
        let Some(p) = self.players.get(&id) else {
            return;
        };
        self.send(
            id,
            &RecipeItemMakeInfo {
                id: recipe_id,
                is_common: !recipe.is_dwarven,
                cur_mp: p.core.cur_mp as i32,
                max_mp: p.core.max_mp as i32,
                success,
            },
        );
    }

    // ------------------------------------------------------------------ the Recipes item

    /// Java `Recipes.onItemUse`: a recipe item joins the right book and is used up. `true` when
    /// the item was used.
    // The `!playable.isPlayer()` branch ("your pet cannot carry this item") belongs to the pet
    // card: item handlers run for players here.
    pub(crate) fn recipes_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        if !self.settings.craft.is_crafting_enabled {
            self.send_message(id, "Crafting is disabled, you cannot register this recipe.");
            return false;
        }
        if self.is_crafting(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouMayNotAlterYourRecipeBookWhileEngagedInManufacturing),
            );
            return false;
        }
        let Some(rp) = self
            .recipe_data()
            .and_then(|d| d.by_item_id(item.item_id).cloned())
        else {
            return false;
        };
        if self.has_recipe_list(id, rp.id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThatRecipeIsAlreadyRegistered),
            );
            return false;
        }

        let (can_craft, recipe_level, recipe_limit) = if rp.is_dwarven {
            let size = self.players.get(&id).map_or(0, |p| p.craft.dwarven.len());
            (
                self.has_dwarven_craft(id),
                rp.level > self.dwarven_craft(id),
                i64::try_from(size).unwrap_or(i64::MAX) >= i64::from(self.dwarf_recipe_limit(id)),
            )
        } else {
            let size = self.players.get(&id).map_or(0, |p| p.craft.common.len());
            (
                self.has_common_craft(id),
                rp.level > self.common_craft(id),
                i64::try_from(size).unwrap_or(i64::MAX) >= i64::from(self.common_recipe_limit(id)),
            )
        };
        if !can_craft {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::TheRecipeCannotBeRegisteredYouDoNotHaveTheAbilityToCreateItems,
                ),
            );
            return false;
        }
        if recipe_level {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourCreateItemLevelIsTooLowToRegisterThisRecipe),
            );
            return false;
        }
        if recipe_limit {
            let limit = if rp.is_dwarven {
                self.dwarf_recipe_limit(id)
            } else {
                self.common_recipe_limit(id)
            };
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::UpToS1RecipesCanBeRegistered)
                    .with(SystemMessageParam::IntNumber(limit)),
            );
            return false;
        }

        if rp.is_dwarven {
            self.register_dwarven_recipe_list(id, rp.id, true);
        } else {
            self.register_common_recipe_list(id, rp.id, true);
        }

        self.destroy_item_by_object_id(id, item.object_id, 1, false);
        let templates = self.item_templates();
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1HasBeenAdded)
                .with(item_name_param(templates.as_deref(), item.item_id)),
        );
        true
    }

    // ------------------------------------------------------------------ the effects

    /// Java `OpenDwarfRecipeBook.onStart` / `OpenCommonRecipeBook.onStart`.
    pub(crate) fn open_recipe_book_effect(&mut self, effector: ObjectId, dwarven: bool) {
        if !self.is_player(effector) {
            return;
        }
        if self.is_in_store_mode(effector) {
            self.send(
                effector,
                &SystemMessage::new(SystemMessageId::ItemCreationIsNotPossibleWhileEngagedInATrade),
            );
            return;
        }
        self.request_book_open(effector, dwarven);
    }
}
