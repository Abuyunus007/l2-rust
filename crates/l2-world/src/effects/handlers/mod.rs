//! The ported effect handlers, one file per Java class of
//! `dist/game/data/scripts/handlers/skill/effects/`.
// Java: dist/game/data/scripts/handlers/EffectMasterHandler.java
//
// A handler card adds its file here (`mod x;`) and one `register` line in `register_all`, in
// the alphabetical order of `EffectMasterHandler.EFFECTS`. See `effects/registry.rs` for the
// pattern; `buff.rs` is the reference handler.

mod add_hate;
mod apply_skill_effects;
mod blink;
mod buff;
mod cancel;
mod change_face;
mod change_hair_color;
mod change_hair_style;
mod consume_body;
mod cp_heal;
mod damage_over_time;
mod damage_support;
mod detect_hidden_objects;
mod detection;
mod dispel_all;
mod dispel_by_category;
mod dispel_by_slot;
mod dispel_by_slot_probability;
mod escape;
mod fly;
mod get_agro;
mod give_sp;
mod hate;
mod heal;
mod mana_heal;
mod max_vitals;
mod misc_damage;
mod mp_consume_per_level;
mod open_door;
mod params;
mod recovery;
mod restoration;
mod set_skill;
mod statset;
mod steal_abnormal;
mod teleport;
mod teleport_to_target;
mod throw_up;
mod vitality_point_up;
mod vitals;

use super::registry::EffectRegistry;

/// Registers every ported handler (Java `EffectMasterHandler.main`); a name that is not listed
/// here keeps its `Unported` stand-in.
pub(crate) fn register_all(registry: &mut EffectRegistry) {
    registry.register("AddHate", add_hate::AddHate::create);
    registry.register(
        "ApplySkillEffects",
        apply_skill_effects::ApplySkillEffects::create,
    );
    registry.register("Blink", blink::Blink::create);
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
    registry.register("ChangeFace", change_face::ChangeFace::create);
    registry.register("ChangeHairColor", change_hair_color::ChangeHairColor::create);
    registry.register("ChangeHairStyle", change_hair_style::ChangeHairStyle::create);
    registry.register("ConsumeBody", consume_body::ConsumeBody::create);
    registry.register(
        "DetectHiddenObjects",
        detect_hidden_objects::DetectHiddenObjects::create,
    );
    registry.register("Detection", detection::Detection::create);
    registry.register("DispelAll", dispel_all::DispelAll::create);
    registry.register("DispelByCategory", dispel_by_category::DispelByCategory::create);
    registry.register("DispelBySlot", dispel_by_slot::DispelBySlot::create);
    registry.register(
        "DispelBySlotProbability",
        dispel_by_slot_probability::DispelBySlotProbability::create,
    );
    registry.register("Escape", escape::Escape::create);
    registry.register("GetAgro", get_agro::GetAgro::create);
    registry.register("GiveSp", give_sp::GiveSp::create);
    registry.register(
        "MpConsumePerLevel",
        mp_consume_per_level::MpConsumePerLevel::create,
    );
    registry.register("OpenDoor", open_door::OpenDoor::create);
    registry.register("Recovery", recovery::Recovery::create);
    registry.register("Restoration", restoration::Restoration::create);
    registry.register("SetSkill", set_skill::SetSkill::create);
    registry.register("StealAbnormal", steal_abnormal::StealAbnormal::create);
    registry.register("Teleport", teleport::Teleport::create);
    registry.register("TeleportToTarget", teleport_to_target::TeleportToTarget::create);
    registry.register("ThrowUp", throw_up::ThrowUp::create);
    registry.register("VitalityPointUp", vitality_point_up::VitalityPointUp::create);
}
