//! The status handlers whose whole work is an effect flag or an effect type (the checks that read
//! the flag are `isInvul()`, `isAffected(NOBLESS_BLESSING)`, ... in the rest of the world).
// Java: dist/game/data/scripts/handlers/skill/effects/Debuff.java
// Java: dist/game/data/scripts/handlers/skill/effects/Invincible.java
// Java: dist/game/data/scripts/handlers/skill/effects/BlockResurrection.java
// Java: dist/game/data/scripts/handlers/skill/effects/ProtectionBlessing.java
// Java: dist/game/data/scripts/handlers/skill/effects/NoblesseBless.java
// Java: dist/game/data/scripts/handlers/skill/effects/Lucky.java
// Java: dist/game/data/scripts/handlers/skill/effects/PolearmSingleTarget.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
use crate::world::World;

/// Java `Debuff`: an effect type and nothing else.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Debuff;

impl Debuff {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Debuff))
    }
}

impl EffectHandler for Debuff {
    fn effect_type(&self) -> EffectType {
        EffectType::Debuff
    }
}

/// Java `Invincible`: the creature is invulnerable (`isInvul()` reads the flag).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Invincible;

impl Invincible {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Invincible))
    }
}

impl EffectHandler for Invincible {
    fn effect_flags(&self) -> u32 {
        effect_flag::INVUL
    }
}

/// Java `BlockResurrection`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BlockResurrection;

impl BlockResurrection {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(BlockResurrection))
    }
}

impl EffectHandler for BlockResurrection {
    fn effect_flags(&self) -> u32 {
        effect_flag::BLOCK_RESURRECTION
    }
}

/// Java `ProtectionBlessing`: players only.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProtectionBlessing;

impl ProtectionBlessing {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ProtectionBlessing))
    }
}

impl EffectHandler for ProtectionBlessing {
    /// `effector != null && effected != null && effected.isPlayer()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.players.contains_key(&cx.effected)
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::PROTECTION_BLESSING
    }
}

/// Java `NoblesseBless`: playables only (players and, later, summons).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NoblesseBless;

impl NoblesseBless {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(NoblesseBless))
    }
}

impl EffectHandler for NoblesseBless {
    /// `effected.isPlayable()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.players.contains_key(&cx.effected)
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::NOBLESS_BLESSING
    }

    fn effect_type(&self) -> EffectType {
        EffectType::NoblesseBlessing
    }
}

/// Java `Lucky`: the "lucky" passive of newbies. It changes nothing by itself (the stats it
/// stands for are in the skill's functions); as a passive it stays on its tick.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Lucky;

impl Lucky {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Lucky))
    }
}

impl EffectHandler for Lucky {
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.players.contains_key(&cx.effected)
    }

    fn on_action_time(&self, _world: &mut World, cx: &EffectCx<'_>) -> bool {
        cx.skill.is_passive()
    }
}

/// Java `PolearmSingleTarget`: a polearm hits only its main target (`doAttackHitByPole`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PolearmSingleTarget;

impl PolearmSingleTarget {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(PolearmSingleTarget))
    }
}

impl EffectHandler for PolearmSingleTarget {
    fn effect_flags(&self) -> u32 {
        effect_flag::POLEARM_SINGLE_TARGET
    }
}
