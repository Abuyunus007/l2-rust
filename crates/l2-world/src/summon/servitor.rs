//! A servitor's life: the 5 s life task (time and the item it eats), unsummoning, dying and
//! decaying, the owner leaving or teleporting, and the rows Java keeps for it.
// Java: entity/actor/Summon.java (unSummon, deleteMe, doDie, onDecay, stopAllEffects)
// Java: entity/actor/instance/Servitor.java (run, unSummon, doDie, storeMe, storeEffect,
//       restoreEffects, stopSkillEffects)
// Java: entity/actor/Player.java (deleteMe's summon part, onTeleported's summon part)
// Java: data/sql/CharSummonTable.java (saveSummon, removeServitor)
// Java: data/sql/SummonEffectTable.java (the servitor half)
//
// # Rows (D-39)
// `character_summons` (`CharSummonTable`) and `character_summon_skills_save` (`Servitor`'s
// `storeEffect` / `restoreEffects`) are written through [`PersistCommand::Summon`] in Java's
// order. Java reads `character_summon_skills_save` when a servitor of the same skill spawns and
// the memory table has no entry; here the rows of the character are read at load
// (`PlayerProfile::summon_effects`) and kept as the world's view of the table (`saved_rows`),
// updated with every write, so the read finds what Java's `SELECT` would.

use std::collections::HashSet;

use l2_data::stats::AbnormalType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{PetDelete, SetSummonRemainTime, SystemMessage};
use l2_util::ObjectId;

use super::{SummonEffect, SummonTimer};
use crate::effects::SkillKey;
use crate::outbox::PersistCommand;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `Servitor.run()`: `final int usedtime = 5000;` and the fixed rate of the task.
const LIFE_TASK_MS: i32 = 5000;

/// One row of `character_summon_skills_save` (without the owner, class index and reference
/// skill, which are the command's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonSkillSave {
    pub skill_id: i32,
    pub skill_level: i32,
    /// `remaining_time`: `BuffInfo.getTime()`.
    pub remaining_time: i32,
}

/// A saved `character_summon_skills_save` row as the loader read it (the reference skill is
/// part of the row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedSummonEffect {
    /// `summonSkillId`.
    pub summon_skill_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    pub remaining_time: i32,
}

/// The summon table writes (Java's SQL, in order with the character's other commands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummonWrite {
    /// `CharSummonTable.saveSummon`: `REPLACE INTO character_summons
    /// (ownerId,summonSkillId,curHp,curMp,time) VALUES (?,?,?,?,?)`.
    SaveServitor {
        owner: ObjectId,
        summon_skill_id: i32,
        cur_hp: i32,
        cur_mp: i32,
        time: i32,
    },
    /// `CharSummonTable.removeServitor`: `DELETE FROM character_summons WHERE ownerId = ?`.
    RemoveServitor { owner: ObjectId },
    /// `Servitor.storeEffect` / `restoreEffects`: `DELETE FROM character_summon_skills_save WHERE
    /// ownerId=? AND ownerClassIndex=? AND summonSkillId=?`, then one `REPLACE INTO` per row
    /// (`buff_index` counts from 1).
    ServitorEffects {
        owner: ObjectId,
        class_index: i32,
        summon_skill_id: i32,
        effects: Vec<SummonSkillSave>,
    },
}

impl World {
    // ------------------------------------------------------------------ the life task

    /// Java `Servitor.run()`: every 5 s the life time goes down, the servitor eats its item, the
    /// owner sees the time left.
    pub(crate) fn on_servitor_life_timer(&mut self, summon: ObjectId) {
        let spawned = self.objects.get(&summon).is_some_and(|e| e.spawned);
        let dead = self.is_dead(summon);
        let Some(servitor) = self.summon_mut(summon).and_then(|s| s.servitor.as_mut()) else {
            return;
        };
        servitor.life_timer = None;
        servitor.life_time_remaining = servitor.life_time_remaining.saturating_sub(LIFE_TASK_MS);
        if dead || !spawned {
            // `_summonLifeTask.cancel(false)`.
            return;
        }
        // The fixed rate: the next run.
        let timer = self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(LIFE_TASK_MS as u64),
            TimerKind::Summon(SummonTimer::Life(summon)),
        );
        let Some(servitor) = self.summon_mut(summon).and_then(|s| s.servitor.as_mut()) else {
            return;
        };
        servitor.life_timer = Some(timer);
        let owner = self.summon_owner(summon);

        // Check if the summon's lifetime has ran out.
        let Some(servitor) = self.summon(summon).and_then(|s| s.servitor.clone()) else {
            return;
        };
        if servitor.life_time_remaining < 0 {
            self.send_to_summon_owner(summon, &SystemMessage::new(SystemMessageId::YourServitorPassedAway));
            self.unsummon(summon, owner);
            return;
        }

        if servitor.consume_interval > 0 {
            let remaining = servitor.consume_interval_remaining - LIFE_TASK_MS;
            if let Some(s) = self.summon_mut(summon).and_then(|s| s.servitor.as_mut()) {
                s.consume_interval_remaining = remaining;
            }
            // Check if it is time to consume another item.
            let (item_id, count) = servitor.consume_item;
            if remaining <= 0 && count > 0 && item_id > 0 && !self.is_dead(summon) {
                // `Servitor.destroyItemByItemId` -> the owner's, without a message.
                let destroyed =
                    owner.is_some_and(|o| self.destroy_item_by_item_id(o, item_id, i64::from(count), false));
                if destroyed {
                    let templates = self.item_templates();
                    let message = SystemMessage::new(SystemMessageId::ASummonedMonsterUsesS1)
                        .with(crate::inventory::item_name_param(templates.as_deref(), item_id));
                    self.send_to_summon_owner(summon, &message);
                    // Reset.
                    if let Some(s) = self.summon_mut(summon).and_then(|s| s.servitor.as_mut()) {
                        s.consume_interval_remaining = s.consume_interval;
                    }
                } else {
                    self.send_to_summon_owner(
                        summon,
                        &SystemMessage::new(
                            SystemMessageId::SinceYouDoNotHaveEnoughItemsToMaintainTheServitorSStayTheServitorHasDisappeared,
                        ),
                    );
                    // NOTE(java-quirk): Java goes on after this `unSummon`: the owner still gets
                    // `SetSummonRemainTime` (and an icon update of the gone servitor, which has
                    // no effects left and no object here, so it is not sent).
                    self.unsummon(summon, owner);
                    if let Some(owner) = owner {
                        self.send(
                            owner,
                            &SetSummonRemainTime {
                                max_time: servitor.life_time,
                                remaining_time: servitor.life_time_remaining,
                            },
                        );
                    }
                    return;
                }
            }
        }

        self.send_to_summon_owner(
            summon,
            &SetSummonRemainTime {
                max_time: servitor.life_time,
                remaining_time: servitor.life_time_remaining,
            },
        );
        // `updateEffectIcons()`.
        let _ = self.effect_list_mut(summon);
        self.update_effect_icons(summon, false);
    }

    fn cancel_servitor_life_task(&mut self, summon: ObjectId) {
        let timer = self
            .summon_mut(summon)
            .and_then(|s| s.servitor.as_mut())
            .and_then(|s| s.life_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    // ------------------------------------------------------------------ unSummon

    /// Java `Servitor.unSummon(owner)` (with `Summon.unSummon`): the summon leaves the world; its
    /// state is stored for a restore and its buffs for its next summon.
    pub fn unsummon(&mut self, summon: ObjectId, owner: Option<ObjectId>) {
        if !self.is_summon(summon) {
            return;
        }
        self.cancel_servitor_life_task(summon);
        let spawned = self.objects.get(&summon).is_some_and(|e| e.spawned);
        if spawned && !self.is_dead(summon) {
            self.stop_hp_mp_regeneration(summon);
            self.stop_follow(summon);
            let kind = self.summon(summon).map(|s| s.kind);
            if let (Some(owner), Some(kind)) = (owner, kind) {
                self.send(
                    owner,
                    &PetDelete {
                        pet_type: kind.summon_type(),
                        pet_object_id: summon.0,
                    },
                );
                // M6 (PARTY-01): `party.broadcastToPartyMembers(owner, new ExPartyPetWindowDelete(this))`.
                // A servitor has no inventory: `_owner.setPetInvItems(false)` (PET-01 keeps the flag).
            }
            self.abort_attack(summon);
            self.abort_cast(summon);
            self.servitor_store_me(summon);
            self.servitor_store_effect(summon, true);
            if let Some(owner) = owner {
                self.set_pet(owner, None);
            }
            // Stop AI tasks.
            self.stop_summon_ai_task(summon);
            // NOTE(java-quirk): `stopAllEffects()` ends with `updateAndBroadcastStatus(1)`; the
            // summon is still spawned, so its owner gets `PetInfo` and `PetStatusUpdate` again
            // after the `PetDelete` above, and the others `SummonInfo`, before `decayMe()`.
            self.stop_all_effects(summon);
            self.decay(summon);
            // `oldRegion.removeFromZones(this)`: summons have no zone state.
            self.set_creature_target(summon, None);
            if let Some(owner) = owner {
                self.disable_beast_auto_shots(owner);
            }
        }
        // `Servitor.unSummon`: a summon that is not stored for a restore is forgotten.
        let restore = self.summon(summon).is_some_and(|s| s.restore_summon);
        if !restore && let Some(owner) = owner {
            self.remove_servitor(owner);
        }
        if !spawned || self.objects.get(&summon).is_none_or(|e| !e.spawned) {
            self.forget_summon_object(summon);
        }
    }

    /// `for (int itemId : owner.getAutoSoulShot())`: the automatic shots whose handler is a
    /// "Beast" one are switched off (`disableAutoShot`).
    fn disable_beast_auto_shots(&mut self, owner: ObjectId) {
        for item_id in self.auto_soul_shots(owner) {
            let beast = self
                .item_templates()
                .and_then(|t| {
                    t.get(item_id)
                        .and_then(|t| t.etc())
                        .map(|e| e.handler.as_deref().is_some_and(|h| h.contains("Beast")))
                })
                .unwrap_or(false);
            if beast {
                self.disable_auto_shot(owner, item_id);
            }
        }
    }

    /// Java `CharSummonTable.removeServitor(player)`.
    pub(crate) fn remove_servitor(&mut self, owner: ObjectId) {
        self.summons.saved_servitors.remove(&owner);
        self.outbox
            .persist(PersistCommand::Summon(SummonWrite::RemoveServitor { owner }));
    }

    /// Java `Servitor.storeMe()` -> `CharSummonTable.saveSummon(this)` with
    /// `RestoreServitorOnReconnect`.
    pub(crate) fn servitor_store_me(&mut self, summon: ObjectId) {
        let Some(state) = self.summon(summon) else {
            return;
        };
        let Some(servitor) = state.servitor.as_ref() else {
            return;
        };
        if servitor.reference_skill == 0 || self.is_dead(summon) {
            return;
        }
        if !self.summons.settings.restore_servitor_on_reconnect {
            return;
        }
        // `saveSummon`: nothing for a servitor whose time is up.
        if servitor.life_time_remaining <= 0 {
            return;
        }
        let (owner, summon_skill_id, time) = (state.owner, servitor.reference_skill, servitor.life_time_remaining);
        let Some(core) = self.core_of(summon) else {
            return;
        };
        let (cur_hp, cur_mp) = (core.cur_hp, core.cur_mp);
        self.summons.saved_servitors.insert(owner, summon_skill_id);
        self.outbox.persist(PersistCommand::Summon(SummonWrite::SaveServitor {
            owner,
            summon_skill_id,
            // `(int) Math.round(getCurrentHp())`.
            cur_hp: crate::stats::java_round(cur_hp) as i32,
            cur_mp: crate::stats::java_round(cur_mp) as i32,
            time,
        }));
    }

    /// The owner's class index (`getClassIndex()`), the key of the saved buffs.
    fn class_index_of(&self, owner: ObjectId) -> i32 {
        self.players.get(&owner).map_or(0, |p| p.data.profile.class_index)
    }

    /// Java `Servitor.storeEffect(storeEffects)`: the servitor's buffs, kept in memory for its
    /// next summon and written to `character_summon_skills_save`.
    pub(crate) fn servitor_store_effect(&mut self, summon: ObjectId, store_effects: bool) {
        if !self.summons.settings.summon_store_skill_cooltime {
            return;
        }
        let Some(state) = self.summon(summon) else {
            return;
        };
        let owner = state.owner;
        // `getOwner().isInOlympiadMode()`: no olympiad yet.
        if !self.players.contains_key(&owner) {
            return;
        }
        let Some(reference_skill) = state.servitor.as_ref().map(|s| s.reference_skill) else {
            return;
        };
        let class_index = self.class_index_of(owner);
        // Clear list for overwrite.
        self.clear_servitor_effects(owner, class_index, reference_skill);
        let mut rows: Vec<SummonSkillSave> = Vec::new();
        if store_effects && let Some(list) = self.effect_list(summon) {
            let mut stored: HashSet<SkillKey> = HashSet::new();
            for info in list.effects() {
                // Do not save heals.
                if info.abnormal_type() == AbnormalType::LifeForceOthers {
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
            self.add_servitor_effect(
                owner,
                class_index,
                reference_skill,
                SummonEffect {
                    skill: SkillKey::new(row.skill_id, row.skill_level),
                    time: row.remaining_time,
                },
            );
        }
        self.write_servitor_effect_rows(owner, class_index, reference_skill, rows);
    }

    /// Java `Servitor.restoreEffects()`: the buffs of the last servitor of the same skill come
    /// back (from memory, else from the saved rows), and the saved rows are deleted.
    pub(crate) fn servitor_restore_effects(&mut self, summon: ObjectId) {
        let Some(state) = self.summon(summon) else {
            return;
        };
        let owner = state.owner;
        let Some(reference_skill) = state.servitor.as_ref().map(|s| s.reference_skill) else {
            // A pet's are PET-01's.
            return;
        };
        // `getOwner().isInOlympiadMode()`: no olympiad yet.
        let class_index = self.class_index_of(owner);
        if !self.contains_servitor_skill(owner, class_index, reference_skill) {
            let rows = self
                .summons
                .saved_rows
                .get(&(owner, class_index, reference_skill))
                .cloned()
                .unwrap_or_default();
            let skills = self.skill_table();
            for row in rows {
                let Some(skill) = skills.as_ref().and_then(|t| t.get(row.skill_id, row.skill_level)) else {
                    continue;
                };
                // `skill.hasEffects(EffectScope.GENERAL)`.
                if !skill.effects.get(l2_data::skill::EffectScope::General).is_empty() {
                    self.add_servitor_effect(
                        owner,
                        class_index,
                        reference_skill,
                        SummonEffect {
                            skill: SkillKey::new(row.skill_id, row.skill_level),
                            time: row.remaining_time,
                        },
                    );
                }
            }
        }
        self.write_servitor_effect_rows(owner, class_index, reference_skill, Vec::new());
        // `finally`: `applyServitorEffects(this, getOwner(), getReferenceSkill())`.
        let effects = self
            .summons
            .servitor_effects
            .get(&owner)
            .and_then(|m| m.get(&class_index))
            .and_then(|m| m.get(&reference_skill))
            .cloned()
            .unwrap_or_default();
        for effect in effects {
            // `se.getSkill().applyEffects(summon, summon, false, se.getEffectCurTime())`.
            self.apply_effects_timed(summon, summon, effect.skill, false, effect.time);
        }
    }

    /// The `DELETE` (and the rows) of `character_summon_skills_save` for one reference skill,
    /// and the world's view of the table.
    fn write_servitor_effect_rows(
        &mut self,
        owner: ObjectId,
        class_index: i32,
        reference_skill: i32,
        rows: Vec<SummonSkillSave>,
    ) {
        if rows.is_empty() {
            self.summons.saved_rows.remove(&(owner, class_index, reference_skill));
        } else {
            self.summons
                .saved_rows
                .insert((owner, class_index, reference_skill), rows.clone());
        }
        self.outbox
            .persist(PersistCommand::Summon(SummonWrite::ServitorEffects {
                owner,
                class_index,
                summon_skill_id: reference_skill,
                effects: rows,
            }));
    }

    /// `SummonEffectTable.containsSkill(owner, referenceSkill)`.
    fn contains_servitor_skill(&self, owner: ObjectId, class_index: i32, reference_skill: i32) -> bool {
        self.summons
            .servitor_effects
            .get(&owner)
            .and_then(|m| m.get(&class_index))
            .is_some_and(|m| m.contains_key(&reference_skill))
    }

    /// `SummonEffectTable.clearServitorEffects(owner, referenceSkill)`.
    // NOTE(java-quirk): Java empties the set but keeps the key, so `containsSkill` stays true and
    // the saved rows are not read again in this session.
    fn clear_servitor_effects(&mut self, owner: ObjectId, class_index: i32, reference_skill: i32) {
        if let Some(effects) = self
            .summons
            .servitor_effects
            .get_mut(&owner)
            .and_then(|m| m.get_mut(&class_index))
            .and_then(|m| m.get_mut(&reference_skill))
        {
            effects.clear();
        }
    }

    /// `SummonEffectTable.addServitorEffect(owner, referenceSkill, skill, effectCurTime)`.
    // NOTE(java-quirk): Java keeps the effects in a concurrent set, applied in no fixed order;
    // here they are applied in the order they were stored.
    fn add_servitor_effect(&mut self, owner: ObjectId, class_index: i32, reference_skill: i32, effect: SummonEffect) {
        self.summons
            .servitor_effects
            .entry(owner)
            .or_default()
            .entry(class_index)
            .or_default()
            .entry(reference_skill)
            .or_default()
            .push(effect);
    }

    /// Java `Servitor.stopSkillEffects(type, skillId)`: the saved buffs of that skill go too
    /// (`SummonEffectTable.removeServitorEffects`).
    pub(crate) fn servitor_forget_saved_skill(&mut self, summon: ObjectId, skill_id: i32) {
        let Some(state) = self.summon(summon) else {
            return;
        };
        let owner = state.owner;
        let Some(reference_skill) = state.servitor.as_ref().map(|s| s.reference_skill) else {
            return;
        };
        let class_index = self.class_index_of(owner);
        if let Some(effects) = self
            .summons
            .servitor_effects
            .get_mut(&owner)
            .and_then(|m| m.get_mut(&class_index))
            .and_then(|m| m.get_mut(&reference_skill))
        {
            effects.retain(|e| e.skill.id != skill_id);
        }
    }

    /// The saved `character_summon_skills_save` rows of a character entering the world (the
    /// world's view of the table, see the module comment).
    pub(crate) fn take_saved_summon_effects(&mut self, owner: ObjectId) {
        // NOTE: Java brings a stored servitor back at the first action after the login
        // (`Player.onActionRequest` with spawn protection, `CharSummonTable.restoreServitor`), and
        // `ConditionPlayerCanSummon` refuses a new summon while one is stored. The restore is not
        // ported (no spawn protection), so the entry of an earlier session is dropped here: the
        // player can summon again, and its next unsummon or death deletes the row like Java.
        self.summons.saved_servitors.remove(&owner);
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let class_index = player.data.profile.class_index;
        let rows = std::mem::take(&mut player.data.profile.summon_effects);
        for row in rows {
            self.summons
                .saved_rows
                .entry((owner, class_index, row.summon_skill_id))
                .or_default()
                .push(SummonSkillSave {
                    skill_id: row.skill_id,
                    skill_level: row.skill_level,
                    remaining_time: row.remaining_time,
                });
        }
    }

    // ------------------------------------------------------------------ death and decay

    /// Java `Summon.doDie(killer)` then `Servitor.doDie(killer)`, around `Playable.doDie`
    /// (`playable_do_die`, which the caller runs in between: `before` true is the part before
    /// `super.doDie`, `false` the part after it).
    pub(crate) fn summon_do_die_before(&mut self, summon: ObjectId) {
        if self.is_affected(summon, crate::effects::effect_flag::NOBLESS_BLESSING) {
            self.stop_effects(summon, crate::effects::EffectType::NoblesseBlessing);
            self.servitor_store_effect(summon, true);
        } else {
            self.servitor_store_effect(summon, false);
        }
    }

    /// The part of `Summon.doDie` / `Servitor.doDie` after `super.doDie(killer)` succeeded.
    pub(crate) fn summon_do_die_after(&mut self, summon: ObjectId, killer: Option<ObjectId>) {
        let owner = self.summon_owner(summon);
        if let Some(owner) = owner {
            // The monsters that hate the summon hate its owner as much.
            for mob in self.visible_objects(summon) {
                if !self.is_attackable_npc(mob) || self.is_dead(mob) {
                    continue;
                }
                let info = self.aggro_list(mob).iter().find(|a| a.attacker == summon).copied();
                if let Some(info) = info {
                    self.add_damage_hate(mob, owner, info.damage, info.hate);
                }
            }
        }
        // "Retail monster AIs add 500 hate against the owner to the monster that finishes a
        // summon or pet."
        if let (Some(owner), Some(killer)) = (owner, killer)
            && self.is_attackable_npc(killer)
            && !self.is_dead(killer)
        {
            self.add_damage_hate(killer, owner, 0, 500);
        }
        self.add_decay_task(summon);
        // `Servitor.doDie`.
        self.cancel_servitor_life_task(summon);
        if let Some(owner) = owner {
            self.remove_servitor(owner);
        }
    }

    /// Java `Summon.onDecay()` -> `deleteMe(_owner)`.
    pub(crate) fn summon_on_decay(&mut self, summon: ObjectId) {
        // The packets of an owner who left reach nobody (Java's old `Player` object).
        let owner = self
            .summon(summon)
            .filter(|s| !s.owner_offline)
            .map(|s| s.owner);
        self.summon_delete_me(summon, owner);
    }

    /// Java `Summon.deleteMe(owner)`: the owner's window closes, the summon leaves the world.
    pub(crate) fn summon_delete_me(&mut self, summon: ObjectId, owner: Option<ObjectId>) {
        let Some(kind) = self.summon(summon).map(|s| s.kind) else {
            return;
        };
        if let Some(owner) = owner {
            self.send(
                owner,
                &PetDelete {
                    pet_type: kind.summon_type(),
                    pet_object_id: summon.0,
                },
            );
            // M6 (PARTY-01): `party.broadcastToPartyMembers(owner, new ExPartyPetWindowDelete(this))`.
        }
        // A pet's items go with it (PET-01).
        self.cancel_decay_task(summon);
        self.decay(summon);
        if let Some(owner) = owner
            && self.summon_of(owner) == Some(summon)
        {
            self.set_pet(owner, None);
        }
        // `Creature.deleteMe`: the AI stops, every effect goes without a broadcast.
        self.stop_summon_ai_task(summon);
        self.stop_all_effects_without_exclusions(summon, false, false);
        self.forget_summon_object(summon);
    }

    /// Drops everything the world keeps about a summon whose object is gone (Java's object is
    /// garbage once nothing points at it). The object id is not given back, as for NPCs.
    pub(crate) fn forget_summon_object(&mut self, summon: ObjectId) {
        if self.objects.get(&summon).is_some_and(|e| e.spawned) {
            return;
        }
        self.cancel_servitor_life_task(summon);
        self.stop_summon_avoid_task(summon);
        let abnormal = self.summon_mut(summon).and_then(|s| s.abnormal_effect_timer.take());
        if let Some(timer) = abnormal {
            self.timers.cancel(timer);
        }
        self.stop_follow(summon);
        self.clear_pending_path(summon);
        self.cancel_decay_task(summon);
        self.objects.remove(&summon);
        self.forget_effects(summon);
        self.forget_combat(summon);
        self.forget_status(summon);
        self.forget_death_state(summon);
        self.moving_set.remove(&summon);
        self.moving.retain(|m| *m != summon);
        self.npcs.remove(&summon);
        if let Some(state) = self.summons.summons.remove(&summon)
            && self.summons.by_owner.get(&state.owner) == Some(&summon)
        {
            self.summons.by_owner.remove(&state.owner);
        }
    }

    // ------------------------------------------------------------------ the owner

    /// The summon part of Java `Player.deleteMe()`: the summon is stored for a restore and
    /// unsummoned; a dead one stays (its owner's name goes from its title).
    pub(crate) fn summon_on_owner_leave(&mut self, owner: ObjectId) {
        let Some(summon) = self.summon_of(owner) else {
            return;
        };
        if let Some(state) = self.summon_mut(summon) {
            // `Servitor.setRestoreSummon(true)`.
            state.restore_summon = true;
        }
        self.unsummon(summon, Some(owner));
        // "Dead pet wasn't unsummoned, broadcast npcinfo changes (pet will be without owner
        // name - means owner offline)."
        if self.summon_of(owner) == Some(summon) {
            // `setOnlineStatus(false)` came first in `deleteMe`: the title is empty now.
            if let Some(state) = self.summon_mut(summon) {
                state.owner_offline = true;
            }
            self.broadcast_summon_npc_info(summon, 0);
            // A new login is a new `Player` without a summon; the corpse keeps its owner id.
            self.forget_summon_owner(owner);
        }
    }

    /// The summon part of Java `Player.onTeleported()`: the summon comes along.
    pub(crate) fn summon_follow_owner_teleport(&mut self, owner: ObjectId) {
        let Some(summon) = self.summon_of(owner) else {
            return;
        };
        let Some((x, y, z)) = self.position_of(owner) else {
            return;
        };
        let heading = self.heading_of(owner);
        let instance = self.objects.get(&owner).map_or(0, |e| e.instance_id);
        self.set_follow_status(summon, false);
        // `_summon.teleToLocation(getLocation(), false)`.
        self.tele_to_location(
            summon,
            crate::teleport::TeleportTarget {
                x,
                y,
                z,
                heading,
                instance_id: instance,
                random_offset: 0,
            },
        );
        self.set_start_follow_controller(summon, true);
        self.set_follow_status(summon, true);
        self.update_and_broadcast_summon_status(summon, 0);
    }

}
