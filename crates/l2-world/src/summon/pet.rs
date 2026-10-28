//! Pets: a summon made from a pet collar, with its own level, food, exp and the `pets` row.
// Java: entity/actor/instance/Pet.java (spawnPet, restore, storeMe, storeEffect, restoreEffects,
//       FeedTask, startFeed, stopFeed, unSummon, deleteMe, destroyControlItem, doDie, doRevive,
//       deathPenalty, restoreExp, addExpAndSp, setCurrentFed, isHungry, isUncontrollable)
// Java: entity/actor/stat/PetStat.java, PlayableStat.java (addExp, removeExp, addLevel)
// Java: entity/actor/status/PetStatus.java (reduceHp)
// Java: dist/game/data/scripts/handlers/skill/effects/SummonPet.java (see `effects/handlers`)
// Java: dist/game/data/scripts/handlers/items/{SummonItems,PetFood}.java
// Java: data/sql/{CharSummonTable,SummonEffectTable}.java (the pet halves)
//
// # Where a pet lives
// Like a servitor, a pet is a creature in `World::npcs` (`NpcClass::Summon`) with a [`Summon`]
// of kind [`SummonKind::Pet`] in `SummonWorld::summons`; what only a pet has is [`PetState`]. Its
// level is the creature's `core.level` (Java `PetStat._level`), its exp and SP are the summon's
// `exp` and `sp`. HP, MP, attack and defence come from the level row of `PetData` (see
// `stats/creature_stat.rs`), not from the NPC template.
//
// # The pet inventory
// `PetInventory.getOwnerId()` is the *owner's* object id, so a pet's items are rows of `items`
// with the owner's id and `loc` `PET` / `PET_EQUIP`. They are items of `PlayerData::items` with
// that `loc` (the loader reads them with the inventory); nothing but the pet code looks at them
// (`pet_items.rs`).
//
// # Rows (D-39)
// `pets` (`Pet.storeMe`, `destroyControlItem`) and `character_pet_skills_save` (`storeEffect`,
// `restoreEffects`) are written through [`PersistCommand::Summon`] in Java's order. Java reads
// them when the pet spawns; the loader reads the rows of the collars a character holds before it
// enters (`PlayerProfile::pet_rows`, `pet_effects`) and the world keeps them as its view of the
// tables (`PetWorld::rows`, `saved_effect_rows`).
//
// # Left out, with the system that brings it
// - Mounts are `mount.rs` (`Player.mount` unsummons the pet through [`World::pet_unsummon`]).
// - The spawn protection (`World::set_spawn_protection`) delays `CharSummonTable.restorePet` until
//   the first action: the pet comes back at the first `on_action_request`.
// - The flood protector of the pet packets, `PunishmentManager.handleIllegalPlayerAction` for a
//   wrong amount (logged, see `pet_items.rs`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use l2_data::{PetData, PetDataTable, PetLevelData};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status as attr;
use l2_protocol_game::server::{
    ExChangeNpcState, PetDelete, PetItemList, SocialAction, StatusUpdate, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::servitor::{SummonSkillSave, SummonWrite};
use super::{Summon, SummonAi, SummonEffect, SummonKind, SummonTimer};
use crate::effects::SkillKey;
use crate::flood::FloodKind;
use crate::npc::NpcClass;
use crate::objects::{Npc, ObjectKind};
use crate::outbox::PersistCommand;
use crate::player::CreatureCore;
use crate::stats::java_round;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `SocialAction.LEVEL_UP`.
const LEVEL_UP_ACTION: i32 = 2122;
/// `Pet.FeedTask`: `scheduleAtFixedRate(new FeedTask(), 10000, 10000)`.
const FEED_INTERVAL_MS: u64 = 10_000;
/// The NPC id of the Sin Eater (`addExpAndSp` and `RequestAutoSoulShot` single it out).
pub const SIN_EATER_ID: i32 = 12564;
/// The Owl Monk takes pk kills away when it leaves starving (`FeedTask`).
const OWL_MONK_ID: i32 = 16050;
/// Where the pet appears: `spawnMe(player.getX() + 50, player.getY() + 100, player.getZ())`.
const SPAWN_OFFSET: (i32, i32) = (50, 100);
/// `Pet.refreshOverloaded`'s weight penalty skill.
const WEIGHT_PENALTY_SKILL: i32 = 4270;

/// The pet `.ini` values. `Default` is the Java default of each key.
#[derive(Clone)]
pub struct PetSettings {
    /// `PlayerConfig.RESTORE_PET_ON_RECONNECT` (`RestorePetOnReconnect`).
    pub restore_pet_on_reconnect: bool,
    /// `RatesConfig.PET_XP_RATE` (`PetXpRate`).
    pub pet_xp_rate: f32,
    /// `RatesConfig.PET_FOOD_RATE` (`PetFoodRate`).
    pub pet_food_rate: i32,
    /// `RatesConfig.SINEATER_XP_RATE` (`SinEaterXpRate`).
    pub sineater_xp_rate: f32,
    /// `NpcConfig.INVENTORY_MAXIMUM_PET` (`MaximumSlotsForPet`).
    pub inventory_maximum_pet: i32,
    /// `NpcConfig.PET_HP_REGEN_MULTIPLIER` (`PetHpRegenMultiplier / 100`).
    pub pet_hp_regen_multiplier: f64,
    /// `NpcConfig.PET_MP_REGEN_MULTIPLIER` (`PetMpRegenMultiplier / 100`).
    pub pet_mp_regen_multiplier: f64,
    /// `ServerConfig.PET_NAME_TEMPLATE_PATTERN` (`PetNameTemplate`): the whole name matches.
    pub pet_name_template: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// `PlayerConfig.PLAYER_SPAWN_PROTECTION` (`PlayerSpawnProtection`, seconds): a stored pet
    /// comes back at the first action within this time after the login.
    pub player_spawn_protection: i32,
}

impl Default for PetSettings {
    fn default() -> Self {
        Self {
            restore_pet_on_reconnect: true,
            pet_xp_rate: 1.0,
            pet_food_rate: 1,
            sineater_xp_rate: 1.0,
            inventory_maximum_pet: 12,
            pet_hp_regen_multiplier: 1.0,
            pet_mp_regen_multiplier: 1.0,
            pet_name_template: Arc::new(|_| true),
            player_spawn_protection: 0,
        }
    }
}

impl std::fmt::Debug for PetSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PetSettings")
            .field("restore_pet_on_reconnect", &self.restore_pet_on_reconnect)
            .field("pet_xp_rate", &self.pet_xp_rate)
            .field("pet_food_rate", &self.pet_food_rate)
            .field("sineater_xp_rate", &self.sineater_xp_rate)
            .field("inventory_maximum_pet", &self.inventory_maximum_pet)
            .field("pet_hp_regen_multiplier", &self.pet_hp_regen_multiplier)
            .field("pet_mp_regen_multiplier", &self.pet_mp_regen_multiplier)
            .field("player_spawn_protection", &self.player_spawn_protection)
            .finish_non_exhaustive()
    }
}

/// A row of `pets` as the loader read it, and the world's view of the table after its own writes.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedPet {
    /// `item_obj_id`: the object id of the collar.
    pub control_object_id: ObjectId,
    /// `name`, `None` for NULL (the owner has not named the pet).
    pub name: Option<String>,
    pub level: i32,
    pub cur_hp: i32,
    pub cur_mp: i32,
    pub exp: i64,
    pub sp: i64,
    pub fed: i32,
    /// `restore = 'true'`: the pet comes back after a relog.
    pub restore: bool,
}

/// A row of `character_pet_skills_save` as the loader read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedPetEffect {
    /// `petObjItemId`.
    pub control_object_id: ObjectId,
    pub skill_id: i32,
    pub skill_level: i32,
    pub remaining_time: i32,
}

/// The rows of one collar the persistence task read (`WorldCommand::PetRowLoaded`).
#[derive(Debug, Clone, PartialEq)]
pub struct PetRowData {
    /// The `pets` row, `None` when the database really has none (a new pet).
    pub row: Option<SavedPet>,
    /// The `character_pet_skills_save` rows of the collar.
    pub effects: Vec<SavedPetEffect>,
}

/// What a `pets` write carries (Java binds these in `Pet.storeMe`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetRowWrite {
    /// `item_obj_id`.
    pub control_object_id: ObjectId,
    /// `ownerId`.
    pub owner: ObjectId,
    /// `getName()`, `None` binds NULL.
    pub name: Option<String>,
    pub level: i32,
    /// `getStatus().getCurrentHp()` is bound with `setDouble` into an integer column, which the
    /// database rounds: the rounded value is carried.
    pub cur_hp: i32,
    pub cur_mp: i32,
    pub exp: i64,
    pub sp: i64,
    pub fed: i32,
    /// `String.valueOf(_restoreSummon)`.
    pub restore: bool,
}

/// The fields only a pet has (Java `Pet`).
#[derive(Debug, Clone, PartialEq)]
pub struct PetState {
    /// `_controlObjectId`: the object id of the collar.
    pub control_object_id: ObjectId,
    /// `_curFed`.
    pub cur_fed: i32,
    /// `_respawned`: the `pets` row exists.
    pub respawned: bool,
    /// `_expBeforeDeath`.
    pub exp_before_death: i64,
    /// `_curWeightPenalty`.
    pub weight_penalty: i32,
    /// `Creature.isOverloaded()` (`Pet.refreshOverloaded`): the pet cannot move.
    pub overloaded: bool,
    /// `_feedTask`.
    pub feed_timer: Option<TimerId>,
    /// `BabyPet`'s fields; `None` for any other pet.
    pub baby: Option<super::baby_pet::BabyState>,
}

/// The pet tables and the pet `.ini` values the world keeps.
#[derive(Debug, Default)]
pub struct PetWorld {
    /// `PetDataTable` (`data/stats/pets`).
    pub(crate) table: Option<Arc<PetDataTable>>,
    pub(crate) settings: PetSettings,
    /// `PetNameTable`: the `(lower case name, collar item id)` of every named pet, read once at
    /// startup (`doesPetNameExist`); a new name joins it.
    pub(crate) names: HashSet<(String, i32)>,
    /// `CharSummonTable._pets`: owner -> collar of the pet that comes back after a relog.
    pub(crate) saved: HashMap<ObjectId, ObjectId>,
    /// The world's view of `pets`, by collar.
    pub(crate) rows: HashMap<ObjectId, SavedPet>,
    /// `SummonEffectTable._petEffects`: collar -> the buffs kept for the next summon.
    pub(crate) effects: HashMap<ObjectId, Vec<SummonEffect>>,
    /// The world's view of `character_pet_skills_save`, by collar.
    pub(crate) effect_rows: HashMap<ObjectId, Vec<SummonSkillSave>>,
    /// `PetItemHolder`: the collar a player is summoning with (owner -> item object id).
    pub(crate) pending_collar: HashMap<ObjectId, ObjectId>,
    /// `Player._petItems` (`havePetInvItems`): the pet had items when it was unsummoned.
    pub(crate) have_inv_items: HashSet<ObjectId>,
    /// The collars whose `pets` row is known to be absent (read at the login, or answered by the
    /// persistence task): they make a new pet. A collar neither here nor in `rows` is asked for.
    pub(crate) no_row: HashSet<ObjectId>,
    /// Owners waiting for the answer to a row request, with the collar.
    pub(crate) row_pending: HashMap<ObjectId, ObjectId>,
}

impl World {
    // ------------------------------------------------------------------ installing

    /// Installs `data/stats/pets`. Call before players enter.
    pub fn set_pet_data(&mut self, table: Arc<PetDataTable>) {
        self.summons.pet_world.table = Some(table);
    }

    /// Installs the pet `.ini` values. Call before players enter.
    pub fn set_pet_settings(&mut self, settings: PetSettings) {
        self.summons.pet_world.settings = settings;
    }

    /// Installs the names of the pets that exist (`PetNameTable.doesPetNameExist`): the name and
    /// the collar item id of each. Call before players enter.
    pub fn load_pet_names(&mut self, names: impl IntoIterator<Item = (String, i32)>) {
        self.summons.pet_world.names = names
            .into_iter()
            .map(|(name, item_id)| (name.to_lowercase(), item_id))
            .collect();
    }

    // ------------------------------------------------------------------ lookups

    /// The pet state of `id`, if it is a pet.
    pub fn pet_state(&self, id: ObjectId) -> Option<&PetState> {
        self.summon(id)?.pet.as_ref()
    }

    pub(crate) fn pet_state_mut(&mut self, id: ObjectId) -> Option<&mut PetState> {
        self.summon_mut(id)?.pet.as_mut()
    }

    /// `PetDataTable` of the world.
    pub(crate) fn pet_table(&self) -> Option<&Arc<PetDataTable>> {
        self.summons.pet_world.table.as_ref()
    }

    /// Java `Pet.getPetData()`.
    pub fn pet_data_of(&self, summon: ObjectId) -> Option<&PetData> {
        let npc_id = self.summon_npc_id(summon)?;
        self.pet_table()?.pet_data(npc_id)
    }

    /// Java `Pet.getPetLevelData()`: the numbers of the pet's level.
    pub fn pet_level_data_of(&self, summon: ObjectId) -> Option<&PetLevelData> {
        let npc = self.npcs.get(&summon)?;
        self.pet_table()?.pet_level_data(npc.npc_id, npc.core.level)
    }

    /// Java `Pet.getControlObjectId()`.
    pub fn pet_control_object_id(&self, summon: ObjectId) -> Option<ObjectId> {
        self.pet_state(summon).map(|p| p.control_object_id)
    }

    /// Java `Pet.getCurrentFed()`.
    pub fn pet_current_fed(&self, summon: ObjectId) -> i32 {
        self.pet_state(summon).map_or(0, |p| p.cur_fed)
    }

    /// Java `Pet.getMaxFed()` (`PetStat.getMaxFeed()`).
    pub fn pet_max_fed(&self, summon: ObjectId) -> i32 {
        self.pet_level_data_of(summon).map_or(0, |l| l.max_feed)
    }

    /// Java `Pet.isHungry()`: `_curFed < (hungryLimit / 100f) * maxFeed`.
    pub fn pet_is_hungry(&self, summon: ObjectId) -> bool {
        let (Some(data), Some(level)) = (self.pet_data_of(summon), self.pet_level_data_of(summon)) else {
            return false;
        };
        // `(getPetData().getHungryLimit() / 100f) * getPetLevelData().getPetMaxFeed()` in floats.
        let limit = (data.hungry_limit as f32 / 100.0_f32) * level.max_feed as f32;
        (self.pet_current_fed(summon) as f32) < limit
    }

    /// Java `Pet.isUncontrollable()`.
    pub fn pet_is_uncontrollable(&self, summon: ObjectId) -> bool {
        self.pet_state(summon).is_some_and(|p| p.cur_fed <= 0)
    }

    /// Java `Pet.getControlItem()`: the collar in the owner's inventory.
    pub(crate) fn pet_control_item(&self, summon: ObjectId) -> Option<crate::player::ItemData> {
        let owner = self.summon_owner(summon)?;
        let control = self.pet_control_object_id(summon)?;
        self.inventory_item(owner, control).cloned()
    }

    /// Java `Pet.getExpForLevel(level)` (`PetStat.getExpForLevel`): the exp the level starts at;
    /// 0 (and a warning) for a level the pet has no row for.
    pub(crate) fn pet_exp_for_level(&self, summon: ObjectId, level: i32) -> i64 {
        let Some(npc_id) = self.summon_npc_id(summon) else {
            return 0;
        };
        match self.pet_table().and_then(|t| t.pet_level_data(npc_id, level)) {
            Some(data) => data.max_exp,
            None => {
                warn!(
                    "Pet objectId:{summon}, NpcId:{npc_id}, level:{level} is missing data from pets_stats table!"
                );
                0
            }
        }
    }

    /// Java `Pet.getExpForThisLevel()`.
    pub(crate) fn pet_exp_for_this_level(&self, summon: ObjectId) -> i64 {
        let level = self.npcs.get(&summon).map_or(0, |n| n.core.level);
        self.pet_exp_for_level(summon, level)
    }

    /// Java `Pet.getExpForNextLevel()`.
    pub(crate) fn pet_exp_for_next_level(&self, summon: ObjectId) -> i64 {
        let level = self.npcs.get(&summon).map_or(0, |n| n.core.level);
        self.pet_exp_for_level(summon, level + 1)
    }

    fn pet_message(&self, to: ObjectId, id: SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }

    // ------------------------------------------------------------------ making the pet

    /// Java `SummonItems.onItemUse(playable, item, forceUse)`: the checks of a pet collar, then
    /// the item skill (`SummonPet`) is cast like any item skill.
    pub(crate) fn summon_items_on_use(
        &mut self,
        id: ObjectId,
        item: &crate::player::ItemData,
        template: &l2_data::item::ItemTemplate,
        force_use: bool,
    ) -> bool {
        if !self.is_player(id) {
            self.pet_message(id, SystemMessageId::YourPetCannotCarryThisItem);
            return false;
        }
        // The block checker arena and the observer mode are not modelled.
        if !self.flood_allows(id, FloodKind::ItemPetSummon)
            || self.is_all_skills_disabled(id)
            || self.is_casting_now(id)
        {
            return false;
        }
        if self.is_sitting(id) {
            self.pet_message(id, SystemMessageId::YouCannotMoveWhileSitting);
            return false;
        }
        // `player.hasSummon() || player.isMounted()`.
        if self.summon_of(id).is_some() || self.is_mounted(id) {
            self.pet_message(id, SystemMessageId::YouAlreadyHaveAPet);
            return false;
        }
        if self.is_attacking_now(id) || self.is_in_combat(id) {
            self.pet_message(id, SystemMessageId::YouCannotSummonDuringCombat);
            return false;
        }
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) || self.is_in_store_mode(id) {
            self.pet_message(
                id,
                SystemMessageId::YouCannotSummonDuringATradeOrWhileUsingAPrivateStore,
            );
            return false;
        }
        let known = self.pet_table().is_some_and(|t| {
            t.pet_data_by_item_id(item.item_id)
                .is_some_and(|d| d.npc_id != -1)
        });
        if !known {
            return false;
        }
        // `player.addScript(new PetItemHolder(item))`.
        self.summons.pet_world.pending_collar.insert(id, item.object_id);
        self.item_skills_on_use(id, item, template, force_use)
    }

    /// Java `SummonPet.onStart` from the checks on: the collar the player used (the
    /// `PetItemHolder` `SummonItems` attached) makes the pet.
    pub(crate) fn summon_pet_effect(&mut self, player: ObjectId) {
        // `player.hasSummon() || player.isMounted()`.
        if self.summon_of(player).is_some() || self.is_mounted(player) {
            self.pet_message(player, SystemMessageId::YouAlreadyHaveAPet);
            return;
        }
        let Some(control) = self.summons.pet_world.pending_collar.remove(&player) else {
            warn!("Summoning pet without attaching PetItemHandler!");
            return;
        };
        let Some(item) = self.inventory_item(player, control).cloned() else {
            warn!("Player: {player} is trying to summon pet from item that he doesn't owns.");
            return;
        };
        let known = self
            .pet_table()
            .is_some_and(|t| t.pet_data_by_item_id(item.item_id).is_some());
        if !known {
            return;
        }
        // The `pets` row of a collar that came to the character after the login is not in the
        // world: it is read first, so a pet that has a row is never made new (D-39).
        let pet_world = &self.summons.pet_world;
        if !pet_world.rows.contains_key(&control) && !pet_world.no_row.contains(&control) {
            if self
                .summons
                .pet_world
                .row_pending
                .insert(player, control)
                .is_none()
            {
                self.outbox.persist(PersistCommand::PetRowRequest {
                    owner: player,
                    control_object_id: control,
                });
            }
            return;
        }
        if self.summon_pet(player, control).is_none() {
            warn!("Could not spawn pet for player: {player} using item: {control}");
        }
    }

    /// `WorldCommand::PetRowLoaded`: the rows of the collar are known now; the summon goes on, as
    /// a restore when the row exists and as a new pet when the database has none. A failed read
    /// summons nothing.
    pub(crate) fn on_pet_row_loaded(
        &mut self,
        owner: ObjectId,
        control: ObjectId,
        result: Result<PetRowData, ()>,
    ) {
        if self.summons.pet_world.row_pending.get(&owner) != Some(&control) {
            return;
        }
        self.summons.pet_world.row_pending.remove(&owner);
        if !self.players.contains_key(&owner) {
            return;
        }
        let Ok(data) = result else {
            warn!("Could not read the pet row of collar {control} for {owner}; the pet is not summoned.");
            return;
        };
        match data.row {
            Some(row) => {
                self.summons.pet_world.no_row.remove(&control);
                self.summons.pet_world.rows.insert(control, row);
            }
            None => {
                self.summons.pet_world.rows.remove(&control);
                self.summons.pet_world.no_row.insert(control);
            }
        }
        self.summons.pet_world.effect_rows.remove(&control);
        for effect in data.effects {
            self.summons
                .pet_world
                .effect_rows
                .entry(control)
                .or_default()
                .push(SummonSkillSave {
                    skill_id: effect.skill_id,
                    skill_level: effect.skill_level,
                    remaining_time: effect.remaining_time,
                });
        }
        // The state may have changed while the database answered.
        if self.summon_of(owner).is_some()
            || self.is_alike_dead(owner)
            || self.inventory_item(owner, control).is_none()
        {
            return;
        }
        if self.summon_pet(owner, control).is_none() {
            warn!("Could not spawn pet for player: {owner} using item: {control}");
        }
    }

    /// Java `Pet.spawnPet(template, owner, control)`: the pet of the collar `control`, made from its
    /// row when there is one, at the level the owner's synchronises it to. It is not stored,
    /// not the owner's summon yet and not in view: `summon_pet` (`SummonPet.onStart`) and the pet
    /// manager's evolve and restore go on from here.
    pub(crate) fn pet_spawn_pet(&mut self, owner: ObjectId, control: ObjectId) -> Option<ObjectId> {
        let item = self.inventory_item(owner, control)?.clone();
        let (npc_id, sync_level, max_level) = {
            let data = self.pet_table()?.pet_data_by_item_id(item.item_id)?;
            (data.npc_id, data.sync_level, data.max_level())
        };
        // `Pet.spawnPet`: "Owner has a pet listed in world."
        if let Some(existing) = self.summon_of(owner).filter(|s| self.is_pet(*s)) {
            self.pet_unsummon(existing, Some(owner));
        }
        let pet = self.restore_pet(owner, control, npc_id)?;

        // `pet.setTitle(owner.getName())`, the level the owner's synchronises it to.
        let owner_level = self.core_of(owner).map_or(1, |c| c.level);
        if sync_level && self.npcs.get(&pet).is_some_and(|n| n.core.level != owner_level) {
            let available = max_level.min(owner_level);
            self.pet_set_level(pet, available);
            let exp = self.pet_exp_for_level(pet, available);
            if let Some(state) = self.summon_mut(pet) {
                state.exp = exp;
            }
        }
        Some(pet)
    }

    /// Java `Pet.spawnPet(template, owner, control)` followed by the rest of `SummonPet.onStart`:
    /// the pet of the collar `control` is made, stored when it is new, spawned next to the owner
    /// and starts to eat. `None` (with a warning, like Java) when it cannot be made.
    pub fn summon_pet(&mut self, owner: ObjectId, control: ObjectId) -> Option<ObjectId> {
        let pet = self.pet_spawn_pet(owner, control)?;

        // `SummonPet.onStart`.
        if let Some(state) = self.summon_mut(pet) {
            state.show_summon_animation = true;
        }
        let respawned = self.pet_state(pet).is_some_and(|p| p.respawned);
        if !respawned {
            // `pet.fullRestore()`, the exp of its level, a full stomach.
            self.pet_refresh_max_vitals(pet);
            let (hp, mp) = self
                .stats(pet)
                .map_or((0.0, 0.0), |v| (f64::from(v.max_hp()), f64::from(v.max_mp())));
            self.set_current_hp(pet, hp, false);
            self.set_current_mp(pet, mp, false);
            let exp = self.pet_exp_for_this_level(pet);
            if let Some(state) = self.summon_mut(pet) {
                state.exp = exp;
            }
            let max_fed = self.pet_max_fed(pet);
            self.pet_set_current_fed(pet, max_fed);
        }
        self.summon_set_running(pet, true);
        if !respawned {
            self.pet_store_me(pet);
        }
        // `item.setEnchantLevel(pet.getLevel())`: written with the next store of the item.
        let level = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        self.pet_set_control_enchant(owner, control, level);
        self.set_pet(owner, Some(pet));
        let (x, y, z) = self.position_of(owner)?;
        self.spawn_at(pet, x + SPAWN_OFFSET.0, y + SPAWN_OFFSET.1, z);
        if !self.npcs.contains_key(&pet) {
            // Out of the world: `spawn` disposed of it.
            self.set_pet(owner, None);
            self.summons.summons.remove(&pet);
            return None;
        }
        self.pet_start_feed(pet);
        self.set_follow_status(pet, true);
        let list = self.pet_item_list(owner);
        self.send(owner, &list);
        self.broadcast_status_update(pet);
        Some(pet)
    }

    /// Java `Pet.restore(control, template, owner)` and the constructor: the creature, from the
    /// `pets` row when there is one.
    fn restore_pet(&mut self, owner: ObjectId, control: ObjectId, npc_id: i32) -> Option<ObjectId> {
        let templates = self.npc_world.templates_arc()?;
        let Some(template) = templates.get(npc_id) else {
            warn!("Pet: no NPC template {npc_id} for {owner}.");
            return None;
        };
        let row = self.summons.pet_world.rows.get(&control).cloned();
        let (owner_name, owner_level) = {
            let player = self.players.get(&owner)?;
            (player.data.name.clone(), player.core.level)
        };
        // `new Pet(template, owner, control)`: "template.getDisplayId() == 12564 ? owner level :
        // template level"; from a row, the stored level. Never below the pet's lowest level.
        let given = match &row {
            Some(row) => row.level,
            None if template.display_id == SIN_EATER_ID => owner_level,
            None => i32::from(template.level),
        };
        let min_level = self.pet_table()?.pet_min_level(npc_id)?;
        let level = given.max(min_level);
        if self.pet_table()?.pet_level_data(npc_id, level).is_none() {
            // `PetStat.setLevel`: "No pet data for npc: .. level: ..".
            warn!(
                "Could not restore pet data for owner: {owner} - No pet data for npc: {npc_id} level: {level}"
            );
            return None;
        }
        let ids = self.npc_world.ids.clone()?;
        let id = match ids.allocate() {
            Ok(id) => id,
            Err(e) => {
                warn!("Pet: cannot make the pet of {owner}: {e}");
                return None;
            }
        };
        let (x, y, z) = self.position_of(owner)?;
        let instance = self.objects.get(&owner).map_or(0, |e| e.instance_id);
        let npc = Npc {
            core: CreatureCore {
                object_id: id,
                x,
                y,
                z,
                heading: 0,
                level,
                cur_hp: 0.0,
                max_hp: 0.0,
                cur_mp: 0.0,
                max_mp: 0.0,
                cur_cp: 0.0,
                max_cp: 0.0,
                calculators: self.npc_calculators(),
                skills: Vec::new(),
            },
            npc_id: template.id,
            class: NpcClass::Summon,
            spawn: None,
            leader: None,
            decayed: false,
            in_water: self.npc_world.is_water_at(x, y, z),
            right_hand: 0,
            left_hand: 0,
            enchant_effect: template.weapon_enchant,
            display_effect: 0,
            collision_radius: f64::from(template.collision_radius),
            must_reward_exp_sp: false,
            special_drop: false,
            movement: crate::movement::NpcMovement::default(),
            spawn_point: (0, 0, 0),
            spawn_time_ms: 0,
        };
        self.npcs.insert(id, npc);
        self.forget_effects(id);
        self.forget_status(id);
        self.add_npc_template_skills(id);

        let name = row.as_ref().and_then(|r| r.name.clone()).unwrap_or_default();
        let baby = (template.npc_type == "BabyPet").then(super::baby_pet::BabyState::default);
        let summon = Summon {
            owner,
            owner_offline: false,
            kind: SummonKind::Pet,
            name: name.clone(),
            // `pet.setTitle(owner.getName())`.
            title: owner_name,
            follow: true,
            previous_follow: true,
            restore_summon: true,
            attack_range: 36,
            show_summon_animation: true,
            exp: 0,
            sp: 0,
            ai: SummonAi {
                start_follow: true,
                ..SummonAi::default()
            },
            abnormal_effect_timer: None,
            servitor: None,
            pet: Some(PetState {
                control_object_id: control,
                cur_fed: 0,
                respawned: row.is_some(),
                exp_before_death: 0,
                weight_penalty: 0,
                overloaded: false,
                feed_timer: None,
                baby,
            }),
        };
        self.summons.summons.insert(id, summon);
        self.ai.creatures.insert(id, crate::ai::AiState::default());
        self.register_object(id, ObjectKind::Summon);
        if let Some(entry) = self.objects.get_mut(&id) {
            entry.instance_id = instance;
        }
        // `_inventory.restore()`: the worn items put their functions on.
        self.pet_restore_equipment(owner, id);
        // The level changes the maxima the current values are capped by.
        self.pet_refresh_max_vitals(id);
        // `controlItem.setEnchantLevel(level)` (PetStat.setLevel).
        self.pet_set_control_enchant(owner, control, level);

        if let Some(row) = row {
            // `pet._respawned = true; pet.setName(rs.getString("name"))`.
            self.pet_sync_control_name_flag(owner, control, row.name.is_some());
            // DS: update experience based by level, avoiding pet delevels due to exp per level
            // values changed.
            let mut exp = row.exp;
            if let Some(info) = self.pet_level_data_of(id)
                && exp < info.max_exp
            {
                exp = info.max_exp;
            }
            if let Some(state) = self.summon_mut(id) {
                state.exp = exp;
                state.sp = row.sp;
            }
            self.set_current_hp(id, f64::from(row.cur_hp), false);
            self.set_current_mp(id, f64::from(row.cur_mp), false);
            if row.cur_hp < 1 {
                self.set_dead(id, true);
                self.stop_hp_mp_regeneration(id);
            }
            self.pet_set_current_fed(id, row.fed);
        } else {
            self.pet_sync_control_name_flag(owner, control, false);
        }
        Some(id)
    }

    /// The maxima into the creature (`refresh_max_vitals` for a player or NPC): HP and MP of the
    /// pet's level row.
    fn pet_refresh_max_vitals(&mut self, pet: ObjectId) {
        self.refresh_max_vitals(pet, false);
    }

    /// `Item.setEnchantLevel(level)` of the collar (the pet's level is shown as its enchant).
    pub(crate) fn pet_set_control_enchant(&mut self, owner: ObjectId, control: ObjectId, level: i32) {
        if let Some(item) = self
            .players
            .get_mut(&owner)
            .and_then(|p| p.data.items.iter_mut().find(|i| i.object_id == control))
        {
            item.enchant_level = level;
        }
    }

    /// `Pet.setName(name)`: the collar's second custom type tells whether the pet is named
    /// (`controlItem.getCustomType2() == (name == null ? 1 : 0)` flips it), written at once with an
    /// inventory update.
    pub(crate) fn pet_sync_control_name_flag(&mut self, owner: ObjectId, control: ObjectId, named: bool) {
        let flag = i32::from(named);
        let current = self.inventory_item(owner, control).map(|i| i.custom_type2);
        match current {
            Some(current) if current == i32::from(!named) => {
                if let Some(item) = self
                    .players
                    .get_mut(&owner)
                    .and_then(|p| p.data.items.iter_mut().find(|i| i.object_id == control))
                {
                    item.custom_type2 = flag;
                }
                self.update_item_in_db(owner, control);
                if let Some(entry) = self.modified_entry(owner, control) {
                    self.send_inventory_update(owner, entry);
                }
            }
            Some(_) => {}
            None => warn!("Pet control item null, for pet of {owner}"),
        }
    }

    // ------------------------------------------------------------------ food

    /// Java `Pet.setCurrentFed(num)`: the owner hears when the food runs out and when it comes
    /// back; the value is capped at the maximum.
    pub fn pet_set_current_fed(&mut self, pet: ObjectId, num: i32) {
        let Some(old) = self.pet_state(pet).map(|p| p.cur_fed) else {
            return;
        };
        if num <= 0 {
            self.send(
                pet,
                &ExChangeNpcState {
                    object_id: pet.0,
                    state: 0x64,
                },
            );
        } else if old <= 0 && num > 0 {
            self.send(
                pet,
                &ExChangeNpcState {
                    object_id: pet.0,
                    state: 0x65,
                },
            );
        }
        let max_fed = self.pet_max_fed(pet);
        if let Some(state) = self.pet_state_mut(pet) {
            state.cur_fed = if num > max_fed { max_fed } else { num };
        }
    }

    /// Java `Pet.startFeed()`: the feed task runs while the pet is alive and its owner's summon.
    pub(crate) fn pet_start_feed(&mut self, pet: ObjectId) {
        self.pet_stop_feed(pet);
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        if !self.is_dead(pet) && self.summon_of(owner) == Some(pet) {
            let due = self.tick + TimerWheel::ms_to_ticks(FEED_INTERVAL_MS);
            let timer = self
                .timers
                .schedule_at(due, TimerKind::Summon(SummonTimer::PetFeed(pet)));
            if let Some(state) = self.pet_state_mut(pet) {
                state.feed_timer = Some(timer);
            }
        }
    }

    /// Java `Pet.stopFeed()`.
    pub(crate) fn pet_stop_feed(&mut self, pet: ObjectId) {
        let timer = self.pet_state_mut(pet).and_then(|p| p.feed_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Java `Pet.FeedTask.run()`: every 10 s the pet eats into its food bar, eats what it has in
    /// its inventory when it is hungry, and leaves when it starves.
    pub(crate) fn on_pet_feed_timer(&mut self, pet: ObjectId) {
        let Some(state) = self.pet_state_mut(pet) else {
            return;
        };
        // The fixed rate: the next run first (the task cancels itself below when it must).
        state.feed_timer = None;
        let owner = self.summon_owner(pet);
        // `getOwner() == null || !getOwner().hasSummon() || getOwner().getSummon() != this`.
        if owner.is_none_or(|o| self.summon_of(o) != Some(pet)) {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(FEED_INTERVAL_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Summon(SummonTimer::PetFeed(pet)));
        if let Some(state) = self.pet_state_mut(pet) {
            state.feed_timer = Some(timer);
        }
        let Some(owner) = owner else {
            return;
        };

        let consume = self.pet_level_data_of(pet).map_or(0, |l| {
            if self.is_attacking_now(pet) {
                l.feed_battle
            } else {
                l.feed_normal
            }
        });
        let cur = self.pet_current_fed(pet);
        self.pet_set_current_fed(pet, if cur > consume { cur - consume } else { 0 });
        self.broadcast_status_update(pet);

        if self.pet_is_uncontrollable(pet) {
            if self.rng().below(100) < 30 {
                // Owl Monk remove PK.
                if self.summon_npc_id(pet) == Some(OWL_MONK_ID) {
                    let take = self.rng().get(1, 6);
                    if let Some(player) = self.players.get_mut(&owner) {
                        player.data.pk_kills = (player.data.pk_kills - take).max(0);
                    }
                }
                self.pet_message(pet, SystemMessageId::StarvingGrumpyAndFedUpYourPetHasLeft);
                self.pet_unsummon(pet, Some(owner));
                self.pet_stop_feed(pet);
                return;
            }
            self.pet_message(
                pet,
                SystemMessageId::YourPetServitorIsUnresponsiveAndWillNotObeyAnyOrders,
            );
            return;
        }

        let food_ids = self.pet_data_of(pet).map(PetData::food).unwrap_or_default();
        if food_ids.is_empty() {
            if self.pet_is_hungry(pet) {
                self.pet_message(pet, SystemMessageId::YourPetIsVeryHungryPleaseBeCareful);
            }
            return;
        }
        // Try to find food in pet inventory.
        let food = food_ids
            .iter()
            .find_map(|id| self.pet_item_by_item_id(owner, *id).cloned());
        // Pet is hungry and food is available - auto feed.
        if let Some(food) = food
            && self.pet_is_hungry(pet)
            && self.item_has_handler(food.item_id)
        {
            let templates = self.item_templates();
            self.send(
                pet,
                &SystemMessage::new(SystemMessageId::YourPetWasHungrySoItAteS1).with(
                    crate::inventory::item_name_param(templates.as_deref(), food.item_id),
                ),
            );
            self.pet_item_handler_use(pet, &food);
        }
        // Notify player if pet uncontrollable due to starvation.
        if self.pet_is_uncontrollable(pet) {
            self.pet_message(
                pet,
                SystemMessageId::YourPetIsStarvingAndWillNotObeyUntilItGetsItSFoodFeedYourPet,
            );
        }
    }

    // ------------------------------------------------------------------ exp and levels

    /// The lowest level of the pet (`PetDataTable.getPetMinLevel`), 1 without data.
    fn pet_min_level(&self, pet: ObjectId) -> i32 {
        self.summon_npc_id(pet)
            .and_then(|id| self.pet_table()?.pet_min_level(id))
            .unwrap_or(1)
    }

    /// Java `PetStat.setLevel(level)`: the level row changes (a level without one is refused,
    /// Java throws), the feed task restarts, the collar shows the level.
    pub(crate) fn pet_set_level(&mut self, pet: ObjectId, level: i32) -> bool {
        let Some(npc_id) = self.summon_npc_id(pet) else {
            return false;
        };
        if self
            .pet_table()
            .and_then(|t| t.pet_level_data(npc_id, level))
            .is_none()
        {
            warn!("No pet data for npc: {npc_id} level: {level}");
            return false;
        }
        self.pet_stop_feed(pet);
        if let Some(npc) = self.npcs.get_mut(&pet) {
            npc.core.level = level;
        }
        self.pet_start_feed(pet);
        if let (Some(owner), Some(control)) = (self.summon_owner(pet), self.pet_control_object_id(pet)) {
            self.pet_set_control_enchant(owner, control, level);
        }
        true
    }

    /// Java `Pet.addExpAndSp(addToExp, addToSp)`: the rate, then `PetStat.addExpAndSp` (the SP is
    /// not used).
    pub fn pet_add_exp_and_sp(&mut self, pet: ObjectId, add_to_exp: f64, add_to_sp: f64) {
        let _ = add_to_sp;
        let settings = &self.summons.pet_world.settings;
        let rate = if self.summon_npc_id(pet) == Some(SIN_EATER_ID) {
            settings.sineater_xp_rate
        } else {
            settings.pet_xp_rate
        };
        // `Math.round(addToExp * RatesConfig.SINEATER_XP_RATE)`: a double times a float.
        let final_exp = java_round(add_to_exp * f64::from(rate));
        if self.pet_is_uncontrollable(pet) || !self.pet_add_exp(pet, final_exp) {
            return;
        }
        self.update_and_broadcast_summon_status(pet, 1);
        self.send(
            pet,
            &SystemMessage::new(SystemMessageId::YourPetGainedS1ExperiencePoints)
                .with(SystemMessageParam::LongNumber(final_exp)),
        );
    }

    /// The level the exp makes: `PlayableStat.addExp`'s loop with the pet's exp table.
    fn pet_level_for_exp(&self, pet: ObjectId, max_level: i32) -> i32 {
        let minimum = self.pet_min_level(pet);
        let exp = self.summon(pet).map_or(0, |s| s.exp);
        let mut level = minimum;
        for tmp in minimum..=max_level {
            if exp >= self.pet_exp_for_level(pet, tmp) {
                continue;
            }
            level = tmp - 1;
            break;
        }
        level
    }

    /// Java `PlayableStat.addExp(amount)` of a pet (`PetStat.getMaxLevel()` is the highest pet
    /// level, `getExpForLevel` the pet's own table). `false` without an experience table.
    pub(crate) fn pet_add_exp(&mut self, pet: ObjectId, amount: i64) -> bool {
        let Some(max_level) = self.exp_table_max_pet_level() else {
            return false;
        };
        let cap = self.pet_exp_for_level(pet, max_level);
        let exp = self.summon(pet).map_or(0, |s| s.exp);
        if exp.wrapping_add(amount) < 0 || (amount > 0 && exp == cap - 1) {
            return true;
        }
        let mut value = amount;
        if exp.wrapping_add(value) >= cap {
            value = cap - 1 - exp;
        }
        if let Some(state) = self.summon_mut(pet) {
            state.exp = exp.wrapping_add(value);
        }
        let minimum = self.pet_min_level(pet);
        let level = self.pet_level_for_exp(pet, max_level);
        let current = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        if level != current && level >= minimum {
            self.pet_add_level(pet, level - current);
        }
        true
    }

    /// Java `PlayableStat.removeExp(amount)` of a pet: never below 1 exp, and the level drops to
    /// what the exp makes. `false` without an experience table.
    pub(crate) fn pet_remove_exp(&mut self, pet: ObjectId, amount: i64) -> bool {
        let Some(max_level) = self.exp_table_max_pet_level() else {
            return false;
        };
        let exp = self.summon(pet).map_or(0, |s| s.exp);
        let mut value = amount;
        if exp - value < 0 {
            value = exp - 1;
        }
        if let Some(state) = self.summon_mut(pet) {
            state.exp = exp - value;
        }
        let minimum = self.pet_min_level(pet);
        let level = self.pet_level_for_exp(pet, max_level);
        let current = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        if level != current && level >= minimum {
            self.pet_add_level(pet, level - current);
        }
        true
    }

    /// Java `PetStat.addLevel(value)` with `PlayableStat.addLevel`: `true` when the level went up.
    pub(crate) fn pet_add_level(&mut self, pet: ObjectId, amount: i32) -> bool {
        let Some(max_level) = self.exp_table_max_pet_level() else {
            return false;
        };
        let current = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        if current + amount > max_level - 1 {
            return false;
        }
        // `PlayableStat.addLevel`.
        let increased = amount > 0;
        if !self.pet_set_level(pet, current + amount) {
            return false;
        }
        // Sync up exp with current level.
        let level = current + amount;
        let exp = self.summon(pet).map_or(0, |s| s.exp);
        if exp >= self.pet_exp_for_level(pet, level + 1) || self.pet_exp_for_level(pet, level) > exp {
            let for_level = self.pet_exp_for_level(pet, level);
            if let Some(state) = self.summon_mut(pet) {
                state.exp = for_level;
            }
        }
        self.pet_refresh_max_vitals(pet);
        if increased {
            let (hp, mp) = self
                .stats(pet)
                .map_or((0.0, 0.0), |v| (f64::from(v.max_hp()), f64::from(v.max_mp())));
            self.set_current_hp(pet, hp, true);
            self.set_current_mp(pet, mp, true);
        }
        // `PetStat.addLevel`: the level and maxima to everybody, the level-up effect.
        let (level, max_hp, max_mp) = match self.stats(pet) {
            Some(v) => (v.level(), v.max_hp(), v.max_mp()),
            None => return increased,
        };
        let mut update = StatusUpdate::new(pet.0);
        update.add_attribute(attr::LEVEL, level);
        update.add_attribute(attr::MAX_HP, max_hp);
        update.add_attribute(attr::MAX_MP, max_mp);
        self.broadcast_packet(pet, &update);
        if increased {
            self.broadcast_packet(
                pet,
                &SocialAction {
                    object_id: pet.0,
                    action_id: LEVEL_UP_ACTION,
                },
            );
        }
        // Send a Server->Client packet PetInfo to the Player.
        self.update_and_broadcast_summon_status(pet, 1);
        increased
    }

    /// Java `Pet.deathPenalty()`: part of the level's exp is lost.
    fn pet_death_penalty(&mut self, pet: ObjectId) {
        // TODO in Java: "Need Correct Penalty".
        let level = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        let percent_lost = (-0.07 * f64::from(level)) + 6.5;
        // Calculate the Experience loss.
        let span = self.pet_exp_for_level(pet, level + 1) - self.pet_exp_for_level(pet, level);
        let lost_exp = java_round((span as f64 * percent_lost) / 100.0);
        // Get the Experience before applying penalty.
        let exp = self.summon(pet).map_or(0, |s| s.exp);
        if let Some(state) = self.pet_state_mut(pet) {
            state.exp_before_death = exp;
        }
        // Set the new Experience value of the Pet.
        self.pet_add_exp(pet, -lost_exp);
    }

    /// Java `Pet.restoreExp(restorePercent)`: the share of the lost exp that comes back.
    pub(crate) fn pet_restore_exp(&mut self, pet: ObjectId, restore_percent: f64) {
        let before = self.pet_state(pet).map_or(0, |p| p.exp_before_death);
        if before > 0 {
            let exp = self.summon(pet).map_or(0, |s| s.exp);
            self.pet_add_exp(pet, java_round(((before - exp) as f64 * restore_percent) / 100.0));
            if let Some(state) = self.pet_state_mut(pet) {
                state.exp_before_death = 0;
            }
        }
    }

    /// The player's exp and level changes that reach the pet (`PlayerStat.addExpAndSp`'s share and
    /// the level synchronisation): called with the exp a kill gave (after the bonuses); returns
    /// the `(exp, sp)` that stay with the owner.
    pub(crate) fn pet_take_owner_share(
        &mut self,
        owner: ObjectId,
        add_to_exp: f64,
        add_to_sp: f64,
    ) -> (f64, f64) {
        // if this player has a pet and it is in his range he takes from the owner's Exp, give the
        // pet Exp now
        let Some(pet) = self.summon_of(owner).filter(|s| self.is_pet(*s)) else {
            return (add_to_exp, add_to_sp);
        };
        let range = f64::from(self.settings.exp.alt_party_range);
        let near = match (self.position_of(owner), self.position_of(pet)) {
            (Some(a), Some(b)) => crate::target::distance_3d(a, b) < range,
            _ => false,
        };
        if !near {
            return (add_to_exp, add_to_sp);
        }
        let taken = self.pet_level_data_of(pet).map_or(0, |l| l.owner_exp_taken);
        let mut ratio = f64::from(taken as f32 / 100.0_f32);
        // only give exp/sp to the pet by taking from the owner if the pet has a non-zero,
        // positive ratio; allow possible customizations that would have the pet earning more than
        // 100% of the owner's exp/sp
        if ratio > 1.0 {
            ratio = 1.0;
        }
        if !self.is_dead(pet) {
            self.pet_add_exp_and_sp(pet, add_to_exp * (1.0 - ratio), add_to_sp * (1.0 - ratio));
        }
        // now adjust the max ratio to avoid the owner earning negative exp/sp
        (add_to_exp * ratio, add_to_sp * ratio)
    }

    /// The pet part of `PlayerStat.addLevel`: "Synchronize level with pet if possible".
    pub(crate) fn pet_sync_level_with_owner(&mut self, owner: ObjectId) {
        let Some(pet) = self.summon_of(owner).filter(|s| self.is_pet(*s)) else {
            return;
        };
        let owner_level = self.core_of(owner).map_or(1, |c| c.level);
        let Some((sync, max_level)) = self.pet_data_of(pet).map(|d| (d.sync_level, d.max_level())) else {
            return;
        };
        let level = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        if sync && level != owner_level {
            let available = max_level.min(owner_level);
            self.pet_set_level(pet, available);
            // `pet.getStat().getExpForLevel(availableLevel)`: the value is not used.
            self.pet_refresh_max_vitals(pet);
            let (hp, mp) = self
                .stats(pet)
                .map_or((0.0, 0.0), |v| (f64::from(v.max_hp()), f64::from(v.max_mp())));
            self.set_current_hp(pet, hp, true);
            self.set_current_mp(pet, mp, true);
            self.broadcast_packet(
                pet,
                &SocialAction {
                    object_id: owner.0,
                    action_id: LEVEL_UP_ACTION,
                },
            );
            self.update_and_broadcast_summon_status(pet, 1);
        }
    }

    // ------------------------------------------------------------------ the rows

    /// Java `Pet.storeMe()`: the `pets` row, and the collar shows the level.
    pub(crate) fn pet_store_me(&mut self, pet: ObjectId) {
        let Some(state) = self.summon(pet) else {
            return;
        };
        let Some(pet_state) = state.pet.as_ref() else {
            return;
        };
        let control = pet_state.control_object_id;
        let owner = state.owner;
        // "This is a summon, not a pet, don't store anything."
        if control.0 == 0 {
            return;
        }
        let restore = state.restore_summon && self.summons.pet_world.settings.restore_pet_on_reconnect;
        let (name, exp, sp) = (state.name.clone(), state.exp, state.sp);
        let insert = !pet_state.respawned;
        let fed = pet_state.cur_fed;
        let level = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        let (cur_hp, cur_mp) = self.core_of(pet).map_or((0.0, 0.0), |c| (c.cur_hp, c.cur_mp));
        let name = (!name.is_empty()).then_some(name);
        // The restore flag is the one the row keeps even when the config turns it off.
        self.outbox.persist(PersistCommand::Summon(SummonWrite::StorePet {
            insert,
            pet: PetRowWrite {
                control_object_id: control,
                owner,
                name: name.clone(),
                level,
                cur_hp: java_round(cur_hp) as i32,
                cur_mp: java_round(cur_mp) as i32,
                exp,
                sp,
                fed,
                restore,
            },
        }));
        self.summons.pet_world.no_row.remove(&control);
        self.summons.pet_world.rows.insert(
            control,
            SavedPet {
                control_object_id: control,
                name,
                level,
                cur_hp: java_round(cur_hp) as i32,
                cur_mp: java_round(cur_mp) as i32,
                exp,
                sp,
                fed,
                restore,
            },
        );
        if let Some(pet_state) = self.pet_state_mut(pet) {
            pet_state.respawned = true;
        }
        if restore {
            self.summons.pet_world.saved.insert(owner, control);
        } else {
            self.summons.pet_world.saved.remove(&owner);
        }
        // `itemInst.setEnchantLevel(level); itemInst.updateDatabase()` when they differ.
        if self
            .inventory_item(owner, control)
            .is_some_and(|i| i.enchant_level != level)
        {
            self.pet_set_control_enchant(owner, control, level);
            self.update_item_in_db(owner, control);
        }
    }

    /// Java `Pet.storeEffect(storeEffects)`: the buffs of the pet, kept for its next summon and
    /// written to `character_pet_skills_save`.
    pub(crate) fn pet_store_effect(&mut self, pet: ObjectId, store_effects: bool) {
        if !self.summons.settings.summon_store_skill_cooltime {
            return;
        }
        let Some(control) = self.pet_control_object_id(pet) else {
            return;
        };
        // Clear list for overwrite.
        if let Some(effects) = self.summons.pet_world.effects.get_mut(&control) {
            effects.clear();
        }
        let mut rows: Vec<SummonSkillSave> = Vec::new();
        if store_effects && let Some(list) = self.effect_list(pet) {
            let mut stored: HashSet<SkillKey> = HashSet::new();
            for info in list.effects() {
                // Do not save heals.
                if info.abnormal_type() == l2_data::stats::AbnormalType::LifeForceOthers {
                    continue;
                }
                if info.facts.toggle {
                    continue;
                }
                // Dances and songs are not kept in retail.
                if info.facts.dance && !self.settings.effects.alt_store_dances {
                    continue;
                }
                if !stored.insert(info.skill) {
                    continue;
                }
                rows.push(SummonSkillSave {
                    skill_id: info.skill.id,
                    skill_level: info.skill.level,
                    remaining_time: info.time(self.tick),
                });
            }
        }
        for row in &rows {
            self.summons
                .pet_world
                .effects
                .entry(control)
                .or_default()
                .push(SummonEffect {
                    skill: SkillKey::new(row.skill_id, row.skill_level),
                    time: row.remaining_time,
                });
        }
        self.write_pet_effect_rows(control, rows);
    }

    /// Java `Pet.restoreEffects()`: the buffs of the last summon of this collar come back (from
    /// memory, else from the saved rows), and the saved rows are deleted.
    pub(crate) fn pet_restore_effects(&mut self, pet: ObjectId) {
        let Some(control) = self.pet_control_object_id(pet) else {
            return;
        };
        if !self.summons.pet_world.effects.contains_key(&control) {
            let rows = self
                .summons
                .pet_world
                .effect_rows
                .get(&control)
                .cloned()
                .unwrap_or_default();
            let skills = self.skill_table();
            for row in rows {
                let Some(skill) = skills.as_ref().and_then(|t| t.get(row.skill_id, row.skill_level)) else {
                    continue;
                };
                // `skill.hasEffects(EffectScope.GENERAL)`.
                if !skill.effects.get(l2_data::skill::EffectScope::General).is_empty() {
                    self.summons
                        .pet_world
                        .effects
                        .entry(control)
                        .or_default()
                        .push(SummonEffect {
                            skill: SkillKey::new(row.skill_id, row.skill_level),
                            time: row.remaining_time,
                        });
                }
            }
        }
        self.write_pet_effect_rows(control, Vec::new());
        // `finally`: `applyPetEffects(this, getControlObjectId())`.
        let effects = self
            .summons
            .pet_world
            .effects
            .get(&control)
            .cloned()
            .unwrap_or_default();
        for effect in effects {
            self.apply_effects_timed(pet, pet, effect.skill, false, effect.time);
        }
    }

    /// The `DELETE` (and the rows) of `character_pet_skills_save` for one pet and the world's view
    /// of the table.
    fn write_pet_effect_rows(&mut self, control: ObjectId, rows: Vec<SummonSkillSave>) {
        if rows.is_empty() {
            self.summons.pet_world.effect_rows.remove(&control);
        } else {
            self.summons.pet_world.effect_rows.insert(control, rows.clone());
        }
        self.outbox
            .persist(PersistCommand::Summon(SummonWrite::PetEffects {
                control_object_id: control,
                effects: rows,
            }));
    }

    /// Java `Pet.stopSkillEffects(type, skillId)`: the saved buff of that skill goes too
    /// (`SummonEffectTable.removePetEffects`).
    pub(crate) fn pet_forget_saved_skill(&mut self, pet: ObjectId, skill_id: i32) {
        if let Some(control) = self.pet_control_object_id(pet)
            && let Some(effects) = self.summons.pet_world.effects.get_mut(&control)
        {
            effects.retain(|e| e.skill.id != skill_id);
        }
    }

    // ------------------------------------------------------------------ unsummon, death, decay

    /// Java `Pet.unSummon(owner)`: the feed stops, the summon is stored and leaves the world, and
    /// a living pet's inventory is forgotten.
    pub fn pet_unsummon(&mut self, pet: ObjectId, owner: Option<ObjectId>) {
        if !self.is_pet(pet) {
            return;
        }
        self.pet_stop_feed(pet);
        self.baby_on_unsummon(pet);
        let spawned = self.objects.get(&pet).is_some_and(|e| e.spawned);
        if spawned && !self.is_dead(pet) {
            // `Summon.unSummon`.
            self.stop_hp_mp_regeneration(pet);
            self.stop_follow(pet);
            if let Some(owner) = owner {
                self.send(
                    owner,
                    &PetDelete {
                        pet_type: SummonKind::Pet.summon_type(),
                        pet_object_id: pet.0,
                    },
                );
                self.summon_party_window_delete(pet, owner);
                if self.pet_item_count(owner) > 0 {
                    self.summons.pet_world.have_inv_items.insert(owner);
                    self.send(
                        owner,
                        &SystemMessage::new(
                            SystemMessageId::ThereAreItemsInYourPetInventoryRenderingYouUnableToSellTradeDropPetSummoningItemsPleaseEmptyYourPetInventory,
                        ),
                    );
                } else {
                    self.summons.pet_world.have_inv_items.remove(&owner);
                }
            }
            self.abort_attack(pet);
            self.abort_cast(pet);
            self.pet_store_me(pet);
            self.pet_store_effect(pet, true);
            if let Some(owner) = owner {
                self.set_pet(owner, None);
            }
            // Stop AI tasks.
            self.stop_summon_ai_task(pet);
            self.stop_all_effects(pet);
            self.decay(pet);
            self.set_creature_target(pet, None);
            if let Some(owner) = owner {
                self.disable_beast_auto_shots(owner);
            }
        }
        // `Pet.unSummon`: `if (!isDead()) { _inventory.deleteMe(); World.removePet(...) }`. The
        // items stay rows of the owner; the pet object goes.
        if !self.is_dead(pet) {
            self.forget_summon_object(pet);
        }
    }

    /// The part of `Pet.doDie(killer)` before `Summon.doDie(killer, true)`: the death penalty
    /// unless the owner duels or the pet is in a PvP zone outside a siege.
    pub(crate) fn pet_do_die_before(&mut self, pet: ObjectId) {
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        let in_duel = self.is_in_duel(owner);
        let pvp = self.is_inside_zone(pet, crate::zones::ZoneId::Pvp);
        let siege = self.is_inside_zone(pet, crate::zones::ZoneId::Siege);
        if !in_duel && (!pvp || siege) {
            self.pet_death_penalty(pet);
        }
    }

    /// The part of `Pet.doDie(killer)` after `Playable.doDie` succeeded: `Summon.doDie(killer,
    /// true)` (the killer's hate, no decay task), then the feed stops and the corpse waits.
    pub(crate) fn pet_do_die_after(&mut self, pet: ObjectId, killer: Option<ObjectId>) {
        let owner = self.summon_owner(pet);
        // "Retail monster AIs add 500 hate against the owner to the monster that finishes a
        // summon or pet."
        if let (Some(owner), Some(killer)) = (owner, killer)
            && self.is_attackable_npc(killer)
            && !self.is_dead(killer)
        {
            self.add_damage_hate(killer, owner, 0, 500);
        }
        self.pet_stop_feed(pet);
        self.baby_on_die(pet);
        self.pet_message(
            pet,
            SystemMessageId::ThePetHasBeenKilledIfYouDonTResurrectItWithin24HoursThePetSBodyWillDisappearAlongWithAllThePetSItems,
        );
        self.add_decay_task(pet);
    }

    /// Java `Pet.doRevive()`: the owner's request is over, the pet is back, eating, and idle.
    pub fn pet_do_revive(&mut self, pet: ObjectId) {
        if let Some(owner) = self.summon_owner(pet) {
            self.remove_reviving(owner);
        }
        // `Creature.doRevive()`.
        self.do_revive(pet);
        // stopDecay
        self.cancel_decay_task(pet);
        self.pet_start_feed(pet);
        if !self.pet_is_hungry(pet) {
            self.summon_set_running(pet, true);
        }
        self.set_intention_active(pet);
        self.baby_start_cast_task(pet);
    }

    /// Java `Pet.doRevive(revivePower)`: the lost exp comes back by the power, then the revive.
    pub fn pet_do_revive_with_power(&mut self, pet: ObjectId, revive_power: f64) {
        self.pet_restore_exp(pet, revive_power);
        self.pet_do_revive(pet);
    }

    /// Java `Pet.deleteMe(owner)` (a dead pet's corpse decayed): the items go back to the owner,
    /// the pet leaves, its collar is destroyed with its row.
    pub(crate) fn pet_delete_me(&mut self, pet: ObjectId, owner: Option<ObjectId>) {
        let control = self.pet_control_object_id(pet);
        // An owner who is not in the world: Java's retained `Player` object gets the items and
        // loses the collar, the database has to do the same (`pet_offline.rs`).
        let offline_owner = owner
            .is_none()
            .then(|| self.summon(pet).map(|s| s.owner))
            .flatten();
        // `_inventory.transferItemsToOwner()`.
        if let Some(owner) = owner {
            self.pet_transfer_items_to_owner(owner);
        }
        // `Summon.deleteMe(owner)`.
        self.pet_stop_feed(pet);
        self.baby_stop_cast_task(pet);
        if let Some(owner) = owner {
            self.send(
                owner,
                &PetDelete {
                    pet_type: SummonKind::Pet.summon_type(),
                    pet_object_id: pet.0,
                },
            );
            self.summon_party_window_delete(pet, owner);
        }
        self.cancel_decay_task(pet);
        self.decay(pet);
        if let Some(owner) = owner
            && self.summon_of(owner) == Some(pet)
        {
            self.set_pet(owner, None);
        }
        self.stop_summon_ai_task(pet);
        self.stop_all_effects_without_exclusions(pet, false, false);
        self.forget_summon_object(pet);
        // `destroyControlItem(owner, false)`, `CharSummonTable.getPets().remove(owner)`.
        if let (Some(owner), Some(control)) = (owner, control) {
            self.pet_destroy_control_item(owner, control);
            self.summons.pet_world.saved.remove(&owner);
        }
        if let (Some(owner), Some(control)) = (offline_owner, control) {
            self.pet_decayed_offline(owner, control);
            self.summons.pet_world.saved.remove(&owner);
        }
    }

    /// Java `Pet.destroyControlItem(owner, false)`: the collar leaves the inventory and the `pets`
    /// row goes.
    pub(crate) fn pet_destroy_control_item(&mut self, owner: ObjectId, control: ObjectId) {
        // The pet instance leaves the world's table (`World.removePet`): done by the caller.
        if self.inventory_item(owner, control).is_some() {
            let templates = self.item_templates();
            let item_id = self.inventory_item(owner, control).map_or(0, |i| i.item_id);
            if let Some(removed) = self.destroy_inventory_item(owner, control, 1) {
                self.send(
                    owner,
                    &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                        .with(crate::inventory::item_name_param(templates.as_deref(), item_id)),
                );
                let entry = self.update_entry(&removed, l2_protocol_game::server::InventoryUpdate::REMOVED);
                self.send_inventory_update(owner, entry);
                self.send_cur_load(owner);
                self.broadcast_user_info(owner);
            }
        } else {
            warn!("Couldn't destroy pet control item for {owner} pet: {control}");
        }
        // Pet control item no longer exists, delete the pet from the db.
        self.pet_row_deleted(control);
    }

    /// `DELETE FROM pets WHERE item_obj_id = ?` (also what `ItemManager.destroyItem` of any pet
    /// collar does) and the world's view of it.
    pub(crate) fn pet_row_deleted(&mut self, control: ObjectId) {
        self.summons.pet_world.rows.remove(&control);
        self.summons.pet_world.no_row.insert(control);
        self.summons.pet_world.effects.remove(&control);
        self.outbox
            .persist(PersistCommand::Summon(SummonWrite::DeletePet {
                control_object_id: control,
            }));
    }

    /// Java `PetData.getAvailableLevel(skillId, petLevel)` of the pet (the action buttons).
    pub fn pet_available_skill_level(&self, pet: ObjectId, skill_id: i32) -> i32 {
        let (Some(data), Some(table)) = (self.pet_data_of(pet), self.skill_table()) else {
            return 0;
        };
        let level = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        data.available_level(skill_id, level, &|id| table.max_level(id))
    }

    /// Java `PetClick.onAction(player, target, interact)`.
    pub(crate) fn pet_click(&mut self, player: ObjectId, pet: ObjectId, interact: bool) {
        // Aggression target lock effect.
        if self.is_locked_target(player) && self.locked_target(player) != Some(pet) {
            self.pet_message(player, SystemMessageId::FailedToChangeAttackTarget);
            return;
        }
        let is_owner = self.summon_owner(pet) == Some(player);
        if self.target_of(player) != Some(pet) {
            // Set the target of the Player player.
            self.set_target(player, Some(pet));
        } else if interact {
            // Check if the pet is attackable (without a forced attack) and isn't dead.
            if self.summon_auto_attackable_by(pet, player) && !is_owner {
                self.set_intention_attack(player, pet);
                self.on_action_request(player);
            } else if !self.pet_inside_radius_2d(pet, player, 150) {
                self.set_intention_interact(player, pet);
                self.on_action_request(player);
            } else {
                if is_owner {
                    self.send(
                        player,
                        &l2_protocol_game::server::PetStatusShow {
                            summon_type: SummonKind::Pet.summon_type(),
                        },
                    );
                    // ON_PLAYER_SUMMON_TALK (S-API-04, queued).
                    self.script_fire_async(
                        crate::script::ScriptEvent::PlayerSummonTalk { summon: pet },
                        Some(pet),
                    );
                }
                self.update_not_move_until(player);
            }
        }
    }

    /// `Creature.isInsideRadius2D(other, radius)`: strictly inside.
    fn pet_inside_radius_2d(&self, a: ObjectId, b: ObjectId, radius: i32) -> bool {
        match (self.position_of(a), self.position_of(b)) {
            (Some(a), Some(b)) => crate::target::distance_2d(a, b) < f64::from(radius),
            _ => false,
        }
    }

    /// Java `Player.onActionRequest()`: the pet that was stored for a restore comes back at the
    /// first action. The servitor restore is not modelled.
    pub(crate) fn on_action_request(&mut self, player: ObjectId) {
        if self.is_spawn_protected(player) {
            self.pet_on_action_request(player);
            self.set_spawn_protection(player, false);
            if !self.is_inside_zone(player, crate::zones::ZoneId::Peace) {
                self.send(
                    player,
                    &l2_protocol_game::server::SystemMessage::new(
                        l2_protocol_game::SystemMessageId::YouAreNoLongerProtectedFromAggressiveMonsters,
                    ),
                );
            }
        }
        // The teleport protection ends with the first action.
        if self.is_teleport_protected(player) {
            self.set_teleport_protection(player, false);
            if !self.is_inside_zone(player, crate::zones::ZoneId::Peace) {
                self.send_message(player, "Teleport spawn protection ended.");
            }
        }
    }

    // ------------------------------------------------------------------ the owner

    /// Whether the player's pet inventory kept items when it was unsummoned (`havePetInvItems`):
    /// the collar can then be neither traded nor dropped.
    pub fn have_pet_inv_items(&self, owner: ObjectId) -> bool {
        self.summons.pet_world.have_inv_items.contains(&owner)
    }

    /// The pet part of the login: the rows of the character's collars become the world's view of
    /// the tables, and a pet that was stored for a restore comes back at the first action.
    pub(crate) fn take_saved_pets(&mut self, owner: ObjectId) {
        // Java `Player.restorePetInventoryItems`: one `PET` or `PET_EQUIP` row of the owner means
        // the collar cannot be traded, sold, deposited or mailed (`havePetInvItems`).
        let has_pet_items = self.pet_item_count(owner) > 0;
        if has_pet_items {
            self.summons.pet_world.have_inv_items.insert(owner);
        } else {
            self.summons.pet_world.have_inv_items.remove(&owner);
        }
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let rows = std::mem::take(&mut player.data.profile.pet_rows);
        let effects = std::mem::take(&mut player.data.profile.pet_effects);
        let mut restore = None;
        // Every collar the character holds without a row is known to make a new pet.
        let collars: Vec<ObjectId> = {
            let templates = self.item_templates();
            self.players
                .get(&owner)
                .map(|p| {
                    p.data
                        .items
                        .iter()
                        .filter(|i| {
                            templates
                                .as_deref()
                                .and_then(|t| t.get(i.item_id))
                                .is_some_and(|t| t.is_pet_item())
                        })
                        .map(|i| i.object_id)
                        .collect()
                })
                .unwrap_or_default()
        };
        self.summons.pet_world.no_row.extend(collars);
        for row in rows {
            self.summons.pet_world.no_row.remove(&row.control_object_id);
            if row.restore && restore.is_none() {
                restore = Some(row.control_object_id);
            }
            self.summons.pet_world.rows.insert(row.control_object_id, row);
        }
        for effect in effects {
            self.summons
                .pet_world
                .effect_rows
                .entry(effect.control_object_id)
                .or_default()
                .push(SummonSkillSave {
                    skill_id: effect.skill_id,
                    skill_level: effect.skill_level,
                    remaining_time: effect.remaining_time,
                });
        }
        // `RestorePetOnReconnect`: `CharSummonTable._pets`. The pet is brought back at the first
        // action, `isSpawnProtected()` in Java.
        if self.summons.pet_world.settings.restore_pet_on_reconnect
            && let Some(control) = restore
        {
            self.summons.pet_world.saved.insert(owner, control);
        } else {
            self.summons.pet_world.saved.remove(&owner);
        }
        // `EnterWorld`: `if (PLAYER_SPAWN_PROTECTION > 0) player.setSpawnProtection(true)`.
        if self.summons.pet_world.settings.player_spawn_protection > 0 {
            self.set_spawn_protection(owner, true);
        }
    }

    /// The pet part of `Player.onActionRequest()`: a pet stored for a restore comes back
    /// (`CharSummonTable.restorePet`).
    pub fn pet_on_action_request(&mut self, owner: ObjectId) {
        // `isSpawnProtected()`: the protection ends with the first action or its time.
        if !self.is_spawn_protected(owner) {
            return;
        }
        if self.summon_of(owner).is_some() {
            return;
        }
        let Some(control) = self.summons.pet_world.saved.get(&owner).copied() else {
            return;
        };
        self.restore_pet_of(owner, control);
    }

    /// Java `CharSummonTable.restorePet(player)`.
    fn restore_pet_of(&mut self, owner: ObjectId, control: ObjectId) {
        let Some(item) = self.inventory_item(owner, control).cloned() else {
            warn!("CharSummonTable: Null pet summoning item for: {owner}");
            return;
        };
        let known = self
            .pet_table()
            .is_some_and(|t| t.pet_data_by_item_id(item.item_id).is_some());
        if !known {
            warn!("CharSummonTable: Null pet data for: {owner} and summoning item: {control}");
            return;
        }
        if self.summon_pet(owner, control).is_none() {
            warn!("CharSummonTable: Null pet instance for: {owner}");
        }
    }

    /// Whether a living pet stands for the owner's `Player.deleteMe`: handled by
    /// `summon_on_owner_leave`; the pet's feed stops with its unsummon.
    pub(crate) fn pet_forget_owner(&mut self, owner: ObjectId) {
        self.summons.pet_world.pending_collar.remove(&owner);
        self.summons.pet_world.row_pending.remove(&owner);
        self.summons.pet_world.have_inv_items.remove(&owner);
        self.summons.pet_world.saved.remove(&owner);
    }

    // ------------------------------------------------------------------ weight

    /// Java `Pet.refreshOverloaded()`: the weight penalty skill of the pet's load.
    pub(crate) fn pet_refresh_overloaded(&mut self, pet: ObjectId) {
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        let Some(view) = self.stats(pet) else {
            return;
        };
        let max_load = view.max_load();
        if max_load <= 0 {
            return;
        }
        let load = self.pet_total_weight(owner);
        let bonus = view.bonus_weight_penalty();
        let weight_proc = i64::from(load - bonus) * 1000 / i64::from(max_load);
        let new_penalty = if weight_proc < 500 {
            0
        } else if weight_proc < 666 {
            1
        } else if weight_proc < 800 {
            2
        } else if weight_proc < 1000 {
            3
        } else {
            4
        };
        // `getOwner().getDietMode()` is not modelled.
        let old = self.pet_state(pet).map_or(0, |p| p.weight_penalty);
        if old == new_penalty {
            return;
        }
        if let Some(state) = self.pet_state_mut(pet) {
            state.weight_penalty = new_penalty;
            state.overloaded = new_penalty > 0 && load >= max_load;
        }
        if new_penalty > 0 {
            self.add_skill(pet, WEIGHT_PENALTY_SKILL, new_penalty);
        } else {
            self.remove_skill(pet, WEIGHT_PENALTY_SKILL, true);
        }
    }

    /// `PetItemList` of the owner's pet inventory (`new PetItemList(pet.getInventory().getItems())`).
    pub(crate) fn pet_item_list(&self, owner: ObjectId) -> PetItemList {
        PetItemList {
            items: self
                .pet_items(owner)
                .map(|item| self.update_entry(item, 0).item)
                .collect(),
        }
    }
}
