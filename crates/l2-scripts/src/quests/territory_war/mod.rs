//! `data/scripts/quests/TerritoryWarScripts`: the Territory War quests (D-05), on the territory
//! API of `l2_world::territory` (M7-TW-01).
//!
//! The base class is `territory_war_super_class` (the data of a quest, the hooks, the script the
//! war manager talks to) and `territory_war_super_class_loader` the lists of Java's loader.

pub mod q00717_for_the_sake_of_the_territory_gludio;
pub mod q00718_for_the_sake_of_the_territory_dion;
pub mod q00719_for_the_sake_of_the_territory_giran;
pub mod q00720_for_the_sake_of_the_territory_oren;
pub mod q00721_for_the_sake_of_the_territory_aden;
pub mod q00722_for_the_sake_of_the_territory_innadril;
pub mod q00723_for_the_sake_of_the_territory_goddard;
pub mod q00724_for_the_sake_of_the_territory_rune;
pub mod q00725_for_the_sake_of_the_territory_schuttgart;
pub mod q00729_protect_the_territory_catapult;
pub mod q00730_protect_the_supplies_safe;
pub mod q00731_protect_the_military_association_leader;
pub mod q00732_protect_the_religious_association_leader;
pub mod q00733_protect_the_economic_association_leader;
pub mod q00734_pierce_through_a_shield;
pub mod q00735_make_spears_dull;
pub mod q00736_weaken_the_magic;
pub mod q00737_deny_blessings;
pub mod q00738_destroy_key_targets;
pub mod territory_war_super_class;
pub mod territory_war_super_class_loader;

use l2_world::ScriptRegistry;

/// Java's `TerritoryWarSuperClassLoader.main`: the super class, then the quests by id.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(territory_war_super_class::TerritoryWarSuperClass);
    r.add(q00717_for_the_sake_of_the_territory_gludio::Q00717ForTheSakeOfTheTerritoryGludio);
    r.add(q00718_for_the_sake_of_the_territory_dion::Q00718ForTheSakeOfTheTerritoryDion);
    r.add(q00719_for_the_sake_of_the_territory_giran::Q00719ForTheSakeOfTheTerritoryGiran);
    r.add(q00720_for_the_sake_of_the_territory_oren::Q00720ForTheSakeOfTheTerritoryOren);
    r.add(q00721_for_the_sake_of_the_territory_aden::Q00721ForTheSakeOfTheTerritoryAden);
    r.add(q00722_for_the_sake_of_the_territory_innadril::Q00722ForTheSakeOfTheTerritoryInnadril);
    r.add(q00723_for_the_sake_of_the_territory_goddard::Q00723ForTheSakeOfTheTerritoryGoddard);
    r.add(q00724_for_the_sake_of_the_territory_rune::Q00724ForTheSakeOfTheTerritoryRune);
    r.add(q00725_for_the_sake_of_the_territory_schuttgart::Q00725ForTheSakeOfTheTerritorySchuttgart);
    r.add(q00729_protect_the_territory_catapult::Q00729ProtectTheTerritoryCatapult);
    r.add(q00730_protect_the_supplies_safe::Q00730ProtectTheSuppliesSafe);
    r.add(q00731_protect_the_military_association_leader::Q00731ProtectTheMilitaryAssociationLeader);
    r.add(q00732_protect_the_religious_association_leader::Q00732ProtectTheReligiousAssociationLeader);
    r.add(q00733_protect_the_economic_association_leader::Q00733ProtectTheEconomicAssociationLeader);
    r.add(q00734_pierce_through_a_shield::Q00734PierceThroughAShield);
    r.add(q00735_make_spears_dull::Q00735MakeSpearsDull);
    r.add(q00736_weaken_the_magic::Q00736WeakenTheMagic);
    r.add(q00737_deny_blessings::Q00737DenyBlessings);
    r.add(q00738_destroy_key_targets::Q00738DestroyKeyTargets);
}
