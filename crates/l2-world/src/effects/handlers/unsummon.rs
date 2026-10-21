//! The `Unsummon` effect: sends the target's summon away.
// Java: dist/game/data/scripts/handlers/skill/effects/Unsummon.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Unsummon`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unsummon {
    chance: i32,
}

impl Unsummon {
    /// Java `Unsummon(attachCond, applyCond, set, params)`: `chance` (100).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for Unsummon {
    fn is_instant(&self) -> bool {
        true
    }

    /// `magicLevel <= 0 || effected level - 9 <= magicLevel`, then `chance * attribute bonus *
    /// trait bonus > Rnd.nextDouble() * 100`.
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let magic_level = cx.skill.magic_level;
        let effected_level = world.stats(cx.effected).map_or(0, |v| v.level());
        if magic_level <= 0 || (effected_level - 9) <= magic_level {
            let chance = f64::from(self.chance)
                * world.calc_attribute_bonus_for_effect(cx.effector, cx.effected, cx.skill)
                * world.calc_general_trait_bonus(cx.effector, cx.effected, cx.skill.trait_type, false);
            if chance > world.rng().next_f64() * 100.0 {
                return true;
            }
        }
        false
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effected.asSummon()`: nothing for anybody else.
        let summon = cx.effected;
        if !world.is_summon(summon) {
            return;
        }
        world.abort_attack(summon);
        world.abort_cast(summon);
        world.stop_all_effects(summon);
        let owner = world.summon_owner(summon);
        world.unsummon(summon, owner);
        // `player.sendPacket(...)`.
        if let Some(owner) = owner {
            world.send(
                owner,
                &SystemMessage::new(SystemMessageId::YourServitorHasVanishedYouLlNeedToSummonANewOne),
            );
        }
    }
}
