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
mod block;
mod blow;
mod buff;
mod call_skill;
mod cancel;
mod change_face;
mod change_hair_color;
mod change_hair_style;
mod confuse;
mod consume_body;
mod convert_item;
mod cp_heal;
mod damage_over_time;
mod damage_support;
mod detect_hidden_objects;
mod detection;
mod disarm;
mod dispel_all;
mod dispel_by_category;
mod dispel_by_slot;
mod dispel_by_slot_probability;
mod distrust;
mod enemy_charge;
mod escape;
mod fake_death;
mod fear;
mod flags;
mod fly;
mod focus;
mod get_agro;
mod give_sp;
mod grow;
mod hate;
mod hate_chance;
mod heal;
mod heal_skill;
mod hide;
mod hold;
mod immobile_buff;
mod lethal;
mod magical_damage;
mod mana_heal;
mod max_vitals;
mod misc;
mod misc_damage;
mod mp_consume_per_level;
mod mute;
mod nevits_hourglass;
mod open_chest;
mod open_door;
mod open_recipe_book;
mod params;
mod physical_damage;
mod player_slots;
mod recovery;
mod relax;
mod restoration;
mod restoration_random;
mod resurrection;
mod ride;
mod servitor_share;
mod set_skill;
mod silent_move;
mod skill_calc;
mod spoil;
mod statset;
mod status_params;
mod steal_abnormal;
mod summon;
mod summon_cubic;
mod summon_pet;
mod sweeper;
mod target_me;
mod teleport;
mod teleport_to_target;
mod throw_up;
mod trait_effects;
pub(crate) mod trigger_skill;
mod unsummon;
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
    registry.register("Ride", ride::Ride::create);
    registry.register("SetSkill", set_skill::SetSkill::create);
    registry.register("StealAbnormal", steal_abnormal::StealAbnormal::create);
    registry.register("Teleport", teleport::Teleport::create);
    registry.register("TeleportToTarget", teleport_to_target::TeleportToTarget::create);
    registry.register("Transformation", crate::transform::effect::Transformation::create);
    registry.register("ThrowUp", throw_up::ThrowUp::create);
    registry.register("VitalityPointUp", vitality_point_up::VitalityPointUp::create);
    // M5-EFX-02: the status handlers.
    registry.register("AttackTrait", trait_effects::AttackTrait::create);
    registry.register("Betray", confuse::Betray::create);
    registry.register("BlockAbnormalSlot", block::BlockAbnormalSlot::create);
    registry.register("BlockAction", block::BlockAction::create);
    registry.register("BlockChat", block::BlockChat::create);
    registry.register("BlockParty", block::BlockParty::create);
    registry.register("BlockResurrection", flags::BlockResurrection::create);
    registry.register("Bluff", misc::Bluff::create);
    registry.register("ChameleonRest", relax::ChameleonRest::create);
    registry.register("Confuse", confuse::Confuse::create);
    registry.register("CrystalGradeModify", player_slots::CrystalGradeModify::create);
    registry.register("Debuff", flags::Debuff::create);
    registry.register("DefenceTrait", trait_effects::DefenceTrait::create);
    registry.register("Disarm", disarm::Disarm::create);
    registry.register("Distrust", distrust::Distrust::create);
    registry.register("EnableCloak", player_slots::EnableCloak::create);
    registry.register("EnemyCharge", enemy_charge::EnemyCharge::create);
    registry.register("EnlargeAbnormalSlot", block::EnlargeAbnormalSlot::create);
    registry.register("FakeDeath", fake_death::FakeDeath::create);
    registry.register("Fear", fear::Fear::create);
    registry.register("Flag", misc::Flag::create);
    registry.register("FocusEnergy", focus::FocusEnergy::create);
    registry.register("FocusMaxEnergy", focus::FocusMaxEnergy::create);
    registry.register("FocusSouls", focus::FocusSouls::create);
    registry.register("Hide", hide::Hide::create);
    registry.register("ImmobileBuff", immobile_buff::ImmobileBuff::create);
    registry.register("Invincible", flags::Invincible::create);
    registry.register("Lucky", flags::Lucky::create);
    registry.register("Mute", mute::Mute::create);
    registry.register("NoblesseBless", flags::NoblesseBless::create);
    registry.register("Paralyze", hold::Paralyze::create);
    registry.register("Passive", misc::Passive::create);
    registry.register("Petrification", hold::Petrification::create);
    registry.register("PhysicalDamageMute", mute::PhysicalDamageMute::create);
    registry.register("PhysicalMute", mute::PhysicalMute::create);
    registry.register("PolearmSingleTarget", flags::PolearmSingleTarget::create);
    registry.register("ProtectionBlessing", flags::ProtectionBlessing::create);
    registry.register("Relax", relax::Relax::create);
    registry.register("ResistSkill", block::ResistSkill::create);
    registry.register("Root", hold::Root::create);
    registry.register("RunAway", fear::RunAway::create);
    registry.register("SilentMove", silent_move::SilentMove::create);
    registry.register("SkillTurning", misc::SkillTurning::create);
    registry.register("Sleep", hold::Sleep::create);
    registry.register("SoulEating", focus::SoulEating::create);
    registry.register("Stun", hold::Stun::create);
    registry.register("TalismanSlot", player_slots::TalismanSlot::create);
    registry.register("TargetCancel", misc::TargetCancel::create);
    // M5-EFX-05: the handlers that needed casting or new world parts.
    registry.register("CallSkill", call_skill::CallSkill::create);
    registry.register("ConvertItem", convert_item::ConvertItem::create);
    registry.register("Grow", grow::Grow::create);
    registry.register("NevitsHourglass", nevits_hourglass::NevitsHourglass::create);
    registry.register("OpenChest", open_chest::OpenChest::create);
    registry.register("RestorationRandom", restoration_random::RestorationRandom::create);
    registry.register("Resurrection", resurrection::Resurrection::create);
    registry.register("ResurrectionSpecial", resurrection::ResurrectionSpecial::create);
    registry.register("TargetMe", target_me::TargetMe::create);
    registry.register("TriggerSkillByAvoid", trigger_skill::ByAvoid::create);
    registry.register("TriggerSkillByDamageDealt", trigger_skill::ByDamageDealt::create);
    registry.register(
        "TriggerSkillByDamageReceived",
        trigger_skill::ByDamageReceived::create,
    );
    registry.register("TriggerSkillBySkill", trigger_skill::BySkill::create);
    // M5-SPOIL-01.
    registry.register("Spoil", spoil::Spoil::create);
    registry.register("Sweeper", sweeper::Sweeper::create);
    // M5-EFX-04: damage, Heal, the hate chances.
    registry.register("Backstab", blow::Backstab::create);
    registry.register("DeathLink", magical_damage::DeathLink::create);
    registry.register("DeleteHate", hate_chance::DeleteHate::create);
    registry.register("DeleteHateOfMe", hate_chance::DeleteHateOfMe::create);
    registry.register("EnergyDamage", physical_damage::EnergyDamage::create);
    registry.register("FatalBlow", blow::FatalBlow::create);
    registry.register("Heal", heal_skill::Heal::create);
    registry.register("HpDrain", magical_damage::HpDrain::create);
    registry.register("Lethal", lethal::Lethal::create);
    registry.register("MagicalDamage", magical_damage::MagicalDamage::create);
    registry.register(
        "MagicalDamageByAbnormal",
        magical_damage::MagicalDamageByAbnormal::create,
    );
    registry.register("MagicalDamageMp", magical_damage::MagicalDamageMp::create);
    registry.register("MagicalSoulDamage", magical_damage::MagicalSoulDamage::create);
    registry.register("PhysicalDamage", physical_damage::PhysicalDamage::create);
    registry.register(
        "PhysicalDamageHpLink",
        physical_damage::PhysicalDamageHpLink::create,
    );
    registry.register("PhysicalSoulDamage", physical_damage::PhysicalSoulDamage::create);
    registry.register("RandomizeHate", hate_chance::RandomizeHate::create);
    registry.register("SoulBlow", blow::SoulBlow::create);
    registry.register("TargetMeProbability", hate_chance::TargetMeProbability::create);
    registry.register("TransferHate", hate_chance::TransferHate::create);
    // M6-SUMMON-01: servitors.
    registry.register("ServitorShare", servitor_share::ServitorShare::create);
    registry.register("Summon", summon::Summon::create);
    // M6-PET-01.
    registry.register("SummonPet", summon_pet::SummonPet::create);
    registry.register("Unsummon", unsummon::Unsummon::create);
    registry.register("ImmobilePetBuff", immobile_buff::ImmobilePetBuff::create);
    // M6-CRAFT-01: the recipe book skills.
    registry.register(
        "OpenCommonRecipeBook",
        open_recipe_book::OpenCommonRecipeBook::create,
    );
    registry.register(
        "OpenDwarfRecipeBook",
        open_recipe_book::OpenDwarfRecipeBook::create,
    );
    // M6-CUBIC-01: cubics.
    registry.register("CubicMastery", summon_cubic::CubicMastery::create);
    registry.register("SummonCubic", summon_cubic::SummonCubic::create);
}
