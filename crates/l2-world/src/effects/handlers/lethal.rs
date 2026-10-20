//! The `Lethal` effect: a chance to take a target down to 1 HP (or half its HP).
// Java: dist/game/data/scripts/handlers/skill/effects/Lethal.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::formulas::Rnd;
use crate::world::World;

/// Java `Lethal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lethal {
    full_lethal: i32,
    half_lethal: i32,
}

impl Lethal {
    /// Java `Lethal(attachCond, applyCond, set, params)`: `fullLethal` and `halfLethal` (0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            full_lethal: params::int(template, "fullLethal", 0)?,
            half_lethal: params::int(template, "halfLethal", 0)?,
        }))
    }
}

impl EffectHandler for Lethal {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        // NOTE: the access level's `canGiveDamage` is not loaded (every level may give damage,
        // the shipped `AccessLevels.xml` default), as in `Skill.applyEffects`.
        let effected_level = world.stats(effected).map_or(0, |v| v.level());
        if skill.magic_level < (effected_level - 6) {
            return;
        }
        if !is_lethalable(world, effected) || world.is_invul(effected) {
            return;
        }

        let chance_multiplier = world.calc_attribute_bonus_for_effect(effector, effected, skill)
            * world.calc_general_trait_bonus(effector, effected, skill.trait_type, false);
        let current_hp = world.core_of(effected).map_or(0.0, |c| c.cur_hp);

        // Lethal Strike
        if f64::from(world.rng().get_int(100)) < (f64::from(self.full_lethal) * chance_multiplier) {
            // for Players CP and HP is set to 1.
            if world.is_player(effected) {
                world.notify_damage_received(effected, current_hp - 1.0);
                world.set_current_cp(effected, 1.0, true);
                world.set_current_hp(effected, 1.0, true);
                world.send(effected, &SystemMessage::new(SystemMessageId::LethalStrike));
            }
            // For Monsters HP is set to 1 (summons: M6).
            else if world.is_monster(effected) {
                world.notify_damage_received(effected, current_hp - 1.0);
                world.set_current_hp(effected, 1.0, true);
            }
            world.send(
                effector,
                &SystemMessage::new(SystemMessageId::YourLethalStrikeWasSuccessful),
            );
        }
        // Half-Kill
        else if f64::from(world.rng().get_int(100)) < (f64::from(self.half_lethal) * chance_multiplier) {
            // for Players CP is set to 1.
            if world.is_player(effected) {
                world.set_current_cp(effected, 1.0, true);
                world.send(effected, &SystemMessage::new(SystemMessageId::HalfKill));
                world.send(
                    effected,
                    &SystemMessage::new(SystemMessageId::YourCpWasDrainedBecauseYouWereHitWithAHalfKillSkill),
                );
            }
            // For Monsters HP is set to 50%.
            else if world.is_monster(effected) {
                world.notify_damage_received(effected, current_hp * 0.5);
                world.set_current_hp(effected, current_hp * 0.5, true);
            }
            world.send(effector, &SystemMessage::new(SystemMessageId::HalfKill));
        }
    }
}

/// Java `Creature.isLethalable()`: true unless the class or a script made it false. Raid and
/// grand bosses and doors set it false in their constructors (`NonLethalableNpcs` and the
/// instance scripts are not modelled).
fn is_lethalable(world: &World, id: ObjectId) -> bool {
    !world.is_raid(id) && !world.doors.contains_key(&id)
}
