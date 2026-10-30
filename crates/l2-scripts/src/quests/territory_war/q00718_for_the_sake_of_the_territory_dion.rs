//! Quest 718: For the Sake of the Territory - Dion.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00718_ForTheSakeOfTheTerritoryDion.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00718_ForTheSakeOfTheTerritoryDion`.
pub struct Q00718ForTheSakeOfTheTerritoryDion;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00718_ForTheSakeOfTheTerritoryDion",
    quest_id: 718,
    catapult_id: 36500,
    territory_id: 82,
    leader_ids: &[36514, 36516, 36519, 36592],
    guard_ids: &[36515, 36517, 36518],
    npc_string: &[NpcStringId::TheCatapultOfDionHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00718ForTheSakeOfTheTerritoryDion, DATA);
