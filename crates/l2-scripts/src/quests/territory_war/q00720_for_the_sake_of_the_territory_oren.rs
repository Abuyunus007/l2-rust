//! Quest 720: For the Sake of the Territory - Oren.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00720_ForTheSakeOfTheTerritoryOren.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00720_ForTheSakeOfTheTerritoryOren`.
pub struct Q00720ForTheSakeOfTheTerritoryOren;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00720_ForTheSakeOfTheTerritoryOren",
    quest_id: 720,
    catapult_id: 36502,
    territory_id: 84,
    leader_ids: &[36526, 36528, 36531, 36594],
    guard_ids: &[36527, 36529, 36530],
    npc_string: &[NpcStringId::TheCatapultOfOrenHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00720ForTheSakeOfTheTerritoryOren, DATA);
