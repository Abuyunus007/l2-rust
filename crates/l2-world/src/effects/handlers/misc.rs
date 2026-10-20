//! Passive, Flag and the instant "mess with the target" effects: TargetCancel, Bluff,
//! SkillTurning.
// Java: dist/game/data/scripts/handlers/skill/effects/Passive.java
// Java: dist/game/data/scripts/handlers/skill/effects/Flag.java
// Java: dist/game/data/scripts/handlers/skill/effects/TargetCancel.java
// Java: dist/game/data/scripts/handlers/skill/effects/Bluff.java
// Java: dist/game/data/scripts/handlers/skill/effects/SkillTurning.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::server::{StartRotation, StopRotation};

use super::status_params::int_or;
use crate::effects::registry::{EffectCx, EffectHandler, effect_flag};
use crate::world::World;

/// The NPC id of the headquarters, which does not turn around (`Bluff`).
const HEADQUARTERS_NPC_ID: i32 = 35062;

/// Java `Passive`: the monster stops being aggressive (`Monster.isAggressive()` reads the flag).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Passive;

impl Passive {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Passive))
    }
}

impl EffectHandler for Passive {
    fn effect_flags(&self) -> u32 {
        effect_flag::PASSIVE
    }

    /// `effected.isAttackable()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_attackable_npc(cx.effected)
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let target = cx.effected;
        if !world.is_attackable_npc(target) || world.is_player(target) || world.is_raid_or_minion(target) {
            return;
        }
        world.set_creature_target(target, None);
        // `setWalking()`.
        world.set_running_effect(target, false);
        world.set_intention_active(target);
    }
}

/// Java `Flag`: the player is flagged for PvP while it lasts.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Flag;

impl Flag {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Flag))
    }
}

impl EffectHandler for Flag {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.update_pvp_flag(cx.effected, 0);
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.update_pvp_flag(cx.effected, 1);
    }
}

/// Java `TargetCancel`: the target loses its target and whatever it was doing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TargetCancel {
    chance: i32,
}

impl TargetCancel {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TargetCancel {
            chance: int_or(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for TargetCancel {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        world.calc_probability_for_effect(f64::from(self.chance), cx.effector, cx.effected, cx.skill)
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_raid_or_minion(effected) {
            return;
        }
        world.set_creature_target(effected, None);
        world.abort_attack(effected);
        world.abort_cast_for_effects(effected);
        world.set_intention_idle(effected);
    }
}

/// Java `Bluff`: the target turns to face the effector's back (it looks the other way).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bluff {
    chance: i32,
}

impl Bluff {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Bluff {
            chance: int_or(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for Bluff {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        world.calc_probability_for_effect(f64::from(self.chance), cx.effector, cx.effected, cx.skill)
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        // Headquarters NPC should not rotate.
        let npc_id = world.npcs.get(&effected).map(|n| n.npc_id);
        if npc_id == Some(HEADQUARTERS_NPC_ID) || world.is_raid_or_minion(effected) {
            return;
        }
        let own = world.heading_of(effected);
        let effector_heading = world.heading_of(cx.effector);
        world.broadcast_creature_packet(
            effected,
            &StartRotation {
                object_id: effected.0,
                degree: own,
                side: 1,
                speed: 65535,
            },
        );
        world.broadcast_creature_packet(
            effected,
            &StopRotation {
                object_id: effected.0,
                degree: effector_heading,
                speed: 65535,
            },
        );
        world.set_heading(effected, effector_heading);
    }
}

/// Java `SkillTurning`: the target's cast is broken.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SkillTurning {
    chance: i32,
}

impl SkillTurning {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(SkillTurning {
            chance: int_or(template, "chance", 100)?,
        }))
    }
}

impl EffectHandler for SkillTurning {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        world.calc_probability_for_effect(f64::from(self.chance), cx.effector, cx.effected, cx.skill)
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if cx.effected == cx.effector || world.is_raid_or_minion(cx.effected) {
            return;
        }
        world.break_cast_for_effects(cx.effected);
    }
}
