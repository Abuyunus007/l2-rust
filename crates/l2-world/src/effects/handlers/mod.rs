//! The ported effect handlers, one file per Java class of
//! `dist/game/data/scripts/handlers/skill/effects/`.
// Java: dist/game/data/scripts/handlers/EffectMasterHandler.java
//
// A handler card adds its file here (`mod x;`) and one `register` line in `register_all`, in
// the alphabetical order of `EffectMasterHandler.EFFECTS`. See `effects/registry.rs` for the
// pattern; `buff.rs` is the reference handler.

mod buff;
mod cp_heal;
mod damage_over_time;
mod damage_support;
mod heal;
mod mana_heal;
mod max_vitals;
mod misc_damage;
mod statset;
mod vitals;

use super::registry::EffectRegistry;

/// Registers every ported handler (Java `EffectMasterHandler.main`); a name that is not listed
/// here keeps its `Unported` stand-in.
pub(crate) fn register_all(registry: &mut EffectRegistry) {
    registry.register("Buff", buff::Buff::create);
    registry.register("CpDamPercent", damage_over_time::CpDamPercent::create);
    registry.register("CpHeal", cp_heal::CpHeal::create);
    registry.register("CpHealOverTime", cp_heal::CpHealOverTime::create);
    registry.register("CpHealPercent", cp_heal::CpHealPercent::create);
    registry.register("DamOverTime", damage_over_time::DamOverTime::create);
    registry.register("DamOverTimePercent", damage_over_time::DamOverTimePercent::create);
    registry.register("HealOverTime", heal::HealOverTime::create);
    registry.register("HealPercent", heal::HealPercent::create);
    registry.register("HpByLevel", heal::HpByLevel::create);
    registry.register("ManaDamOverTime", damage_over_time::ManaDamOverTime::create);
    registry.register("ManaHeal", mana_heal::ManaHeal::create);
    registry.register("ManaHealByLevel", mana_heal::ManaHealByLevel::create);
    registry.register("ManaHealOverTime", mana_heal::ManaHealOverTime::create);
    registry.register("ManaHealPercent", mana_heal::ManaHealPercent::create);
    registry.register("MaxCp", max_vitals::MaxCp::create);
    registry.register("MaxHp", max_vitals::MaxHp::create);
    registry.register("RebalanceHP", misc_damage::RebalanceHp::create);
    registry.register("StaticDamage", misc_damage::StaticDamage::create);
    registry.register("TransferDamage", misc_damage::TransferDamage::create);
}
