//! The skills a hit or a cast triggers by chance: the activation skills of worn item options
//! (`Creature._triggerSkills`, `OptionSkillHolder`) and the weapon's `oncrit_skill` /
//! `onmagic_skill`.
// Java: entity/actor/Creature.java (getTriggerSkills, addTriggerSkill, removeTriggerSkill, the end
//       of doAttackHitSimple's hit and `callSkill`)
// Java: entity/item/Weapon.java (castOnCriticalSkill, castOnMagicSkill)
// Java: mechanics/options/{Options,OptionSkillHolder,OptionSkillType}.java
//
// # Where they fire
// - [`World::trigger_skills_on_hit`]: `Creature.onHitTimer` after a normal attack landed
//   (`combat/damage.rs`): the ATTACK holders when it was not a critical hit, the CRITICAL holders
//   when it was, then the weapon's on-critical skill.
// - [`World::trigger_skills_on_call_skill`]: `Creature.callSkill`, once per target
//   (`skills/cast.rs`): the weapon's on-magic skill, then the MAGIC holders for a magic skill and
//   the ATTACK holders for a physical one. Static skills trigger nothing.
//
// The option holders are put on the creature by `Options.apply` (an item with the option was
// put on, `stats/skills.rs`) and taken off by `Options.remove`. They are kept by skill id, so two
// items with the same trigger skill share one entry: the first one taken off removes it.
//
// Each holder rolls `Rnd.get(100) < chance` only when its type fits the event, so the RNG draws
// are the ones of Java. They go through [`World::make_trigger_cast`] (`effects/triggers.rs`).

use l2_data::SkillTemplate;
use l2_data::augmentation::{OptionSkillHolder, OptionSkillType};
use l2_data::item::ChanceSkill;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::effects::apply::has_negative_effect;
use crate::effects::buff_info::skill_name_param;
use crate::formulas::{Rnd, SHIELD_DEFENSE_PERFECT_BLOCK};
use crate::stats::skills::java_hash_map_order;
use crate::world::World;

/// Whether the weapon skill is the on-crit or the on-magic one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponSkill {
    Critical,
    Magic,
}

impl World {
    /// Java `Creature.addTriggerSkill(holder)`: `_triggerSkills.put(skillId, holder)`. A holder
    /// for the same skill id is replaced in its place.
    pub fn add_trigger_skill(&mut self, id: ObjectId, holder: OptionSkillHolder) {
        let list = &mut self.cast_state_mut(id).trigger_skills;
        match list.iter_mut().find(|h| h.skill.id == holder.skill.id) {
            Some(existing) => *existing = holder,
            None => list.push(holder),
        }
    }

    /// Java `Creature.removeTriggerSkill(holder)`: `_triggerSkills.remove(skillId)`.
    pub fn remove_trigger_skill(&mut self, id: ObjectId, skill_id: i32) {
        if let Some(state) = self.cast.creatures.get_mut(&id) {
            state.trigger_skills.retain(|h| h.skill.id != skill_id);
        }
    }

    /// Java `_triggerSkills.values()`: in the order of a `HashMap<Integer, _>`.
    pub fn trigger_skills(&self, id: ObjectId) -> Vec<OptionSkillHolder> {
        let Some(state) = self.cast.creatures.get(&id) else {
            return Vec::new();
        };
        let keys: Vec<i32> = state.trigger_skills.iter().map(|h| h.skill.id).collect();
        java_hash_map_order(&keys)
            .into_iter()
            .filter_map(|key| state.trigger_skills.iter().find(|h| h.skill.id == key).copied())
            .collect()
    }

    /// The end of a landed normal attack (`onHitTimer`, damage > 0): the option holders that fit
    /// the hit, then `weapon.castOnCriticalSkill` after a critical hit.
    pub(crate) fn trigger_skills_on_hit(&mut self, attacker: ObjectId, target: ObjectId, crit: bool) {
        for holder in self.trigger_skills(attacker) {
            let fits = (!crit && holder.kind == OptionSkillType::Attack)
                || (holder.kind == OptionSkillType::Critical && crit);
            if fits && self.roll_chance(holder.chance) {
                self.make_trigger_cast(
                    attacker,
                    SkillKey::new(holder.skill.id, holder.skill.level),
                    target,
                    false,
                );
            }
        }
        // Launch weapon onCritical Special ability effect if available.
        if crit && self.has_active_weapon(attacker) {
            self.weapon_cast_on_critical_skill(attacker, target);
        }
    }

    /// `callSkill`'s part for one target of a skill that is not static: the weapon's on-magic
    /// skill (when the target is alive), then the option holders that fit the skill.
    pub(crate) fn trigger_skills_on_call_skill(
        &mut self,
        caster: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) {
        // Static skills not trigger any chance skills.
        if skill.is_static() {
            return;
        }
        // Launch weapon Special ability skill effect if available.
        if self.has_active_weapon(caster) && !self.is_dead(target) {
            self.weapon_cast_on_magic_skill(caster, target, skill);
        }
        for holder in self.trigger_skills(caster) {
            let fits = (skill.is_magic() && holder.kind == OptionSkillType::Magic)
                || (skill.is_physical() && holder.kind == OptionSkillType::Attack);
            if fits && self.roll_chance(holder.chance) {
                self.make_trigger_cast(
                    caster,
                    SkillKey::new(holder.skill.id, holder.skill.level),
                    target,
                    false,
                );
            }
        }
    }

    /// `Rnd.get(100) < chance`.
    fn roll_chance(&mut self, chance: f64) -> bool {
        f64::from(self.rng().get_int(100)) < chance
    }

    /// `getActiveWeaponItem() != null`.
    fn has_active_weapon(&self, id: ObjectId) -> bool {
        self.stats(id).is_some_and(|v| v.active_weapon_item().is_some())
    }

    /// The `oncrit_skill` / `onmagic_skill` of the weapon in hand.
    fn weapon_chance_skill(&self, id: ObjectId, which: WeaponSkill) -> Option<ChanceSkill> {
        let view = self.stats(id)?;
        let weapon = view.active_weapon_item()?;
        match which {
            WeaponSkill::Critical => weapon.skills.on_crit,
            WeaponSkill::Magic => weapon.skills.on_magic,
        }
    }

    /// Java `Weapon.castOnCriticalSkill(caster, target)`.
    pub fn weapon_cast_on_critical_skill(&mut self, caster: ObjectId, target: ObjectId) {
        let Some(chance_skill) = self.weapon_chance_skill(caster, WeaponSkill::Critical) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(on_crit) = table.get_skill(chance_skill.skill.id, chance_skill.skill.level.max(1)) else {
            return;
        };
        // `ConditionGameChance`: chance not met.
        if !self.roll_chance(f64::from(chance_skill.chance)) {
            return;
        }
        let key = SkillKey::of(on_crit);
        // Skill condition not met.
        if !self.check_skill_condition(caster, target, key) {
            return;
        }
        self.activate_skill(caster, &[target], key);
    }

    /// Java `Weapon.castOnMagicSkill(caster, target, trigger)`.
    pub fn weapon_cast_on_magic_skill(
        &mut self,
        caster: ObjectId,
        target: ObjectId,
        trigger: &SkillTemplate,
    ) {
        let Some(chance_skill) = self.weapon_chance_skill(caster, WeaponSkill::Magic) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(on_magic) = table.get_skill(chance_skill.skill.id, chance_skill.skill.level.max(1)) else {
            return;
        };
        // Trigger only if both are good or bad magic.
        if has_negative_effect(trigger) != has_negative_effect(on_magic) {
            return;
        }
        // No Trigger if not Magic Skill.
        if !trigger.is_magic() && !on_magic.is_magic() {
            return;
        }
        if trigger.is_toggle() {
            return;
        }
        if self.cast_target_of(caster) != Some(target) {
            return;
        }
        // `ConditionGameChance`: chance not met.
        if !self.roll_chance(f64::from(chance_skill.chance)) {
            return;
        }
        let key = SkillKey::of(on_magic);
        // Skill condition not met.
        if !self.check_skill_condition(caster, target, key) {
            return;
        }
        if has_negative_effect(on_magic)
            && self.calc_shld_use_skill(caster, target, on_magic, true) == SHIELD_DEFENSE_PERFECT_BLOCK
        {
            return;
        }

        // Launch the magic skill and calculate its effects.
        self.activate_skill(caster, &[target], key);

        // Notify quests of a skill use.
        if self.players.contains_key(&caster) {
            let targets = [target];
            let watchers: Vec<ObjectId> = self
                .visible_objects_in_range(caster, 1000)
                .into_iter()
                .filter(|o| self.npcs.contains_key(o))
                .collect();
            for npc in watchers {
                self.hall_script_on_skill_see(npc, caster, on_magic.id, &targets);
                self.territory_on_skill_see(npc, caster, on_magic.id, &targets);
            }
            self.send(
                caster,
                &SystemMessage::new(SystemMessageId::S1HasBeenActivated).with(skill_name_param(on_magic)),
            );
        }
    }
}
