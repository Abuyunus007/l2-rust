//! Quest 723: For the Sake of the Territory - Goddard.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00723_ForTheSakeOfTheTerritoryGoddard.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00723_ForTheSakeOfTheTerritoryGoddard`.
pub struct Q00723ForTheSakeOfTheTerritoryGoddard;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00723_ForTheSakeOfTheTerritoryGoddard",
    quest_id: 723,
    catapult_id: 36505,
    territory_id: 87,
    leader_ids: &[36544, 36546, 36549, 36597],
    guard_ids: &[36545, 36547, 36548],
    npc_string: &[NpcStringId::TheCatapultOfGoddardHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00723ForTheSakeOfTheTerritoryGoddard, DATA);
