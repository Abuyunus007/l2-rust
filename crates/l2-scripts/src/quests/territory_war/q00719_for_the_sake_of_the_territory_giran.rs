//! Quest 719: For the Sake of the Territory - Giran.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00719_ForTheSakeOfTheTerritoryGiran.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00719_ForTheSakeOfTheTerritoryGiran`.
pub struct Q00719ForTheSakeOfTheTerritoryGiran;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00719_ForTheSakeOfTheTerritoryGiran",
    quest_id: 719,
    catapult_id: 36501,
    territory_id: 83,
    leader_ids: &[36520, 36522, 36525, 36593],
    guard_ids: &[36521, 36523, 36524],
    npc_string: &[NpcStringId::TheCatapultOfGiranHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00719ForTheSakeOfTheTerritoryGiran, DATA);
