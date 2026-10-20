//! The `Spoil` effect: the caster becomes the spoiler of a monster, which can then be swept after
//! it dies.
// Java: dist/game/data/scripts/handlers/skill/effects/Spoil.java
// Java: mechanics/stats/Formulas.java (calcMagicSuccess)

use l2_data::SkillTemplate;
use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::formulas::{MagicSuccessInput, calc_magic_success};
use crate::stats::Stat;
use crate::stats::skills::calc_skill;
use crate::world::World;

/// Java `Spoil`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Spoil;

impl Spoil {
    /// Java `Spoil(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Spoil))
    }
}

/// Java `Formulas.calcMagicSuccess(attacker, target, skill)`: builds the inputs of the
/// vector-checked formula (`formulas/magic.rs`) from the world and draws from the world RNG.
pub(crate) fn magic_success(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    skill: &SkillTemplate,
) -> bool {
    let ctx = calc_skill(skill);
    let by_skill_magic_level = world.settings.combat.calculate_magic_success_by_skill_magic_level;
    let min_npc_level_magic_penalty = world.settings.combat.min_npc_level_magic_penalty;
    let npc_skill_chance_penalty = world.settings.combat.npc_skill_chance_penalty.clone();
    world.with_rng(|w, rng| {
        let (Some(av), Some(tv)) = (w.stats(attacker), w.stats(target)) else {
            // A creature that left the world cannot be rolled against.
            return false;
        };
        let input = MagicSuccessInput {
            skill_power: f64::from(skill.power),
            skill_activate_rate: skill.activate_rate,
            calculate_magic_success_by_skill_magic_level: by_skill_magic_level,
            skill_magic_level: skill.magic_level,
            attacker_level: av.level(),
            target_level: tv.level(),
            target_is_attackable: tv.is_attackable(),
            target_is_raid: tv.is_raid(),
            // Java `isRaidMinion()`: a minion whose leader is a raid boss.
            target_is_raid_minion: tv
                .npc()
                .and_then(|n| n.leader)
                .is_some_and(|leader| w.is_raid(leader)),
            min_npc_level_magic_penalty,
            attacker_player_level: av.player().map(|p| p.core.level),
            npc_skill_chance_penalty: &npc_skill_chance_penalty,
            magic_success_res: tv.calc_stat(Stat::MagicSuccessRes, 1.0, None, Some(&ctx)),
        };
        calc_magic_success(&input, rng)
    })
}

impl EffectHandler for Spoil {
    fn is_instant(&self) -> bool {
        true
    }

    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        magic_success(world, cx.effector, cx.effected, cx.skill)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_monster(cx.effected) || world.is_dead(cx.effected) {
            world.send(cx.effector, &SystemMessage::new(SystemMessageId::InvalidTarget));
            return;
        }
        if world.is_spoiled(cx.effected) {
            world.send(
                cx.effector,
                &SystemMessage::new(SystemMessageId::ItHasAlreadyBeenSpoiled),
            );
            return;
        }
        world.set_spoiler(cx.effected, Some(cx.effector));
        world.send(
            cx.effector,
            &SystemMessage::new(SystemMessageId::TheSpoilConditionHasBeenActivated),
        );
        world.notify_action_attacked(cx.effected, cx.effector);
    }
}
