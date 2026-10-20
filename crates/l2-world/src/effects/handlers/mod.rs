//! The ported effect handlers, one file per Java class of
//! `dist/game/data/scripts/handlers/skill/effects/`.
// Java: dist/game/data/scripts/handlers/EffectMasterHandler.java
//
// A handler card adds its file here (`mod x;`) and one `register` line in `register_all`, in
// the alphabetical order of `EffectMasterHandler.EFFECTS`. See `effects/registry.rs` for the
// pattern; `buff.rs` is the reference handler.

mod block;
mod buff;
mod confuse;
mod disarm;
mod distrust;
mod enemy_charge;
mod fake_death;
mod fear;
mod flags;
mod focus;
mod hide;
mod hold;
mod immobile_buff;
mod misc;
mod mute;
mod player_slots;
mod relax;
mod silent_move;
mod status_params;
mod trait_effects;

use super::registry::EffectRegistry;

/// Registers every ported handler (Java `EffectMasterHandler.main`); a name that is not listed
/// here keeps its `Unported` stand-in.
pub(crate) fn register_all(registry: &mut EffectRegistry) {
    registry.register("Buff", buff::Buff::create);
    // M5-EFX-02: the status handlers.
    registry.register("AttackTrait", trait_effects::AttackTrait::create);
    registry.register("Betray", confuse::Betray::create);
    registry.register("BlockAbnormalSlot", block::BlockAbnormalSlot::create);
    registry.register("BlockAction", block::BlockAction::create);
    registry.register("BlockChat", block::BlockChat::create);
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
}
