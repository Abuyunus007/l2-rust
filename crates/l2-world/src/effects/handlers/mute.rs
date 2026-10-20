//! The mutes: no magic, no physical skills, no physical attacks.
// Java: dist/game/data/scripts/handlers/skill/effects/Mute.java
// Java: dist/game/data/scripts/handlers/skill/effects/PhysicalMute.java
// Java: dist/game/data/scripts/handlers/skill/effects/PhysicalDamageMute.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
use crate::world::World;

/// Java `Mute`: no spells (`isMuted()`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Mute;

impl Mute {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Mute))
    }
}

impl EffectHandler for Mute {
    fn effect_flags(&self) -> u32 {
        effect_flag::MUTED
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Mute
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.abort_cast_for_effects(cx.effected);
        world.notify_action_muted(cx.effected);
    }
}

/// Java `PhysicalMute`: no physical skills (`isPhysicalMuted()`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhysicalMute;

impl PhysicalMute {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(PhysicalMute))
    }
}

impl EffectHandler for PhysicalMute {
    fn effect_flags(&self) -> u32 {
        effect_flag::PSYCHICAL_MUTED
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // Raid bosses cannot be muted.
        if world.is_raid(cx.effected) {
            return;
        }
        world.notify_action_muted(cx.effected);
    }
}

/// Java `PhysicalDamageMute`: no physical attacks (`isPhysicalAttackMuted()`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhysicalDamageMute;

impl PhysicalDamageMute {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(PhysicalDamageMute))
    }
}

impl EffectHandler for PhysicalDamageMute {
    fn effect_flags(&self) -> u32 {
        effect_flag::PSYCHICAL_ATTACK_MUTED
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.abort_attack(cx.effected);
    }
}
