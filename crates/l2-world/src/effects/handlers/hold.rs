//! The handlers that hold a creature in place: stun, sleep, root, paralysis and petrification.
// Java: dist/game/data/scripts/handlers/skill/effects/Stun.java
// Java: dist/game/data/scripts/handlers/skill/effects/Sleep.java
// Java: dist/game/data/scripts/handlers/skill/effects/Root.java
// Java: dist/game/data/scripts/handlers/skill/effects/Paralyze.java
// Java: dist/game/data/scripts/handlers/skill/effects/Petrification.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
use crate::world::World;

/// Java `Stun`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Stun;

impl Stun {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Stun))
    }
}

impl EffectHandler for Stun {
    fn effect_flags(&self) -> u32 {
        effect_flag::STUNNED
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Stun
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        // Raid bosses cannot be stunned.
        if world.is_raid(cx.effected) {
            return;
        }
        world.stop_stunning(cx.effected, false);
        let effected = cx.effected;
        if world.is_summon(effected) {
            let owner = world.summon_owner(effected);
            let to_owner = |world: &mut World| {
                // `getAI().setIntentionMoveTo(effected.asPlayer())`.
                if let Some((x, y, z)) = owner.and_then(|o| world.position_of(o)) {
                    world.summon_set_intention_move_to(effected, x, y, z);
                }
            };
            let effector = cx.effector;
            if world.objects.contains_key(&effector) && !world.is_dead(effector) {
                let owner_flag = owner.map_or(0, |o| world.pvp_flag_of(o));
                if world.is_playable(effector) && owner_flag == 0 {
                    to_owner(world);
                } else {
                    world.do_summon_attack(effected, Some(effector));
                }
            } else {
                to_owner(world);
            }
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_raid(cx.effected) {
            return;
        }
        world.start_stunning(cx.effected);
    }
}

/// Java `Sleep`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sleep;

impl Sleep {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Sleep))
    }
}

impl EffectHandler for Sleep {
    fn effect_flags(&self) -> u32 {
        effect_flag::SLEEP
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Sleep
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effected) {
            world.set_intention_active(cx.effected);
        } else {
            world.notify_action_think(cx.effected);
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        world.abort_attack(effected);
        world.abort_cast_for_effects(effected);
        world.stop_move(effected, None);
        world.notify_action_sleeping(effected);
    }
}

/// Java `Root`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Root;

impl Root {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Root))
    }
}

impl EffectHandler for Root {
    fn effect_flags(&self) -> u32 {
        effect_flag::ROOTED
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Root
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            world.notify_action_think(cx.effected);
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.stop_move(cx.effected, None);
        world.notify_action_rooted(cx.effected);
    }
}

/// Java `Paralyze`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Paralyze;

impl Paralyze {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Paralyze))
    }
}

impl EffectHandler for Paralyze {
    fn effect_flags(&self) -> u32 {
        effect_flag::PARALYZED
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Paralyze
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            world.notify_action_think(cx.effected);
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.set_intention_idle(cx.effected);
        world.start_paralyze(cx.effected);
    }
}

/// Java `Petrification`: paralysis and invulnerability, without an effect type.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Petrification;

impl Petrification {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Petrification))
    }
}

impl EffectHandler for Petrification {
    fn effect_flags(&self) -> u32 {
        effect_flag::PARALYZED | effect_flag::INVUL
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            world.notify_action_think(cx.effected);
        }
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.start_paralyze(cx.effected);
    }
}
