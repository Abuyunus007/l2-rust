//! Quest 721: For the Sake of the Territory - Aden.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00721_ForTheSakeOfTheTerritoryAden.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00721_ForTheSakeOfTheTerritoryAden`.
pub struct Q00721ForTheSakeOfTheTerritoryAden;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00721_ForTheSakeOfTheTerritoryAden",
    quest_id: 721,
    catapult_id: 36503,
    territory_id: 85,
    leader_ids: &[36532, 36534, 36537, 36595],
    guard_ids: &[36533, 36535, 36536],
    npc_string: &[NpcStringId::TheCatapultOfAdenHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00721ForTheSakeOfTheTerritoryAden, DATA);
