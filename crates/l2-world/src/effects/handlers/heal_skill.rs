//! The `Heal` effect: the amount of HP a healing skill restores.
// Java: dist/game/data/scripts/handlers/skill/effects/Heal.java
//
// The amount is M5-FORM-02's `calc_heal_amount` (checked against Java vectors). M5-SHOT-01: the
// spiritshot charges are read as "not charged".

use l2_data::Stat;
use l2_data::item::CrystalType;
use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use super::params;
use super::vitals::{Vital, is_door};
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::formulas::{HealInput, HealWeaponGrade, calc_heal_amount};
use crate::world::World;

/// Java `Heal`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Heal {
    power: f64,
}

impl Heal {
    /// Java `Heal(attachCond, applyCond, set, params)`: `power` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for Heal {
    fn effect_type(&self) -> EffectType {
        EffectType::Heal
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_dead(effected) || is_door(world, effected) || world.is_invul(effected) {
            return;
        }
        let m_critical_hit = world.m_critical_hit_of(effector, effected, skill);
        let power = self.power;
        let amount = world.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(effector), world.stats(effected)) else {
                return 0.0;
            };
            let heal_effect = |amount: f64| t.calc_stat(Stat::HealEffect, amount, None, None);
            let input = HealInput {
                power,
                skill_is_magic: skill.is_magic(),
                skill_is_static: skill.is_static(),
                skill_mp_consume: skill.mp_consume,
                skill_item_consume_count: skill.item_consume_count,
                // M5-SHOT-01: `isChargedShot(SPIRITSHOTS)` and `isChargedShot(BLESSED_SPIRITSHOTS)`.
                effector_spiritshot_charged: false,
                effector_blessed_spiritshot_charged: false,
                effector_is_mage_player: a.player().is_some_and(|p| {
                    u16::try_from(p.data.class_id).is_ok_and(|id| l2_data::ClassId(id).is_mage())
                }),
                // Summons are M6.
                effector_is_summon: false,
                effector_is_npc: a.npc().is_some(),
                effector_weapon_grade: match a.active_weapon_instance() {
                    None => HealWeaponGrade::NoWeapon,
                    Some(_) => match a.active_weapon_item().map(|item| item.crystal_type) {
                        Some(CrystalType::S84) => HealWeaponGrade::S84,
                        Some(CrystalType::S80) => HealWeaponGrade::S80,
                        _ => HealWeaponGrade::Other,
                    },
                },
                effector_m_atk: a.m_atk(Some(effector), None),
                heal_effect: &heal_effect,
                effector_is_playable: a.is_playable(),
                // M5-CFG: `PLAYER_HEALING_SKILL_MULTIPLIERS` (Java default 1.0).
                player_healing_skill_multiplier: 1.0,
                effector_m_critical_hit: m_critical_hit,
                effected_max_recoverable_hp: t.max_recoverable_hp(),
                effected_current_hp: world.core_of(effected).map_or(0.0, |c| c.cur_hp),
            };
            calc_heal_amount(&input, rng)
        });

        // The formula already prevents overheal and a negative amount.
        Vital::Hp.add(world, effected, amount);

        if world.is_player(effected) {
            if skill.id == 4051 {
                world.send(effected, &SystemMessage::new(SystemMessageId::RejuvenatingHp));
            } else {
                // Only a player healer is named; an NPC healer gets the plain message.
                let by_other = world.is_player(effector) && effector != effected;
                Vital::Hp.send_restored(world, effector, effected, by_other, amount);
            }
        }
    }
}
