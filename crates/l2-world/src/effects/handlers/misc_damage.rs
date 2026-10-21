//! `StaticDamage`, and the two party effects `RebalanceHP` and `TransferDamage` (the party side is
//! `party/buffs.rs`).
// Java: dist/game/data/scripts/handlers/skill/effects/StaticDamage.java
// Java: dist/game/data/scripts/handlers/skill/effects/RebalanceHP.java
// Java: dist/game/data/scripts/handlers/skill/effects/TransferDamage.java

use l2_data::skill::EffectTemplate;

use super::statset::get_int;
use crate::effects::SkillKey;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `StaticDamage`: a fixed amount of damage.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StaticDamage {
    power: i32,
}

impl StaticDamage {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(StaticDamage {
            power: get_int(template, "power", 0)?,
        }))
    }
}

impl EffectHandler for StaticDamage {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_alike_dead(cx.effector) {
            return;
        }
        world.reduce_current_hp_by_skill(cx.effected, f64::from(self.power), Some(cx.effector), cx.skill);
        world.notify_damage_received_by(
            cx.effected,
            f64::from(self.power),
            Some(cx.effector),
            Some(SkillKey::of(cx.skill)),
            false,
            false,
        );
        if world.is_player(cx.effector) {
            world.send_damage_message(cx.effector, cx.effected, self.power, false, false, false);
        }
    }
}

/// Java `RebalanceHP`: shares the HP of a party. A caster outside a party gets nothing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RebalanceHp;

impl RebalanceHp {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(RebalanceHp))
    }
}

impl EffectHandler for RebalanceHp {
    fn effect_type(&self) -> EffectType {
        EffectType::RebalanceHp
    }

    fn is_instant(&self) -> bool {
        true
    }

    /// `if (!effector.isPlayer() || !effector.isInParty()) return;` and then the party members
    /// in range share their HP, their summons too (`party/buffs.rs`).
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.rebalance_party_hp(cx.effector, cx.skill);
    }
}

/// Java `TransferDamage`: the effected player passes part of the damage it takes to the caster
/// (the damage code reads the link only for a player with a party, `party/buffs.rs`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TransferDamage;

impl TransferDamage {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TransferDamage))
    }
}

impl EffectHandler for TransferDamage {
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effected.isPlayable() && effector.isPlayer()`: summons do not exist yet, so a player.
        if world.is_player(cx.effected) && world.is_player(cx.effector) {
            world.set_transfer_damage_to(cx.effected, Some(cx.effector));
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effected) && world.is_player(cx.effector) {
            world.set_transfer_damage_to(cx.effected, None);
        }
    }
}
