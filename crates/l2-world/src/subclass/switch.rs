//! `Player.addSubClass`, `modifySubClass` and `setActiveClass` (the two halves of the switch, see
//! the module notes of `subclass/`).
// Java: entity/actor/Player.java (addSubClass, modifySubClass, setActiveClass, setClassTemplate,
//       rewardSkills, regiveTemporarySkills, restoreDeathPenaltyBuffLevel, restoreRecipeBook)
// Java: entity/actor/holders/player/{SubClassHolder,Shortcuts}.java (restoreMe)

use l2_data::ClassId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use tracing::warn;

use super::{AfterSwitch, ClassIndexLoad, ClassStats, SubClassHolder, SubclassWrite};
use crate::outbox::PersistCommand;
use crate::player::SkillData;
use crate::residence::ResidenceRef;
use crate::world::World;

/// Java `SocialAction.LEVEL_UP`.
const LEVEL_UP_ACTION: i32 = 2122;
/// Java `Player.DEATH_PENALTY_SKILL_ID` (`restoreDeathPenaltyBuffLevel`).
const DEATH_PENALTY_SKILL_ID: i32 = 5076;
/// `addSubClass` stores the skills of the class tree up to this level.
const NEW_SUBCLASS_SKILL_LEVEL: i32 = 40;

/// The second half of a switch: the class index whose rows were asked for and what the caller
/// sends afterwards.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingSwitch {
    pub(crate) class_index: i32,
    pub(crate) after: Vec<AfterSwitch>,
}

impl World {
    /// Java `isChangingClass()` (`_subclassLock`).
    pub fn is_changing_class(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.subclass.lock)
    }

    /// Java `isSubClassActive()` (`_classIndex > 0`).
    pub fn is_subclass_active(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.data.profile.class_index > 0)
    }

    /// Java `getSubClasses().values()`, in class index order.
    pub fn sub_classes(&self, id: ObjectId) -> Vec<SubClassHolder> {
        self.players.get(&id).map_or_else(Vec::new, |p| {
            p.data.profile.subclasses.subs.values().copied().collect()
        })
    }

    /// Java `getTotalSubClasses()`.
    pub fn total_sub_classes(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| {
            i32::try_from(p.data.profile.subclasses.subs.len()).unwrap_or(i32::MAX)
        })
    }

    /// Java `SubClassHolder.MAX_LEVEL`: `MaxSubclassLevel`, unless the experience table ends
    /// before it.
    pub(crate) fn subclass_max_level(&self) -> i32 {
        let max_subclass_level = self.settings.subclass.max_subclass_level;
        match self.java_max_level() {
            Some(max_level) if max_subclass_level >= max_level => max_level - 1,
            _ => max_subclass_level,
        }
    }

    /// Java `SubClassHolder.setLevel(level)`: between `BaseSubclassLevel` and the maximum.
    pub(crate) fn subclass_clamp_level(&self, level: i32) -> i32 {
        let max = self.subclass_max_level();
        if level > max {
            max
        } else if level < self.settings.subclass.base_subclass_level {
            self.settings.subclass.base_subclass_level
        } else {
            level
        }
    }

    /// Java `SubClassHolder.setExp(exp)`: never past the last exp of the maximum level.
    pub(crate) fn subclass_cap_exp(&self, exp: i64) -> i64 {
        match self.exp_for_level(self.subclass_max_level() + 1) {
            Some(next) if exp > next - 1 => next - 1,
            _ => exp,
        }
    }

    /// Java `Player.restore` -> `restoreSubClassData`: the holders get Java's setters (the level
    /// between `BaseSubclassLevel` and the maximum, the exp capped), and an active subclass shows
    /// them. Called when the player enters, before the stats are built.
    pub(crate) fn subclass_on_enter(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let holders: Vec<SubClassHolder> = player.data.profile.subclasses.subs.values().copied().collect();
        let fixed: Vec<SubClassHolder> = holders
            .into_iter()
            .map(|h| SubClassHolder {
                level: self.subclass_clamp_level(h.level),
                exp: self.subclass_cap_exp(h.exp),
                ..h
            })
            .collect();
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let active = player.data.profile.class_index;
        for holder in fixed {
            if holder.class_index == active {
                player.data.level = holder.level;
                player.data.exp = holder.exp;
                player.core.level = holder.level;
            }
            player
                .data
                .profile
                .subclasses
                .subs
                .insert(holder.class_index, holder);
        }
    }

    // ------------------------------------------------------------------ addSubClass

    /// Java `Player.addSubClass(classId, classIndex)`: a new holder at `class_index` with the
    /// start level, its row and the skills of its class tree up to level 40 (stored under the new
    /// index, not learned now). The active class does not change. `false` while the class lock is
    /// on, with all subclasses taken, for index 0 or an index in use.
    // NOTE: Java returns false when the `INSERT` fails; the world does not wait for the database
    // (D-15), so the holder is added and a failed insert is only logged by the persistence task.
    pub fn add_sub_class(&mut self, id: ObjectId, class_id: i32, class_index: i32) -> bool {
        let Some(player) = self.players.get(&id) else {
            return false;
        };
        if player.subclass.lock {
            return false;
        }
        if self.total_sub_classes(id) == self.settings.subclass.max_subclass || class_index == 0 {
            return false;
        }
        if player.data.profile.subclasses.subs.contains_key(&class_index) {
            return false;
        }
        let level = self.settings.subclass.base_subclass_level;
        let holder = SubClassHolder {
            class_id,
            exp: self.exp_for_level(level).unwrap_or(0),
            sp: 0,
            level,
            class_index,
        };
        let skills = self.new_subclass_skills(class_id);
        self.outbox.persist(PersistCommand::Subclass(SubclassWrite::Add {
            char_id: id.0,
            holder,
            skills,
        }));
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.subclasses.subs.insert(class_index, holder);
        }
        true
    }

    /// The skills `addSubClass` stores: every skill of the class's complete tree learned up to
    /// level 40, at the highest such level that exists (Java keeps the higher of two levels it
    /// meets, so the last row of a skill has the highest level).
    // NOTE: Java throws on a tree skill that `SkillData` does not have; it is skipped here.
    fn new_subclass_skills(&self, class_id: i32) -> Vec<SkillData> {
        let Some(class) = u16::try_from(class_id).ok().map(ClassId) else {
            return Vec::new();
        };
        let table = self.skill_table();
        let mut skills: Vec<SkillData> = Vec::new();
        for learn in self.rewards.skill_trees.complete_class_tree(class) {
            if learn.get_level > NEW_SUBCLASS_SKILL_LEVEL {
                continue;
            }
            if table
                .as_ref()
                .is_none_or(|t| t.get(learn.skill_id, learn.skill_level).is_none())
            {
                continue;
            }
            match skills.iter_mut().find(|s| s.skill_id == learn.skill_id) {
                Some(known) if known.skill_level > learn.skill_level => {}
                Some(known) => known.skill_level = learn.skill_level,
                None => skills.push(SkillData {
                    skill_id: learn.skill_id,
                    skill_level: learn.skill_level,
                }),
            }
        }
        skills.sort_by_key(|s| s.skill_id);
        skills
    }

    // ------------------------------------------------------------------ modifySubClass

    /// Java `Player.modifySubClass(classIndex, newClassId)`: everything of the subclass at
    /// `class_index` goes (its holder, its hennas, shortcuts, saved buffs, skills and row), then
    /// `addSubClass(newClassId, classIndex)`. `false` without a subclass there or when the add
    /// fails.
    pub fn modify_sub_class(&mut self, id: ObjectId, class_index: i32, new_class_id: i32) -> bool {
        let Some(player) = self.players.get_mut(&id) else {
            return false;
        };
        if player.data.profile.subclasses.subs.remove(&class_index).is_none() {
            return false;
        }
        // ON_PLAYER_PROFESSION_CANCEL: script event (D-05).
        let henna_slots = self.active_henna_slots(id);
        self.outbox
            .persist(PersistCommand::Subclass(SubclassWrite::Delete {
                char_id: id.0,
                class_index,
                henna_slots,
            }));
        if !self.add_sub_class(id, new_class_id, class_index) {
            return false;
        }
        // Java's `PlayerStat` reads the holder of the active index: when that is the one replaced,
        // the character shows the new holder's level, exp and sp at once.
        if let Some(player) = self.players.get_mut(&id)
            && player.data.profile.class_index == class_index
            && let Some(holder) = player.data.profile.subclasses.subs.get(&class_index).copied()
        {
            player.data.level = holder.level;
            player.data.exp = holder.exp;
            player.data.sp = holder.sp;
            player.core.level = holder.level;
        }
        true
    }

    /// The slots (1 to 3) where the active class wears a dye the datapack knows (Java
    /// `getHenna(slot) != null`).
    fn active_henna_slots(&self, id: ObjectId) -> Vec<i32> {
        let Some(player) = self.players.get(&id) else {
            return Vec::new();
        };
        let known = |dye_id: i32| {
            dye_id != 0
                && self
                    .data
                    .stat_data()
                    .is_some_and(|d| d.hennas.get(dye_id).is_some())
        };
        (1..=3)
            .filter(|slot| {
                usize::try_from(slot - 1)
                    .ok()
                    .and_then(|i| player.data.profile.hennas.get(i))
                    .is_some_and(|dye| known(*dye))
            })
            .collect()
    }

    // ------------------------------------------------------------------ setActiveClass

    /// Java `Player.setActiveClass(classIndex)`: switches to the base class (0) or a subclass. The
    /// rows of the new class index are read first, so the switch finishes when they come back.
    pub fn set_active_class(&mut self, id: ObjectId, class_index: i32) {
        self.set_active_class_then(id, class_index, Vec::new());
    }

    /// [`World::set_active_class`], with what the caller sends after it.
    pub(crate) fn set_active_class_then(&mut self, id: ObjectId, class_index: i32, after: Vec<AfterSwitch>) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.subclass.lock {
            self.flush_after_switch(id, after);
            return;
        }
        // Cannot switch or change subclasses while transformed.
        if self.is_transformed(id) {
            self.flush_after_switch(id, after);
            return;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.subclass.lock = true;
        }
        // "Remove active item skills before saving char to database": the augmentation bonuses
        // come off with the other item skills in the second half (`reloadEquippedItems` puts
        // them back), which the store in between does not see.
        self.abort_cast(id);
        if self.is_channelized(id) {
            self.abort_channelization(id);
        }
        // 1. Call store() before modifying _classIndex to avoid skill effects rollover.
        let store_effects = self.settings.subclass.subclass_store_skill_cooltime;
        if let Some(snapshot) = self.store_snapshot_with(id, store_effects) {
            self.outbox
                .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
        }
        self.reset_time_stamps(id);
        // Clear charges (`_charges.set(0)`, no packet).
        if let Some(control) = self.control_mut(id) {
            control.charges = 0;
        }
        self.stop_charge_task(id);
        if self.has_servitor(id)
            && let Some(summon) = self.summon_of(id)
        {
            self.unsummon(summon, Some(id));
        }
        let known = class_index == 0
            || self
                .players
                .get(&id)
                .is_some_and(|p| p.data.profile.subclasses.subs.contains_key(&class_index));
        if !known {
            let name = self
                .players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            warn!("Could not switch {name}'s sub class to class index {class_index}: no such sub class");
            if let Some(p) = self.players.get_mut(&id) {
                p.subclass.lock = false;
            }
            self.flush_after_switch(id, after);
            return;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.subclass.pending = Some(PendingSwitch { class_index, after });
        }
        // The rest needs the rows of the new class index (D-15); the request is queued after the
        // store, so the rows include it.
        self.outbox.persist(PersistCommand::ClassIndexRequest {
            owner: id,
            class_index,
        });
    }

    /// The answer to `PersistCommand::ClassIndexRequest`: the second half of `setActiveClass`.
    pub(crate) fn on_class_index_loaded(&mut self, id: ObjectId, class_index: i32, load: ClassIndexLoad) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let Some(pending) = player.subclass.pending.take_if(|p| p.class_index == class_index) else {
            warn!("{id}: rows of class index {class_index} came without a class change waiting for them");
            return;
        };
        self.switch_class_values(id, class_index);
        self.finish_class_switch(id, load);
        if let Some(p) = self.players.get_mut(&id) {
            p.subclass.lock = false;
        }
        self.flush_after_switch(id, pending.after);
    }

    /// `setClassTemplate` and `_classIndex = classIndex`: the class, the class index and the level,
    /// exp and sp of the new class (the old ones go back to their holder or to the base values),
    /// the learning class and the party level.
    fn switch_class_values(&mut self, id: ObjectId, class_index: i32) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let data = &mut player.data;
        let current = ClassStats {
            level: player.core.level,
            exp: data.exp,
            sp: data.sp,
        };
        let old_index = data.profile.class_index;
        let subs = &mut data.profile.subclasses;
        if old_index == 0 {
            subs.base = Some(current);
        } else if let Some(holder) = subs.subs.get_mut(&old_index) {
            holder.level = current.level;
            holder.exp = current.exp;
            holder.sp = current.sp;
        }
        let (class_id, next) = if class_index == 0 {
            (data.base_class, subs.base.take().unwrap_or(current))
        } else {
            match subs.subs.get(&class_index) {
                Some(h) => (
                    h.class_id,
                    ClassStats {
                        level: h.level,
                        exp: h.exp,
                        sp: h.sp,
                    },
                ),
                None => (data.class_id, current),
            }
        };
        data.class_id = class_id;
        data.level = next.level;
        data.exp = next.exp;
        data.sp = next.sp;
        data.profile.class_index = class_index;
        player.core.level = next.level;
        // `setLearningClass(getPlayerClass())`.
        player.talk.learning_class = u16::try_from(class_id).ok().map(ClassId);
        // ON_PLAYER_PROFESSION_CHANGE: script event (D-05).
        if self.party_of(id).is_some() {
            self.recalculate_party_level_of(id);
        }
    }

    /// The rest of `setActiveClass`, in Java's order, with the rows of the new class index.
    fn finish_class_switch(&mut self, id: ObjectId, load: ClassIndexLoad) {
        // 3. Remove all existing skills.
        let known: Vec<i32> = self
            .players
            .get(&id)
            .map_or_else(Vec::new, |p| p.core.skills.iter().map(|(s, _)| *s).collect());
        for skill_id in known {
            self.remove_skill(id, skill_id, true);
        }
        self.stop_all_effects_except_those_that_last_through_death(id);
        self.stop_all_effects_not_stay_on_subclass_change(id);
        self.stop_all_toggles(id, true);
        self.stop_cubics(id);
        self.restore_dwarven_recipe_book(id, &load.dwarven_recipes);

        // Restore any Death Penalty Buff.
        self.restore_death_penalty_buff_level(id);

        // 4. Restore all the learned skills for the current class from the database.
        if let Some(player) = self.players.get_mut(&id) {
            player.data.skills = load.skills.clone();
        }
        for skill in &load.skills {
            if !self.add_skill(id, skill.skill_id, skill.skill_level) {
                warn!(
                    "Skipped null skill Id: {} Level: {} while restoring player skills for playerObjId: {id}",
                    skill.skill_id, skill.skill_level
                );
            }
        }
        self.reward_skills(id);
        self.regive_temporary_skills(id);
        // `getInventory().applyItemSkills()`: the passive skills of the worn items came back with
        // `reloadEquippedItems` above.

        // Prevents some issues when changing between subclases that shares skills.
        self.reset_disabled_skills(id);

        // 5. Restore effect/buff data for the new class.
        self.apply_saved_effect_rows(id, load.effects);
        self.install_servitor_effects(id, &load.summon_effects);
        self.send_etc_status_update(id);
        // The `Q00422_RepentYourSins` exit is the script engine's (S-API-01).

        // 6. Restore henna data for the class.
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.hennas = load.hennas;
        }
        let henna = self.henna_info(id);
        self.send(id, &henna);

        // 7. Reset HP/MP/CP stats and send the character status.
        self.refresh_max_vitals(id, false);
        if let Some(view) = self.stats(id) {
            let (max_hp, max_mp, max_cp) = (
                f64::from(view.max_hp()),
                f64::from(view.max_mp()),
                f64::from(view.max_cp()),
            );
            let (hp, mp, cp) = self
                .players
                .get(&id)
                .map_or((0.0, 0.0, 0.0), |p| (p.core.cur_hp, p.core.cur_mp, p.core.cur_cp));
            if hp > max_hp {
                self.set_current_hp(id, max_hp, true);
            }
            if mp > max_mp {
                self.set_current_mp(id, max_mp, true);
            }
            if cp > max_cp {
                self.set_current_cp(id, max_cp, true);
            }
        }
        self.refresh_overloaded(id);
        self.refresh_expertise_penalty(id);
        self.broadcast_user_info(id);

        // Clear resurrect xp calculation.
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.exp_before_death = 0;
        }

        // 8. Restore shortcut data related to this class.
        self.restore_class_shortcuts(id, load.shortcuts);
        // 9. Resend a class change animation effect to broadcast to all nearby players.
        self.broadcast_creature_packet(
            id,
            &SocialAction {
                object_id: id.0,
                action_id: LEVEL_UP_ACTION,
            },
        );
        let cool_time = self.skill_cool_time(id);
        self.send(id, &cool_time);
        if let Some(player) = self.players.get(&id) {
            let is_gm = self.data.access_level(player.data.access_level).is_gm;
            let packet = crate::enter::storage_max_count(&player.data, is_gm, &self.settings);
            self.send(id, &packet);
        }
    }

    /// Java `restoreRecipeBook(false)`: the dwarven book becomes the recipes of the class index.
    fn restore_dwarven_recipe_book(&mut self, id: ObjectId, recipes: &[i32]) {
        if let Some(player) = self.players.get_mut(&id) {
            player.craft.dwarven.clear();
        }
        let Some(data) = self.recipe_data() else {
            if !recipes.is_empty() {
                warn!("Could not restore recipe book data: no recipes are loaded.");
            }
            return;
        };
        for &recipe_id in recipes {
            // NOTE(java-quirk): an unknown recipe throws in `registerDwarvenRecipeList` and the
            // catch ends the read, so the recipes after it stay out of the book.
            if data.get(recipe_id).is_none() {
                warn!("Could not restore recipe book data: unknown recipe {recipe_id}.");
                break;
            }
            self.register_dwarven_recipe_list(id, recipe_id, false);
        }
    }

    /// Java `restoreDeathPenaltyBuffLevel()`: the penalty skill of the level the character has.
    fn restore_death_penalty_buff_level(&mut self, id: ObjectId) {
        let level = self
            .players
            .get(&id)
            .map_or(0, |p| p.data.profile.death_penalty_level);
        if level > 0 {
            self.add_skill(id, DEATH_PENALTY_SKILL_ID, level);
        }
    }

    /// Java `regiveTemporarySkills()`: the skills that are not stored (noble, hero, clan, siege,
    /// residence), the skills of the worn items and the death penalty.
    fn regive_temporary_skills(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        // Add noble skills if noble (`setNoble(true)`).
        if player.data.profile.noble {
            let trees = std::sync::Arc::clone(&self.rewards.skill_trees);
            for skill in trees.noble_skills() {
                self.add_skill(id, skill.skill_id, skill.skill_level);
            }
            self.send_skill_list(id);
        }
        // Add Hero skills if hero (on the base class only, `setHero`).
        if self.is_hero(id) {
            self.set_hero(id, true);
        }
        // Add clan skills.
        let clan_id = self.clan_id_of(id);
        if clan_id != 0 {
            self.clan_add_skill_effects(clan_id, id);
            let min_level = self.siege_settings().siege_clan_min_level;
            if self.clan(clan_id).is_some_and(|c| c.level >= min_level) && self.is_clan_leader(id) {
                self.add_siege_skills(id);
            }
            if let Some(castle) = self.castle_by_owner(clan_id) {
                self.give_residential_skills(ResidenceRef::Castle(castle), id);
            }
            if let Some(fort) = self.fort_by_owner(clan_id) {
                self.give_residential_skills(ResidenceRef::Fort(fort), id);
            }
        }
        // Reload passive skills from armors / jewels / weapons.
        self.reload_equipped_item_skills(id);
        // Add Death Penalty Buff Level.
        self.restore_death_penalty_buff_level(id);
    }

    /// The servitor buffs of the new class index join the world's view of the table (Java reads
    /// them when a servitor of the same skill spawns).
    fn install_servitor_effects(
        &mut self,
        id: ObjectId,
        rows: &[crate::summon::servitor::SavedSummonEffect],
    ) {
        let Some(class_index) = self.players.get(&id).map(|p| p.data.profile.class_index) else {
            return;
        };
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.summon_effects = rows.to_vec();
        }
        self.take_class_summon_effects(id, class_index);
    }

    /// `Shortcuts.restoreMe()`: the shortcuts of the class index; an item shortcut whose item is
    /// not in the inventory goes (`deleteShortcut`, with its packets), then `ShortcutInit`.
    fn restore_class_shortcuts(&mut self, id: ObjectId, shortcuts: Vec<crate::player::ShortcutData>) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.data.shortcuts = shortcuts;
        let missing: Vec<(i32, i32)> =
            player
                .data
                .shortcuts
                .iter()
                .filter(|s| s.kind == crate::shortcuts::TYPE_ITEM)
                .filter(|s| {
                    !player.data.items.iter().any(|i| {
                        i.object_id.0 == s.shortcut_id && (i.loc == "INVENTORY" || i.loc == "PAPERDOLL")
                    })
                })
                .map(|s| (s.slot, s.page))
                .collect();
        for (slot, page) in missing {
            self.delete_shortcut(id, slot, page);
        }
        if let Some(player) = self.players.get(&id) {
            let init = crate::enter::shortcut_init(&player.data, &*self.data);
            self.send(id, &init);
        }
    }

    /// Sends what the caller of `setActiveClass` sends after it.
    pub(crate) fn flush_after_switch(&mut self, id: ObjectId, after: Vec<AfterSwitch>) {
        for item in after {
            match item {
                AfterSwitch::Message(message) => {
                    self.send(id, &l2_protocol_game::server::SystemMessage::new(message));
                }
                AfterSwitch::Text(text) => self.send_message(id, &text),
                AfterSwitch::Html(html) => {
                    self.send_html(id, &html);
                }
            }
        }
    }
}
