//! The quest lists of Java's `TerritoryWarSuperClassLoader`: which quest belongs to a territory,
//! to a class, and the "Protect the ..." quests.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/TerritoryWarSuperClassLoader.java
//
// Java fills three `HashMap`s in `main`; the quests are statics here, so the maps are lookups.

use super::q00717_for_the_sake_of_the_territory_gludio;
use super::q00718_for_the_sake_of_the_territory_dion;
use super::q00719_for_the_sake_of_the_territory_giran;
use super::q00720_for_the_sake_of_the_territory_oren;
use super::q00721_for_the_sake_of_the_territory_aden;
use super::q00722_for_the_sake_of_the_territory_innadril;
use super::q00723_for_the_sake_of_the_territory_goddard;
use super::q00724_for_the_sake_of_the_territory_rune;
use super::q00725_for_the_sake_of_the_territory_schuttgart;
use super::q00729_protect_the_territory_catapult;
use super::q00730_protect_the_supplies_safe;
use super::q00731_protect_the_military_association_leader;
use super::q00732_protect_the_religious_association_leader;
use super::q00733_protect_the_economic_association_leader;
use super::q00734_pierce_through_a_shield;
use super::q00735_make_spears_dull;
use super::q00736_weaken_the_magic;
use super::q00737_deny_blessings;
use super::q00738_destroy_key_targets;
use super::territory_war_super_class::TerritoryWarData;

/// `getForTheSakeScripts()`, keyed by `TERRITORY_ID`.
pub static FOR_THE_SAKE: [&TerritoryWarData; 9] = [
    &q00717_for_the_sake_of_the_territory_gludio::DATA,
    &q00718_for_the_sake_of_the_territory_dion::DATA,
    &q00719_for_the_sake_of_the_territory_giran::DATA,
    &q00720_for_the_sake_of_the_territory_oren::DATA,
    &q00721_for_the_sake_of_the_territory_aden::DATA,
    &q00722_for_the_sake_of_the_territory_innadril::DATA,
    &q00723_for_the_sake_of_the_territory_goddard::DATA,
    &q00724_for_the_sake_of_the_territory_rune::DATA,
    &q00725_for_the_sake_of_the_territory_schuttgart::DATA,
];

/// `getProtectTheScripts()` (keyed by quest id).
pub static PROTECT_THE: [&TerritoryWarData; 5] = [
    &q00729_protect_the_territory_catapult::DATA,
    &q00730_protect_the_supplies_safe::DATA,
    &q00731_protect_the_military_association_leader::DATA,
    &q00732_protect_the_religious_association_leader::DATA,
    &q00733_protect_the_economic_association_leader::DATA,
];

/// The "Kill the" quests in the order `main` puts their class ids in `getKillTheScripts()`.
pub static KILL_THE: [&TerritoryWarData; 5] = [
    &q00734_pierce_through_a_shield::DATA,
    &q00735_make_spears_dull::DATA,
    &q00736_weaken_the_magic::DATA,
    &q00737_deny_blessings::DATA,
    &q00738_destroy_key_targets::DATA,
];

/// Java `getForTheSakeScripts().get(territoryId)`.
pub fn for_the_sake(territory_id: i32) -> Option<&'static TerritoryWarData> {
    FOR_THE_SAKE
        .iter()
        .copied()
        .find(|q| q.territory_id == territory_id)
}

/// Java `getKillTheScripts().get(classId)`. A later quest replaces an earlier one that lists the
/// same class (`HashMap.put`).
pub fn kill_the(class_id: i32) -> Option<&'static TerritoryWarData> {
    KILL_THE
        .iter()
        .rev()
        .copied()
        .find(|q| q.class_ids.contains(&class_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_territories_have_their_quests() {
        for territory in 81..=89 {
            let quest = for_the_sake(territory).unwrap();
            assert_eq!(quest.territory_id, territory);
            assert_eq!(quest.quest_id, 717 + (territory - 81));
            assert_eq!(quest.leader_ids.len(), 4);
            assert_eq!(quest.guard_ids.len(), 3);
        }
        assert!(for_the_sake(80).is_none() && for_the_sake(90).is_none());
    }

    #[test]
    fn the_classes_have_their_kill_the_quest() {
        // Bishop (16) is a healer, a Gladiator (2) a warrior, a Paladin (5) a knight.
        assert_eq!(kill_the(16).unwrap().quest_id, 737);
        assert_eq!(kill_the(2).unwrap().quest_id, 735);
        assert_eq!(kill_the(5).unwrap().quest_id, 734);
        assert_eq!(kill_the(51).unwrap().quest_id, 738);
        assert_eq!(kill_the(13).unwrap().quest_id, 736);
        // A first-class fighter has none.
        assert!(kill_the(0).is_none());
    }

    #[test]
    fn no_class_is_in_two_kill_the_quests() {
        for (i, a) in KILL_THE.iter().enumerate() {
            for b in &KILL_THE[i + 1..] {
                assert!(a.class_ids.iter().all(|c| !b.class_ids.contains(c)));
            }
        }
    }
}
