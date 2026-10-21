//! The `ServitorShare` effect: part of the caster's stats goes to its servitor while it lasts.
// Java: dist/game/data/scripts/handlers/skill/effects/ServitorShare.java

use l2_data::Stat;
use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::EffectType;
use crate::effects::registry::{EffectCx, EffectHandler, effect_flag};
use crate::world::World;

/// Java `ServitorShare`: the stat multipliers by their XML name (`pAtk`, `maxHp`, ...).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ServitorShare {
    stats: Vec<(Stat, f64)>,
}

impl ServitorShare {
    /// Java `ServitorShare(attachCond, applyCond, set, params)`: every parameter is a stat
    /// (`Stat.valueOfXml(key)`, which throws for an unknown name) with its multiplier
    /// (`getDouble(key, 1.)`).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        let mut stats = Vec::with_capacity(template.params.len());
        for key in template.params.keys() {
            let stat = Stat::from_xml(key).ok_or_else(|| format!("Unknown stat: {key}"))?;
            stats.push((stat, params::double(template, key, 1.0)?));
        }
        Ok(Box::new(Self { stats }))
    }
}

impl EffectHandler for ServitorShare {
    fn effect_flags(&self) -> u32 {
        effect_flag::SERVITOR_SHARE
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effected.asPlayer()` (the skill targets the caster).
        let Some(player) = world.acting_player(cx.effected) else {
            return;
        };
        world.set_servitor_share(player, Some(self.stats.clone()));
        if let Some(summon) = world.summon_of(player) {
            // `summon.broadcastInfo()` then `getStatus().startHpMpRegeneration()`.
            world.broadcast_info(summon);
            world.start_hp_mp_regeneration(summon);
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        let Some(player) = world.acting_player(cx.effected) else {
            return;
        };
        world.set_servitor_share(player, None);
        if let Some(summon) = world.summon_of(player) {
            let max = world
                .stats(summon)
                .map(|v| (f64::from(v.max_hp()), f64::from(v.max_mp())));
            let current = world.core_of(summon).map(|c| (c.cur_hp, c.cur_mp));
            if let (Some((max_hp, max_mp)), Some((hp, mp))) = (max, current) {
                if hp > max_hp {
                    world.set_current_hp(summon, max_hp, true);
                }
                if mp > max_mp {
                    world.set_current_mp(summon, max_mp, true);
                }
            }
            world.broadcast_info(summon);
        }
    }
}
