//! `Distrust`: a monster turns on another monster nearby.
// Java: dist/game/data/scripts/handlers/skill/effects/Distrust.java
// Java: ai/DistrustAI.java (see `World::distrust_think_attack`)

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// `World.getRandomVisibleObjectInRange(monster, Monster.class, 1100, ...)`.
const DISTRUST_RANGE: i32 = 1100;

/// Java `Distrust`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Distrust;

impl Distrust {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Distrust))
    }
}

impl EffectHandler for Distrust {
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let target = cx.effected;
        if world.is_dead(target)
            || !world.is_monster(target)
            || world.is_chest(target)
            || world.is_raid_or_minion(target)
        {
            world.send(cx.effector, &SystemMessage::new(SystemMessageId::InvalidTarget));
            return;
        }
        let Some(new_target) = world.random_distrust_target(target, DISTRUST_RANGE) else {
            return;
        };
        // `targetMonster.setAI(new DistrustAI(effected.asAttackable(), newTarget))`: a new AI
        // object, forced to fight the other monster.
        world.detach_ai(target);
        world.wake_ai(target);
        if let Some(control) = world.control_mut(target) {
            control.distrust = Some(new_target);
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        let monster = cx.effected;
        if !world.is_attackable_npc(monster) || world.is_dead(monster) {
            return;
        }
        // `monster.setAI(new AttackableAI(monster))`.
        if let Some(control) = world.control_mut(monster) {
            control.distrust = None;
        }
        world.detach_ai(monster);
        world.wake_ai(monster);
        world.set_creature_target(monster, None);
        // `setWalking()`.
        world.set_running_effect(monster, false);
        world.set_intention_active(monster);
    }
}
